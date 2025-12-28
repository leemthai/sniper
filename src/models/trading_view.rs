use std::sync::Arc;
use std::fmt;

// User crates
use crate::analysis::zone_scoring::find_target_zones;
use crate::analysis::range_gap_finder::{RangeGapFinder, DisplaySegment};
use crate::analysis::scenario_simulator::SimulationResult;

use crate::config::{ANALYSIS, AnalysisConfig};
use crate::config::ZoneParams;

use crate::models::cva::{CVACore, ScoreType};
use crate::models::horizon_profile::HorizonProfile;
use crate::models::OhlcvTimeSeries;

use crate::utils::maths_utils::{normalize_max, smooth_data, calculate_stats, calculate_expected_roi_pct};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeDirection {
    Long,
    Short,
}

// Helper for UI display
impl fmt::Display for TradeDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TradeDirection::Long => write!(f, "Long"),
            TradeDirection::Short => write!(f, "Short"),
        }
    }
}


#[derive(Debug, Clone)]
pub struct TradeOpportunity {
    pub pair_name: String,
    pub target_zone_id: usize,
    pub direction: TradeDirection,
    pub start_price: f64,
    pub target_price: f64,
    pub stop_price: f64,
    pub simulation: SimulationResult,
}

impl TradeOpportunity {
    // ... existing new/methods ...

    /// Calculates the Expected ROI % per trade for this specific opportunity.
    pub fn expected_roi(&self) -> f64 {
        calculate_expected_roi_pct(
            self.start_price,
            self.target_price,
            self.stop_price,
            self.simulation.success_rate
        )
    }
}

/// A single price zone with its properties
#[derive(Debug, Clone)]
pub struct Zone {
    #[allow(dead_code)] // Useful for debugging and zone identification
    pub index: usize,
    pub price_bottom: f64,
    pub price_top: f64,
    pub price_center: f64,
}

/// A SuperZone representing one or more contiguous zones of the same type
/// Aggregates adjacent zones to reduce visual noise and provide more meaningful ranges
#[derive(Debug, Clone)]
pub struct SuperZone {
    /// Unique identifier for this superzone (based on first zone index)
    pub id: usize,
    /// Range of zone indices this superzone covers (inclusive)
    pub index_range: (usize, usize),
    pub price_bottom: f64,
    pub price_top: f64,
    pub price_center: f64,
    /// Original zones that make up this superzone (for debugging/analysis)
    pub constituent_zones: Vec<Zone>,
}

impl Zone {
    fn new(index: usize, price_min: f64, price_max: f64, zone_count: usize) -> Self {
        let zone_height = (price_max - price_min) / zone_count as f64;
        let price_bottom = price_min + (index as f64 * zone_height);
        let price_top = price_bottom + zone_height;
        let price_center = price_bottom + (zone_height / 2.0);

        Self {
            index,
            price_bottom,
            price_top,
            price_center,
        }
    }

    /// Distance from price to zone center
    pub fn distance_to(&self, price: f64) -> f64 {
        (self.price_center - price).abs()
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
            index_range: (first.index, last.index),
            price_bottom,
            price_top,
            price_center,
            constituent_zones: zones,
        }
    }

    /// Check if a price is within this superzone
    pub fn contains(&self, price: f64) -> bool {
        price >= self.price_bottom && price <= self.price_top
    }

    /// Distance from price to superzone center
    pub fn distance_to(&self, price: f64) -> f64 {
        (self.price_center - price).abs()
    }

    /// Number of constituent zones
    pub fn zone_count(&self) -> usize {
        self.constituent_zones.len()
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

    // Don't forget the last group
    if !current_group.is_empty() {
        superzones.push(SuperZone::from_zones(current_group));
    }

    superzones
}

/// Classified zones representing different trading characteristics
#[derive(Debug, Clone, Default)]
pub struct ClassifiedZones {
    // Raw fixed-width zones
    pub low_wicks: Vec<Zone>,
    pub high_wicks: Vec<Zone>,
    pub sticky: Vec<Zone>,

    // SuperZones (aggregated contiguous zones)
    pub sticky_superzones: Vec<SuperZone>,
    pub high_wicks_superzones: Vec<SuperZone>,
    pub low_wicks_superzones: Vec<SuperZone>,
}

