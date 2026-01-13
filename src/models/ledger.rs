// src/models/ledger.rs

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::models::trading_view::TradeOpportunity;

use crate::utils::maths_utils::calculate_percent_diff;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpportunityLedger {
    // Map UUID -> Opportunity
    pub opportunities: HashMap<String, TradeOpportunity>,
}

impl OpportunityLedger {

    pub fn new() -> Self {
        Self {
            opportunities: HashMap::new(),
        }
    }

    /// Intelligently updates the ledger.
    /// Returns: (IsNew, ActiveID)
    pub fn evolve(&mut self, new_opp: TradeOpportunity, tolerance_pct: f64) -> (bool, String) {
        // 1. Try Exact ID Match (Fast Path)
        let exact_id = new_opp.id.clone();
        if self.opportunities.contains_key(&exact_id) {
            self.update_existing(&exact_id, new_opp);
            return (false, exact_id);
        }

        // 2. Try Fuzzy Match (Nearest Neighbor)
        let closest_match = self.opportunities.values()
            .filter(|op| op.pair_name == new_opp.pair_name && op.direction == new_opp.direction)
            .map(|op| {
                let diff = calculate_percent_diff(op.target_price, new_opp.target_price);
                (op.id.clone(), diff)
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // 3. Evaluate Match using Configured Tolerance
        if let Some((id, diff)) = closest_match {
            if diff < tolerance_pct {
                 // LOGGING (Drift Detection)
                #[cfg(debug_assertions)]
                {
                    if id != new_opp.id {
                        log::info!("LEDGER FUZZY MATCH [{}]: New ID {} merged into Existing {}. Drift: {:.3}%", 
                            new_opp.pair_name, 
                            if new_opp.id.len() > 8 { &new_opp.id[..8] } else { &new_opp.id },
                            if id.len() > 8 { &id[..8] } else { &id },
                            diff
                        );
                    }
                }
                
                self.update_existing(&id, new_opp);
                return (false, id);
            }
        }

        // 4. No Match? It's a GENESIS (New Trade)
        let id = new_opp.id.clone();
        #[cfg(debug_assertions)]
        log::info!("LEDGER BIRTH [{}]: New Trade detected. ID: {} (Target: {:.2})", 
            new_opp.pair_name, 
            if id.len() > 8 { &id[..8] } else { &id },
            new_opp.target_price
        );
            
        self.opportunities.insert(id.clone(), new_opp);
        (true, id)
    }

    /// Prunes opportunities based on a predicate.
    /// Keeps the entry if the closure returns true, removes it if false.
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&String, &mut TradeOpportunity) -> bool,
    {
        self.opportunities.retain(f);
    }


    pub fn get_all(&self) -> Vec<&TradeOpportunity> {
        self.opportunities.values().collect()
    }
    
    pub fn remove(&mut self, id: &str) {
        self.opportunities.remove(id);
    }

    /// Helper to update an existing opportunity while preserving its history
    fn update_existing(&mut self, existing_id: &str, mut new_opp: TradeOpportunity) {
        if let Some(existing) = self.opportunities.get(existing_id) {
             // LOGGING ROI Change
            #[cfg(debug_assertions)]
            if (existing.expected_roi() - new_opp.expected_roi()).abs() > 0.1 {
                log::info!("LEDGER EVOLVE [{}]: ID {} kept. ROI {:.2}% -> {:.2}% | SL: {:.2} -> {:.2}", 
                    new_opp.pair_name, 
                    if existing_id.len() > 8 { &existing_id[..8] } else { existing_id }, 
                    existing.expected_roi(), 
                    new_opp.expected_roi(),
                    existing.stop_price,
                    new_opp.stop_price
                );
            }

            // CRITICAL: Preserve Identity
            new_opp.id = existing.id.clone();         // Keep the OLD ID (so UI selection sticks)
            new_opp.created_at = existing.created_at; // Keep Birth Time (Age)
            
            // Insert (Overwrite)
            self.opportunities.insert(existing_id.to_string(), new_opp);
        }
    }



}