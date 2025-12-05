use std::sync::Arc;

use crate::analysis::zone_scoring::find_target_zones;
use crate::models::cva::{CVACore, ScoreType};
use crate::utils::maths_utils::{normalize_max, smooth_data};

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

    /// Check if a price is within this zone
    #[allow(dead_code)] // Useful for trading logic
    pub fn contains(&self, price: f64) -> bool {
        price >= self.price_bottom && price <= self.price_top
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
#[allow(dead_code)] // Will be used for trading strategies
pub struct TradingModel {
    pub pair_name: String,
    pub cva: Arc<CVACore>,
    pub zones: ClassifiedZones,
    pub current_price: Option<f64>,
}

impl TradingModel {
    /// Create a new trading model from CVA results and optional current price
    pub fn from_cva(cva: Arc<CVACore>, current_price: Option<f64>) -> Self {
        let zones = Self::classify_zones(&cva);

        Self {
            pair_name: cva.pair_name.clone(),
            cva,
            zones,
            current_price,
        }
    }

    fn classify_zones(cva: &CVACore) -> ClassifiedZones {
        let (price_min, price_max) = cva.price_range.min_max();
        let zone_count = cva.zone_count;
        let total_candles = cva.total_candles as f64;

        // Helper closure with Custom Normalization Logic
        // divisor: If Some(val), divide by val. If None, divide by Max(smoothed).
        let process_layer = |raw_data: &[f64],
                             smooth_pct: f64,
                             gap_pct: f64,
                             threshold: f64,
                             divisor: Option<f64>| {
            // 1. Smooth
            let smooth_window = ((zone_count as f64 * smooth_pct).ceil() as usize).max(1) | 1;
            let smoothed = smooth_data(raw_data, smooth_window);

            // 2. Normalize (Global vs Max)
            let normalized: Vec<f64> = if let Some(div) = divisor {
                // Global Normalization (Absolute %)
                // If div is 0 (no candles), result is 0.
                if div > 0.0 {
                    smoothed.iter().map(|&s| s / div).collect()
                } else {
                    vec![0.0; smoothed.len()]
                }
            } else {
                // Standard Max Normalization (Relative)
                normalize_max(&smoothed)
            };

            // 3. Contrast (Square)
            let sharpened: Vec<f64> = normalized.iter().map(|&s| s * s).collect();

            // 4. Find Targets
            let gap = (zone_count as f64 * gap_pct).ceil() as usize;
            let targets = find_target_zones(&sharpened, threshold, gap);

            // ... (Explode / Aggregate remains same) ...
            let zones: Vec<Zone> = targets
                .iter()
                .flat_map(|t| t.start_idx..=t.end_idx)
                .map(|idx| Zone::new(idx, price_min, price_max, zone_count))
                .collect();
            let superzones = aggregate_zones(&zones);

            (zones, superzones)
        };

        // --- Sticky Zones (Max Normalized) ---
        // Divisor: None (Relative to Peak)
        // Threshold: 0.16 (Squared) -> ~0.40 Raw
        let (sticky, sticky_superzones) = process_layer(
            cva.get_scores_ref(ScoreType::FullCandleTVW),
            0.02,
            0.02,
            0.16,
            None,
        );

        // --- REVERSAL ZONES (Global / Absolute) ---

        //let reversal_threshold = 0.0004; // 2% Wick Density target (produces very very few zones if any)
        // let reversal_threshold = 0.0001; // 1% wick density (not enough zones)
        //let reversal_threshold = 0.000025; // This is 0.5% Wick Density target (produces not many zones)
        let reversal_threshold = 0.00001; // About 0.3% wick density 
        // let reversal_threshold: f64 = 0.000004; // Requirees 0.2% of all candles to wick here (e.g. 2 wicks per 1000 candles)

        // 2. Define Dynamic Gap for Wicks
        // CHANGE: Set to 0.0. 
        // This means if there is ANY empty space between wicks, they become separate zones.
        // No "Ghost Zones" created by bridging.

        // 1. Low Wicks
        let (low_wicks, low_wicks_superzones) = process_layer(
            cva.get_scores_ref(ScoreType::LowWickCount),
            0.005,
            0.0, // Gap Pct (0.0%) <--- HERE. 0.0 means "Strict Separation"
            reversal_threshold,
            Some(total_candles),
        );

        // 2. High Wicks
        let (high_wicks, high_wicks_superzones) = process_layer(
            cva.get_scores_ref(ScoreType::HighWickCount),
            0.005,
            0.0, // Gap Pct (0.0%) <--- HERE. 0.0 means "Strict Separation"
            reversal_threshold,
            Some(total_candles),
        );

        ClassifiedZones {
            sticky,
            low_wicks,
            high_wicks,
            sticky_superzones,
            low_wicks_superzones,
            high_wicks_superzones,
        }
    }

    /// Update the model with a new current price (recalculates S/R)
    pub fn update_price(&mut self, new_price: f64) {
        self.current_price = Some(new_price);
    }

    /// Get all sticky zones (for potential S/R candidates)
    #[allow(dead_code)] // For trading strategies
    pub fn sticky_zones(&self) -> &[Zone] {
        &self.zones.sticky
    }

    /// Get nearest support superzone
    pub fn nearest_support_superzone(&self) -> Option<&SuperZone> {
        let price = self.current_price?;
        // Find sticky zone below price with minimum distance
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

    /// Get nearest resistance superzone
    pub fn nearest_resistance_superzone(&self) -> Option<&SuperZone> {
        let price = self.current_price?;
        // Find sticky zone above price with minimum distance
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
                let zone_type = if let Some(sup) = self.nearest_support_superzone() {
                    if sup.id == sz.id {
                        ZoneType::Support
                    } else {
                        ZoneType::Sticky
                    }
                } else if let Some(res) = self.nearest_resistance_superzone() {
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