/// Complete trading model for a pair containing CVA and classified zones
/// This is the domain model independent of UI/plotting concerns
#[derive(Debug, Clone)]
pub struct TradingModel {
    pub pair_name: String,
    pub cva: Arc<CVACore>,
    pub zones: ClassifiedZones,
    pub coverage: ZoneCoverageStats,
    pub profile: HorizonProfile,
    pub segments: Vec<DisplaySegment>,
    pub opportunities: Vec<TradeOpportunity>,
}

// New Struct for Stats
#[derive(Debug, Clone, Default)]
pub struct ZoneCoverageStats {
    pub sticky_pct: f64,
    pub resistance_pct: f64,
    pub support_pct: f64,
}

impl TradingModel {
    /// Create a new trading model from CVA results and optional current price
    pub fn from_cva(
        cva: Arc<CVACore>, 
        profile: HorizonProfile,
        ohlcv: &OhlcvTimeSeries,
        config: &AnalysisConfig,
    ) -> Self {
        let (classified, stats) = Self::classify_zones(&cva);
        
        // CALCULATE SEGMENTS (On Worker Thread)
        // 1 Day tolerance merges small "Price < PH" dips, but keeps structural data holes.
        let bounds = cva.price_range.min_max();
        let merge_ms = config.cva.segment_merge_tolerance_ms;
        
        let segments = RangeGapFinder::analyze(
            ohlcv, 
            &cva.included_ranges, 
            bounds, 
            merge_ms
        );

        Self {
            cva,
            profile,
            zones: classified,
            coverage: stats,
            pair_name: ohlcv.pair_interval.name().to_string(),
            segments,
            opportunities: Vec::new(),
        }
    }

