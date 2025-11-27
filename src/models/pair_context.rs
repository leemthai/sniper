use crate::models::trading_view::{TradingModel, ZoneType};
// #[cfg(not(target_arch = "wasm32"))]
// use std::time::Instant;
use crate::utils::app_time::{AppInstant, now};

/// Context and state for a single trading pair
/// Tracks current price, superzone position, and trading signals
#[derive(Debug, Clone)]
pub struct PairContext {
    pub pair_name: String,
    pub current_price: f64,
    pub current_zones: Vec<(usize, ZoneType)>,
    pub trading_model: TradingModel,
    // #[cfg(not(target_arch = "wasm32"))]
    pub last_updated: AppInstant,
    pub signals: Vec<TradingSignal>,
}

impl PairContext {
    /// Create a new context for a pair with initial price
    pub fn new(trading_model: TradingModel, initial_price: f64) -> Self {
        let current_zones = trading_model.find_superzones_at_price(initial_price);

        Self {
            pair_name: trading_model.pair_name.clone(),
            current_price: initial_price,
            current_zones,
            trading_model,
            // #[cfg(not(target_arch = "wasm32"))]
            last_updated: now(),
            signals: Vec::new(),
        }
    }

    /// Initialize signals for the starting state (call after creation for initial signals)
    /// We need this because new signals are normally only generated when needs_update() is true, and this only happens when the price moves into a different superzone
    pub fn initialize_signals(&mut self) {
        self.signals.clear(); // Ensure clean slate, though new() already does this
        self.generate_state_signals();
    }

    /// Check if this context needs updating based on new price
    /// Returns true if price has crossed into a different superzone
    pub fn needs_update(&self, new_price: f64) -> bool {
        // Update needed if superzone changed or we don't know current superzone
        self.trading_model.find_superzones_at_price(new_price) != self.current_zones
    }

    /// Update context with new price and regenerate signals
    pub fn update(&mut self, new_price: f64) {
        // let old_zones = self.current_zones.clone();
        self.current_price = new_price;
        let new_zones = self.trading_model.find_superzones_at_price(new_price);
        self.current_zones = new_zones.clone(); // Clone for passing; could move if not needed elsewhere
        // #[cfg(not(target_arch = "wasm32"))]
        {
            self.last_updated = now();
        }
        // Clear old signals
        self.signals.clear();
        // Generate current state signals (for all zones the price is now in)
        self.generate_state_signals();
    }

    fn generate_state_signals(&mut self) {
        // Existing proximity logic stays the same

        // Generate zone signals for all current zones the price exists in.
        for (superzone_id, zone_type) in &self.current_zones {
            if let ZoneType::Sticky = zone_type {
                self.signals.push(TradingSignal::InStickyZone {
                    superzone_id: *superzone_id,
                });
            }
        }
    }

    /// Check if this pair has signals of interest
    pub fn has_signals(&self) -> bool {
        self.signals.iter().any(|s| s.is_signal())
    }
}

/// Trading signals generated from currently, just either a price being in a sticky zone or having entered a sticky zone (why do we need 2?)
#[derive(Debug, Clone)]
pub enum TradingSignal {
    InStickyZone { superzone_id: usize },
}

impl TradingSignal {
    /// Returns true if this signal is of interest
    // Very bodgy code - just sees if known signal
    pub fn is_signal(&self) -> bool {
        matches!(
            self,
            TradingSignal::InStickyZone { .. } //
        )
    }

    /// Get a human-readable description of this signal
    pub fn description(&self) -> String {
        match self {
            TradingSignal::InStickyZone { superzone_id } => {
                format!("🔒 In sticky superzone {}", superzone_id)
            }
        }
    }
}
