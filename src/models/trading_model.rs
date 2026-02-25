use {
    crate::{
        config::{Price, ZoneClassificationConfig, ZoneParams},
        models::{
            CVACore, DEFAULT_ZONE_CONFIG, DisplaySegment, OhlcvTimeSeries, RangeGapFinder,
            SEGMENT_MERGE_TOLERANCE_MS, ScoreType, TradeOpportunity,
        },
        utils::{mean_and_stddev, normalize_max, smooth_data},
    },
    std::sync::Arc,
};

#[cfg(debug_assertions)]
use crate::config::DF;

/// Represents a clustered "Island" of activity.
#[derive(Debug, Clone)]
pub(crate) struct TargetZone {
    /// The starting index of this zone (inclusive)
    pub start_idx: usize,
    /// The ending index of this zone (inclusive)
    pub end_idx: usize,
}

/// Identifies target zones using the "Islands" strategy (Threshold + Clustering).
/// Scores at or above `threshold` are "land"; islands separated by gaps wider than
/// `max_gap` become distinct [`TargetZone`]s.
pub(crate) fn find_target_zones(scores: &[f64], threshold: f64, max_gap: usize) -> Vec<TargetZone> {
    let valid: Vec<usize> = scores
        .iter()
        .enumerate()
        .filter_map(|(i, &s)| (s >= threshold).then_some(i))
        .collect();

    if valid.is_empty() {
        return Vec::new();
    }

    let mut targets = Vec::new();

    let mut cluster_start = valid[0];
    let mut prev = valid[0];

    for &idx in valid.iter().skip(1) {
        // Bridge breaks when gap between land indices exceeds max_gap.
        // e.g. [2,4] max_gap=1 → 4-2=2 ≤ 2, holds. [2,5] → 5-2=3 > 2, breaks.
        if idx - prev > max_gap + 1 {
            targets.push(TargetZone {
                start_idx: cluster_start,
                end_idx: prev,
            });
            cluster_start = idx;
        }

        prev = idx;
    }
    targets.push(TargetZone {
        start_idx: cluster_start,
        end_idx: prev,
    });

    targets
}

#[derive(Debug, Clone)]
pub struct Zone {
    pub index: usize,
    pub price_bottom: Price,
    pub price_top: Price,
}

/// Aggregates one or more contiguous zones to reduce visual noise.
#[derive(Debug, Clone)]
pub(crate) struct SuperZone {
    pub price_bottom: Price,
    pub price_top: Price,
    pub price_center: Price,
}

impl Zone {
    fn new(index: usize, price_min: f64, price_max: f64, zone_count: usize) -> Self {
        let zone_height = (price_max - price_min) / zone_count as f64;

        let price_bottom = price_min + index as f64 * zone_height;
        Self {
            index,
            price_bottom: Price::new(price_bottom),

            price_top: Price::new(price_bottom + zone_height),
        }
    }
}

impl SuperZone {
    fn from_zones(zones: Vec<Zone>) -> Self {
        assert!(
            !zones.is_empty(),
            "Cannot create SuperZone from empty zone list"
        );
        let price_bottom = zones.first().unwrap().price_bottom;
        let price_top = zones.last().unwrap().price_top;
        Self {
            price_bottom,
            price_top,

            price_center: Price::new((price_bottom + price_top) / 2.0),
        }
    }

    pub(crate) fn contains(&self, price: Price) -> bool {
        price >= self.price_bottom && price <= self.price_top
    }
}