    fn classify_zones(cva: &CVACore) -> (ClassifiedZones, ZoneCoverageStats) {
        let (price_min, price_max) = cva.price_range.min_max();
        let zone_count = cva.zone_count;
        let total_candles = cva.total_candles as f64;

        crate::trace_time!("Classify & Cluster Zones", 1000, {
            // Helper closure
            let process_layer = |raw_data: &[f64], params: ZoneParams, resource_total: f64, _layer_name: &str| {
                
                // STEP 1: VIABILITY GATE (Absolute)
                // Filter out bins that represent insignificant noise relative to the total.
                let viable_data: Vec<f64> = if resource_total > 0.0 {
                    raw_data.iter().map(|&x| {
                        if (x / resource_total) >= params.viability_pct {
                            x
                        } else {
                            0.0
                        }
                    }).collect()
                } else {
                    raw_data.to_vec()
                };

                // STEP 2: SMOOTH
                let smooth_window = ((zone_count as f64 * params.smooth_pct).ceil() as usize).max(1) | 1;
                let smoothed = smooth_data(&viable_data, smooth_window);

                // STEP 3: NORMALIZE (Max)
                let normalized = normalize_max(&smoothed);

                // STEP 4: ADAPTIVE THRESHOLD (Relative)
                let (mean, std_dev) = calculate_stats(&normalized);
                
                // Threshold = Mean + (Sigma * StdDev)
                // We clamp it between 0.05 and 0.95 to prevent 
                // "Selecting Everything" (if flat) or "Selecting Nothing" (if extreme outliers).
                let adaptive_threshold = (mean + (params.sigma * std_dev)).clamp(0.05, 0.95);

                // --- DIAGNOSTIC LOGGING ---
                // #[cfg(debug_assertions)]
                if false
                {
                    let count = normalized.len();
                    let above = normalized.iter().filter(|&&v| v >= adaptive_threshold).count();
                    
                    // Count how many bins survived the Viability Gate
                    let pre_gate_nonzero = raw_data.iter().filter(|&&x| x > 0.0).count();
                    let post_gate_nonzero = viable_data.iter().filter(|&&x| x > 0.0).count();
                    let killed_by_gate = pre_gate_nonzero.saturating_sub(post_gate_nonzero);

                    log::info!(
                        "STATS [{}] for {}: TotalRes={:.1}  | Viable Threshold={:.1} | Mean={:.3} | StdDev={:.3} | Sigma={:.1}",
                        _layer_name, cva.pair_name, resource_total, resource_total * params.viability_pct, mean, std_dev, params.sigma
                    );
                    
                    if killed_by_gate > 0 {
                        log::warn!("   🛑 VIABILITY GATE: Killed {} bins (Noise below {:.4})", killed_by_gate, params.viability_pct);
                    }

                    log::info!(
                        "   -> Adaptive Cutoff: {:.4} | Passing: {}/{} ({:.1}%)",
                        adaptive_threshold, above, count, (above as f64 / count as f64) * 100.0
                    );
                }
                // --------------------------

                // STEP 5: FIND TARGETS
                let gap = (zone_count as f64 * params.gap_pct).ceil() as usize;
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

            // --- Sticky Zones ---
            // Resource: Total Volume in this range
            let total_volume: f64 = cva.get_scores_ref(ScoreType::FullCandleTVW).iter().sum();
            
            let (sticky, sticky_superzones) = process_layer(
                cva.get_scores_ref(ScoreType::FullCandleTVW),
                ANALYSIS.zones.sticky,
                total_volume,
                "STICKY"
            );

            // --- Reversal Zones ---
            // Resource: Total Candles (Opportunity count)
            // Note: Use total_candles, NOT sum of scores (which is inflated by width)
            
            // 1. Low Wicks
            let (low_wicks, low_wicks_superzones) = process_layer(
                cva.get_scores_ref(ScoreType::LowWickCount),
                ANALYSIS.zones.reversal,
                total_candles,
                "LOW WICKS"
            );

            // 2. High Wicks
            let (high_wicks, high_wicks_superzones) = process_layer(
                cva.get_scores_ref(ScoreType::HighWickCount),
                ANALYSIS.zones.reversal,
                total_candles,
                "HIGH WICKS"
            );

            // --- Calculate Coverage Statistics ---
            let calc_coverage = |zones: &[Zone]| -> f64 {
                if zone_count == 0 { return 0.0; }
                (zones.len() as f64 / zone_count as f64) * 100.0
            };

            let stats = ZoneCoverageStats {
                sticky_pct: calc_coverage(&sticky),
                support_pct: calc_coverage(&low_wicks),
                resistance_pct: calc_coverage(&high_wicks),
            };

            let classified = ClassifiedZones {
                sticky,
                low_wicks,
                high_wicks,
                sticky_superzones,
                low_wicks_superzones,
                high_wicks_superzones,
            };

            (classified, stats)
        })
    }

    /// Get nearest support superzone relative to a specific price
    pub fn nearest_support_superzone(&self, price: f64) -> Option<&SuperZone> {
        self.zones
            .sticky_superzones
            .iter()
            .filter(|sz| sz.price_center < price)
            .min_by(|a, b| {
                a.distance_to(price)
                    .partial_cmp(&b.distance_to(price))
                    .unwrap()
            })
    }

    /// Get nearest resistance superzone relative to a specific price
    pub fn nearest_resistance_superzone(&self, price: f64) -> Option<&SuperZone> {
        self.zones
            .sticky_superzones
            .iter()
            .filter(|sz| sz.price_center > price)
            .min_by(|a, b| {
                a.distance_to(price)
                    .partial_cmp(&b.distance_to(price))
                    .unwrap()
            })
    }

    /// Find all superzones containing the given price
    /// Returns a vec of (superzone_id, zone_type) tuples for all matching zones
    pub fn find_superzones_at_price(&self, price: f64) -> Vec<(usize, ZoneType)> {
        let mut zones = Vec::new();

        // Check sticky superzones
        for sz in &self.zones.sticky_superzones {
            if sz.contains(price) {
                // Determine if this specific sticky zone is acting as S or R
                let zone_type = if let Some(sup) = self.nearest_support_superzone(price) {
                    if sup.id == sz.id {
                        ZoneType::Support
                    } else {
                        ZoneType::Sticky
                    }
                } else if let Some(res) = self.nearest_resistance_superzone(price) {
                    if res.id == sz.id {
                        ZoneType::Resistance
                    } else {
                        ZoneType::Sticky
                    }
                } else {
                    ZoneType::Sticky
                };

                zones.push((sz.id, zone_type));
            }
        }

        // Check low wick superzones
        for sz in &self.zones.low_wicks_superzones {
            if sz.contains(price) {
                zones.push((sz.id, ZoneType::LowWicks));
            }
        }
        // Check low wick superzones
        for sz in &self.zones.high_wicks_superzones {
            if sz.contains(price) {
                zones.push((sz.id, ZoneType::HighWicks));
            }
        }

        zones
    }
}

/// Zone classification types for a given price level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneType {
    Sticky,     // High consolidation, price tends to stick here
    Support,    // Nearest sticky zone below current price
    Resistance, // Nearest sticky zone above current price
    LowWicks,   // High rejection activity below current price
    HighWicks,  // High rejection activity above current price
    Neutral,    // No special classification
}
