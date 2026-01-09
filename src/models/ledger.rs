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
    pub fn evolve(&mut self, mut new_opp: TradeOpportunity) -> (bool, String) {
        // 1. Search for a fuzzy match (Same Pair, Direction, Similar Target)
        let match_id = self.opportunities.values().find_map(|existing| {
            if existing.pair_name != new_opp.pair_name { return None; }
            if existing.direction != new_opp.direction { return None; }
            
            // Fuzzy Target Match (Tolerance = 20% of PH)
            let tolerance = new_opp.source_ph * 20.0; 
            let diff = calculate_percent_diff(existing.target_price, new_opp.target_price);
            
            if diff < tolerance {
                Some(existing.id.clone())
            } else {
                None
            }
        });

        if let Some(id) = match_id {
            // CASE A: EVOLUTION (Update In-Place)
            if let Some(existing) = self.opportunities.get(&id) {
                // LOGGING
                #[cfg(debug_assertions)]
                if (existing.expected_roi() - new_opp.expected_roi()).abs() > 0.1 {
                    // Only log if stats changed significantly to avoid spam
                    log::info!("LEDGER EVOLVE [{}]: ID {} kept. ROI {:.2}% -> {:.2}%", 
                        new_opp.pair_name, id, existing.expected_roi(), new_opp.expected_roi());
                }

                new_opp.id = existing.id.clone();         // Keep UUID
                new_opp.created_at = existing.created_at; // Keep Birth Time
            }
            
            self.opportunities.insert(id.clone(), new_opp);
            (false, id)
        } else {
            // CASE B: GENESIS (New Trade)
            let id = new_opp.id.clone();
            
            #[cfg(debug_assertions)]
            log::info!("LEDGER BIRTH [{}]: New Trade detected. ID: {} (Target: {:.2})", 
                new_opp.pair_name, id, new_opp.target_price);
                
            self.opportunities.insert(id.clone(), new_opp);
            (true, id)
        }
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



}