fn aggregate_zones(zones: &[Zone]) -> Vec<SuperZone> {
    if zones.is_empty() {
        return Vec::new();
    }

    let mut superzones = Vec::new();

    let mut group = vec![zones[0].clone()];

    for w in zones.windows(2) {
        if w[1].index == w[0].index + 1 {
            group.push(w[1].clone());
        } else {
            superzones.push(SuperZone::from_zones(group));
            group = vec![w[1].clone()];
        }
    }

    superzones.push(SuperZone::from_zones(group));

    superzones
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ClassifiedZones {
    pub sticky_superzones: Vec<SuperZone>,
    pub high_wicks_superzones: Vec<SuperZone>,
    pub low_wicks_superzones: Vec<SuperZone>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ZoneCoverageStats {
    pub sticky_pct: f64,
    pub resistance_pct: f64,
    pub support_pct: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct TradingModel {
    pub cva: Arc<CVACore>,
    pub zones: ClassifiedZones,
    pub coverage: ZoneCoverageStats,
    pub segments: Vec<DisplaySegment>,
    pub opportunities: Vec<TradeOpportunity>,
}

impl TradingModel {
    pub(crate) fn from_cva(cva: Arc<CVACore>, ohlcv: &OhlcvTimeSeries) -> Self {
        let (zones, coverage) = Self::classify_zones(&cva, &DEFAULT_ZONE_CONFIG);
        let (low, high) = cva.price_range.min_max();

        let bounds = (Price::new(low), Price::new(high));
        let segments = RangeGapFinder::analyze(
            ohlcv,
            &cva.included_ranges,
            bounds,
            SEGMENT_MERGE_TOLERANCE_MS,
        );
        Self {
            cva,
            zones,
            coverage,
            segments,
            opportunities: Vec::new(),
        }
    }

    fn classify_zones(
        cva: &CVACore,
        config: &ZoneClassificationConfig,
    ) -> (ClassifiedZones, ZoneCoverageStats) {
        let (price_min, price_max) = cva.price_range.min_max();
        let zone_count = cva.zone_count;
        let total_candles = cva.total_candles as f64;

        crate::trace_time!("Classify & Cluster Zones", 1000, {
            let process_layer = |raw_data: &[f64],
                                 params: ZoneParams,
                                 resource_total: f64,
                                 _layer_name: &str| {
                // VIABILITY GATE: zero out bins below the noise floor
                let viable_data: Vec<f64> = if resource_total > 0.0 {
                    raw_data
                        .iter()
                        .map(|&x| {
                            if x / resource_total >= params.viability_pct.value() {
                                x
                            } else {
                                0.0
                            }
                        })
                        .collect()
                } else {
                    raw_data.to_vec()
                };

                let smooth_window =
                    ((zone_count as f64 * params.smooth_pct.value()).ceil() as usize).max(1) | 1;

                let normalized = normalize_max(&smooth_data(&viable_data, smooth_window));

                let (mean, std_dev) = mean_and_stddev(&normalized);
                let adaptive_threshold = (mean + params.sigma.value() * std_dev).clamp(0.05, 0.95);

                #[cfg(debug_assertions)]
                if DF.log_zones {
                    let count = normalized.len();
                    let above = normalized
                        .iter()
                        .filter(|&&v| v >= adaptive_threshold)
                        .count();

                    // Count how many bins survived the Viability Gate
                    let pre_gate_nonzero = raw_data.iter().filter(|&&x| x > 0.0).count();
                    let post_gate_nonzero = viable_data.iter().filter(|&&x| x > 0.0).count();
                    let killed_by_gate = pre_gate_nonzero.saturating_sub(post_gate_nonzero);

                    log::info!(
                        "STATS [{}] for {}: TotalRes={:.1}  | Viable Threshold={:.1} | Mean={:.3} | StdDev={:.3} | Sigma={}",
                        _layer_name,
                        cva.pair_name,
                        resource_total,
                        resource_total * params.viability_pct.value(),
                        mean,
                        std_dev,
                        params.sigma
                    );

                    if killed_by_gate > 0 {
                        log::warn!(
                            "   🛑 VIABILITY GATE: Killed {} bins (Noise below {:.4})",
                            killed_by_gate,
                            params.viability_pct
                        );
                    }

                    log::info!(
                        "   -> Adaptive Cutoff: {:.4} | Passing: {}/{} ({:.1}%)",
                        adaptive_threshold,
                        above,
                        count,
                        (above as f64 / count as f64) * 100.0
                    );
                }

                let gap = (zone_count as f64 * params.gap_pct.value()).ceil() as usize;

                let targets = find_target_zones(&normalized, adaptive_threshold, gap);

                let zones: Vec<Zone> = targets
                    .iter()
                    .flat_map(|t| t.start_idx..=t.end_idx)
                    .map(|idx| Zone::new(idx, price_min, price_max, zone_count))
                    .collect();
                let superzones = aggregate_zones(&zones);

                (zones, superzones)
            };

            let total_volume: f64 = cva.get_scores_ref(ScoreType::FullCandleTVW).iter().sum();

            let (sticky, sticky_superzones) = process_layer(
                cva.get_scores_ref(ScoreType::FullCandleTVW),
                config.sticky,
                total_volume,
                "STICKY",
            );

            let (low_wicks, low_wicks_superzones) = process_layer(
                cva.get_scores_ref(ScoreType::LowWickCount),
                config.reversal,
                total_candles,
                "LOW WICKS",
            );

            let (high_wicks, high_wicks_superzones) = process_layer(
                cva.get_scores_ref(ScoreType::HighWickCount),
                config.reversal,
                total_candles,
                "HIGH WICKS",
            );

            let coverage_pct = |zones: &[Zone]| {
                if zone_count == 0 {
                    0.0
                } else {
                    zones.len() as f64 / zone_count as f64 * 100.0
                }
            };

            (
                ClassifiedZones {
                    sticky_superzones,
                    low_wicks_superzones,
                    high_wicks_superzones,
                },
                ZoneCoverageStats {
                    sticky_pct: coverage_pct(&sticky),
                    support_pct: coverage_pct(&low_wicks),
                    resistance_pct: coverage_pct(&high_wicks),
                },
            )
        })
    }
}
