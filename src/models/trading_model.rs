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
/// Filters all zones that meet the `threshold`.
/// Clusters them together if they are within `max_gap` of each other.
/// Computes the mass and center of gravity for each cluster.
pub(crate) fn find_target_zones(scores: &[f64], threshold: f64, max_gap: usize) -> Vec<TargetZone> {
    if scores.is_empty() {
        return Vec::new();
    }

    // Step 1: Identify all "Land" indices (scores above threshold)
    let valid_indices: Vec<usize> = scores
        .iter()
        .enumerate()
        .filter(|&(_, &score)| score >= threshold)
        .map(|(i, _)| i)
        .collect();

    if valid_indices.is_empty() {
        return Vec::new();
    }

    let mut targets = Vec::new();
    let mut cluster_start = valid_indices[0];
    let mut prev_idx = valid_indices[0];

    // Helper to finalize a cluster
    let mut finalize_cluster = |start: usize, end: usize| {
        targets.push(TargetZone {
            start_idx: start,
            end_idx: end,
        });
    };

    // Step 2: Cluster indices based on max_gap
    for &idx in valid_indices.iter().skip(1) {
        // If the distance to the previous index is greater than gap + 1, the bridge breaks.
        // e.g. indices [2, 4] with max_gap 1. 4 - 2 = 2. (gap is 1). <= 2. Bridge holds.
        // e.g. indices [2, 5] with max_gap 1. 5 - 2 = 3. Bridge breaks.
        if idx - prev_idx > max_gap + 1 {
            // Finalize previous cluster
            finalize_cluster(cluster_start, prev_idx);
            // Start new cluster
            cluster_start = idx;
        }
        prev_idx = idx;
    }

    // Finalize the last cluster
    finalize_cluster(cluster_start, prev_idx);

    targets
}

/// A single price zone with its properties
#[derive(Debug, Clone)]
pub struct Zone {
    pub index: usize,
    pub price_bottom: Price,
    pub price_top: Price,
    // pub price_center: Price,
}

/// A SuperZone representing one or more contiguous zones of the same type
/// Aggregates adjacent zones to reduce visual noise and provide more meaningful ranges
#[derive(Debug, Clone)]
pub(crate) struct SuperZone {
    pub id: usize, // Unique identifier for this superzone (based on first zone index)
    pub price_bottom: Price,
    pub price_top: Price,
    pub price_center: Price,
}

impl Zone {
    fn new(index: usize, price_min: f64, price_max: f64, zone_count: usize) -> Self {
        let zone_height = (price_max - price_min) / zone_count as f64;
        let price_bottom = price_min + (index as f64 * zone_height);
        let price_top = price_bottom + zone_height;

        Self {
            index,
            price_bottom: Price::new(price_bottom),
            price_top: Price::new(price_top),
            // price_center: Price::new(price_center),
        }
    }
}

impl SuperZone {
    /// Create a SuperZone from a list of contiguous zones
    fn from_zones(zones: Vec<Zone>) -> Self {
        assert!(
            !zones.is_empty(),
            "Cannot create SuperZone from empty zone list"
        );

        let first = zones.first().unwrap();
        let last = zones.last().unwrap();

        let price_bottom = first.price_bottom;
        let price_top = last.price_top;
        let price_center = (price_bottom + price_top) / 2.0;

        Self {
            id: first.index,
            // index_range: (first.index, last.index),
            price_bottom,
            price_top,
            price_center: Price::new(price_center),
            // constituent_zones: zones,
        }
    }

    /// Check if a price is within this superzone
    pub(crate) fn contains(&self, price: Price) -> bool {
        price >= self.price_bottom && price <= self.price_top
    }
}

/// Aggregate contiguous zones into SuperZones
/// Adjacent zones (index differs by 1) are merged into a single SuperZone
fn aggregate_zones(zones: &[Zone]) -> Vec<SuperZone> {
    if zones.is_empty() {
        return Vec::new();
    }

    let mut superzones = Vec::new();
    let mut current_group = vec![zones[0].clone()];

    for i in 1..zones.len() {
        let prev_index = zones[i - 1].index;
        let curr_index = zones[i].index;

        if curr_index == prev_index + 1 {
            // Contiguous - add to current group
            current_group.push(zones[i].clone());
        } else {
            // Gap found - finalize current group and start new one
            superzones.push(SuperZone::from_zones(current_group));
            current_group = vec![zones[i].clone()];
        }
    }
    if !current_group.is_empty() {
        superzones.push(SuperZone::from_zones(current_group));
    }

    superzones
}

/// Classified zones representing different trading characteristics
#[derive(Debug, Clone, Default)]
pub(crate) struct ClassifiedZones {
    // Raw fixed-width zones
    // pub low_wicks: Vec<Zone>,
    // pub high_wicks: Vec<Zone>,
    // pub sticky: Vec<Zone>,

