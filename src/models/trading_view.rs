use std::sync::Arc;

use crate::analysis::zone_scoring::find_high_activity_zones;
use crate::analysis::zone_scoring::find_target_zones;
use crate::models::cva::{CVACore, ScoreType};
use crate::utils::maths_utils;

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
    // Note: support/resistance superzones are dynamically calculated from sticky superzones
    pub support_superzones: Vec<SuperZone>,
    pub resistance_superzones: Vec<SuperZone>,
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
        let zones = Self::classify_zones(&cva, current_price);

        Self {
            pair_name: cva.pair_name.clone(),
            cva,
            zones,
            current_price,
        }
    }

    /// Classify zones based on CVA results and current price
    fn classify_zones(cva: &CVACore, current_price: Option<f64>) -> ClassifiedZones {
        let (price_min, price_max) = cva.price_range.min_max();
        let zone_count = cva.zone_count;

        let high_wicks_data = maths_utils::normalize_max(cva.get_scores_ref(ScoreType::HighWickVW));
        let low_wicks_data = maths_utils::normalize_max(cva.get_scores_ref(ScoreType::LowWickVW));

        // 1. Get Normalized Data (0.0 to 1.0)
        let raw_sticky = maths_utils::normalize_max(cva.get_scores_ref(ScoreType::FullCandleTVW));

        // 2. Apply Contrast (Sharpening)
        // This pushes "mushy" valleys down, creating separation for the island logic.
        let sharpened_sticky: Vec<f64> = raw_sticky.iter().map(|&s| s * s).collect();

        // 3. Define Dynamic Gap (Scale Independent) - the bigger the gap, the wider the bridget.
        // 2% of the map width. (100 zones -> gap 2. 1000 zones -> gap 20).
        // This answers your concern about scale independence.
        let gap_percent = 0.015; // # A value 0.01 to 0.02 seems reasonable here. Not tried yet with a bigger zone count.
        let calculated_gap = (zone_count as f64 * gap_percent).ceil() as usize;

        // 4. Find Targets
        // Use a LOWER threshold on squared data.
        // 0.1 squared threshold ~= 0.31 original threshold.
        // BUT, a valley of 0.4 (original) -> 0.16 (squared), which is still above 0.1.
        // You might want to try 0.15 or 0.20 here to really break up the super-islands.
        let sticky_targets = find_target_zones(&sharpened_sticky, 0.16, calculated_gap);

        // 5. Explode back to indices for the rest of your app
        let mut sticky_indices = Vec::new();
        for target in sticky_targets {
            for i in target.start_idx..=target.end_idx {
                sticky_indices.push(i);
            }
        }

        let high_wicks_indices = find_high_activity_zones(&high_wicks_data, 0.75);
        let low_wicks_indices = find_high_activity_zones(&low_wicks_data, 0.75);

        // Convert indices to Zone objects
        let sticky_zones: Vec<Zone> = sticky_indices
            .iter()
            .map(|&idx| Zone::new(idx, price_min, price_max, zone_count))
            .collect();

        let low_wicks_zones: Vec<Zone> = low_wicks_indices
            .iter()
            .map(|&idx| Zone::new(idx, price_min, price_max, zone_count))
            .collect();

        let high_wicks_zones: Vec<Zone> = high_wicks_indices
            .iter()
            .map(|&idx| Zone::new(idx, price_min, price_max, zone_count))
            .collect();

        // Aggregate contiguous zones into SuperZones
        let sticky_superzones = aggregate_zones(&sticky_zones);
        let high_wicks_superzones = aggregate_zones(&high_wicks_zones);
        let low_wicks_superzones = aggregate_zones(&low_wicks_zones);

        // Find support/resistance superzones based on current price
        let (support_superzones, resistance_superzones) = if let Some(price) = current_price {
            Self::find_support_resistance_superzones(&sticky_superzones, price)
        } else {
            (Vec::new(), Vec::new())
        };

        ClassifiedZones {
            sticky: sticky_zones,
            low_wicks: low_wicks_zones,
            high_wicks: high_wicks_zones,
            sticky_superzones,
            support_superzones,
            resistance_superzones,
            low_wicks_superzones,
            high_wicks_superzones,
        }
    }

    /// Find nearest sticky superzones above (resistance) and below (support) current price
    fn find_support_resistance_superzones(
        sticky_superzones: &[SuperZone],
        current_price: f64,
    ) -> (Vec<SuperZone>, Vec<SuperZone>) {
        let mut support_superzone = None;
        let mut resistance_superzone = None;
        let mut support_dist = f64::INFINITY;
        let mut resistance_dist = f64::INFINITY;

        for superzone in sticky_superzones {
            if superzone.price_center < current_price {
                // Below current price - potential support
                let dist = superzone.distance_to(current_price);
                if dist < support_dist {
                    support_dist = dist;
                    support_superzone = Some(superzone.clone());
                }
            } else if superzone.price_center > current_price {
                // Above current price - potential resistance
                let dist = superzone.distance_to(current_price);
                if dist < resistance_dist {
                    resistance_dist = dist;
                    resistance_superzone = Some(superzone.clone());
                }
            }
        }

        (
            support_superzone.into_iter().collect(),
            resistance_superzone.into_iter().collect(),
        )
    }

    /// Update the model with a new current price (recalculates S/R)
    pub fn update_price(&mut self, new_price: f64) {
        self.current_price = Some(new_price);
        let (support_superzones, resistance_superzones) =
            Self::find_support_resistance_superzones(&self.zones.sticky_superzones, new_price);
        self.zones.support_superzones = support_superzones;
        self.zones.resistance_superzones = resistance_superzones;
    }

    /// Get all sticky zones (for potential S/R candidates)
    #[allow(dead_code)] // For trading strategies
    pub fn sticky_zones(&self) -> &[Zone] {
        &self.zones.sticky
    }

    /// Get nearest support superzone
    pub fn nearest_support_superzone(&self) -> Option<&SuperZone> {
        self.zones.support_superzones.first()
    }

    /// Get nearest resistance superzone
    pub fn nearest_resistance_superzone(&self) -> Option<&SuperZone> {
        self.zones.resistance_superzones.first()
    }

    /// Find all superzones containing the given price
    /// Returns a vec of (superzone_id, zone_type) tuples for all matching zones
    pub fn find_superzones_at_price(&self, price: f64) -> Vec<(usize, ZoneType)> {
        let mut zones = Vec::new();

        // Check support superzones
        for sz in &self.zones.support_superzones {
            if sz.contains(price) {
                zones.push((sz.id, ZoneType::Support));
            }
        }
        // Check resistance superzones
        for sz in &self.zones.resistance_superzones {
            if sz.contains(price) {
                zones.push((sz.id, ZoneType::Resistance));
            }
        }
        // Check sticky superzones
        for sz in &self.zones.sticky_superzones {
            if sz.contains(price) {
                zones.push((sz.id, ZoneType::Sticky));
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
