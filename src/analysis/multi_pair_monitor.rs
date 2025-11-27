use crate::models::pair_context::{PairContext, TradingSignal};
use std::collections::HashMap;

/// Multi-pair monitoring system for signal detection
/// Tracks all pairs and detects interesting trading signals
pub struct MultiPairMonitor {
    contexts: HashMap<String, PairContext>,
}

impl MultiPairMonitor {
    /// Create a new empty monitor
    pub fn new() -> Self {
        Self {
            contexts: HashMap::new(),
        }
    }

    /// Add a pair to monitoring
    pub fn add_pair(&mut self, mut context: PairContext) {
        context.initialize_signals();
        let pair_name = context.pair_name.clone();
        self.contexts.insert(pair_name, context);
    }

    /// Process a price update for a specific pair
    /// Only recalculates if price crossed a zone boundary
    pub fn process_price_update(&mut self, pair: &str, new_price: f64) -> bool {
        if let Some(context) = self.contexts.get_mut(pair) {
            // Check if update is needed (zone transition)
            if context.needs_update(new_price) {
                // #[cfg(debug_assertions)]
                // log::info!(
                //     "🔄 {}: Zone transition detected at price {:.2}",
                //     pair, new_price
                // );

                context.update(new_price);
                return true; // Update occurred
            }
        }
        false // No update needed
    }

    /// Get all pairs with signals
    pub fn get_signals(&self) -> Vec<&PairContext> {
        self.contexts
            .values()
            .filter(|ctx| ctx.has_signals())
            .collect()
    }

    /// Get context for a specific pair
    pub fn get_context(&self, pair: &str) -> Option<&PairContext> {
        self.contexts.get(pair)
    }

    /// Get all contexts
    pub fn get_all_contexts(&self) -> Vec<&PairContext> {
        self.contexts.values().collect()
    }

    /// Get count of monitored pairs
    pub fn pair_count(&self) -> usize {
        self.contexts.len()
    }

    /// Get summary of all signals across all pairs
    pub fn get_all_signals(&self) -> HashMap<String, Vec<TradingSignal>> {
        self.contexts
            .iter()
            .filter(|(_, ctx)| !ctx.signals.is_empty())
            .map(|(name, ctx)| (name.clone(), ctx.signals.clone()))
            .collect()
    }

    /// Get pairs grouped by current zone type
    pub fn pairs_by_zone_type(&self) -> HashMap<String, Vec<String>> {
        let mut grouped: HashMap<String, Vec<String>> = HashMap::new();

        for (pair_name, context) in &self.contexts {
            for (_, zone_type) in &context.current_zones {
                let type_name = format!("{:?}", zone_type);
                grouped
                    .entry(type_name)
                    .or_default()
                    .push(pair_name.clone());
            }
        }

        grouped
    }
}

impl Default for MultiPairMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    // use super::*;
    // use crate::models::CVACore;
    // use crate::models::trading_view::{TradingModel, ZoneType};
    // use std::sync::Arc;

    // Note: Full tests would require creating mock CVACore and TradingModel
    // This is a placeholder showing the test structure
}