    // SuperZones (aggregated contiguous zones)
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

/// Complete trading model for a pair containing CVA and classified zones
/// This is the domain model independent of UI/plotting concerns
#[derive(Debug, Clone)]
pub(crate) struct TradingModel {
    // pub pair_name: String,
    pub cva: Arc<CVACore>,
    pub zones: ClassifiedZones,
    pub coverage: ZoneCoverageStats,
    pub segments: Vec<DisplaySegment>,
    pub opportunities: Vec<TradeOpportunity>,
}

impl TradingModel {
    /// Create a new trading model from CVA results and optional current price
    pub(crate) fn from_cva(cva: Arc<CVACore>, ohlcv: &OhlcvTimeSeries) -> Self {
        let (classified, stats) = Self::classify_zones(&cva, &DEFAULT_ZONE_CONFIG);

        let (low, high) = cva.price_range.min_max();
        let bounds: (Price, Price) = (Price::new(low), Price::new(high));

        let merge_ms = SEGMENT_MERGE_TOLERANCE_MS;

        let segments = RangeGapFinder::analyze(ohlcv, &cva.included_ranges, bounds, merge_ms);

        Self {
            cva,
            // profile,
            zones: classified,
            coverage: stats,
            // pair_name: ohlcv.pair_interval.name().to_string(),
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
                // VIABILITY GATE (Absolute)
                // Filter out bins that represent insignificant noise relative to the total.
                let viable_data: Vec<f64> = if resource_total > 0.0 {
                    raw_data
                        .iter()
                        .map(|&x| {
                            if (x / resource_total) >= params.viability_pct.value() {
                                x
                            } else {
                                0.0
                            }
                        })
                        .collect()
                } else {
                    raw_data.to_vec()
                };

                // SMOOTH
                let smooth_window =
                    ((zone_count as f64 * params.smooth_pct.value()).ceil() as usize).max(1) | 1;
                let smoothed = smooth_data(&viable_data, smooth_window);

                // NORMALIZE (Max)
                let normalized = normalize_max(&smoothed);

                // ADAPTIVE THRESHOLD (Relative)
                let (mean, std_dev) = mean_and_stddev(&normalized);

                // Threshold = Mean + (Sigma * StdDev)
                // We clamp it between 0.05 and 0.95 to prevent
                // "Selecting Everything" (if flat) or "Selecting Nothing" (if extreme outliers).
                let adaptive_threshold =
                    (mean + (params.sigma.value() * std_dev)).clamp(0.05, 0.95);

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

                // FIND TARGETS
                let gap = (zone_count as f64 * params.gap_pct.value()).ceil() as usize;
                // Note: We use 'normalized' data against the adaptive threshold.
                // We SKIP the 'Sharpening/Contrast' step because Z-Score handles the filtering statistically.
                let targets = find_target_zones(&normalized, adaptive_threshold, gap);

                let zones: Vec<Zone> = targets
                    .iter()
                    .flat_map(|t| t.start_idx..=t.end_idx)
                    .map(|idx| Zone::new(idx, price_min, price_max, zone_count))
                    .collect();
                let superzones = aggregate_zones(&zones);

                (zones, superzones)
            };

            // Sticky Zones
            // Resource: Total Volume in this range
            let total_volume: f64 = cva.get_scores_ref(ScoreType::FullCandleTVW).iter().sum();

            let (sticky, sticky_superzones) = process_layer(
                cva.get_scores_ref(ScoreType::FullCandleTVW),
                config.sticky,
                total_volume,
                "STICKY",
            );

            // Reversal Zones
            // Resource: Total Candles (Opportunity count)
            // Note: Use total_candles, NOT sum of scores (which is inflated by width)

            // Low Wicks
            let (low_wicks, low_wicks_superzones) = process_layer(
                cva.get_scores_ref(ScoreType::LowWickCount),
                config.reversal,
                total_candles,
                "LOW WICKS",
            );

            // High Wicks
            let (high_wicks, high_wicks_superzones) = process_layer(
                cva.get_scores_ref(ScoreType::HighWickCount),
                config.reversal,
                total_candles,
                "HIGH WICKS",
            );

            // Calculate Coverage Statistics
            let calc_coverage = |zones: &[Zone]| -> f64 {
                if zone_count == 0 {
                    return 0.0;
                }
                (zones.len() as f64 / zone_count as f64) * 100.0
            };

            let stats = ZoneCoverageStats {
                sticky_pct: calc_coverage(&sticky),
                support_pct: calc_coverage(&low_wicks),
                resistance_pct: calc_coverage(&high_wicks),
            };

            let classified = ClassifiedZones {
                sticky_superzones,
                low_wicks_superzones,
                high_wicks_superzones,
            };

            (classified, stats)
        })
    }
}
