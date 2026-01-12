// src/models/ledger.rs

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::models::trading_view::TradeOpportunity;

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
        // TRUST THE ID. 
        // The Engine Worker generates deterministic IDs (e.g. based on Grid Index).
        // If the ID matches, it is the same trade evolving.
        // If the ID is different, the Engine has determined it is a distinct trade.
        let id = new_opp.id.clone();

        if let Some(existing) = self.opportunities.get(&id) {
            // CASE A: EVOLUTION (Update In-Place)
            
            // LOGGING
            #[cfg(debug_assertions)]
            if (existing.expected_roi() - new_opp.expected_roi()).abs() > 0.1 {
                log::info!("LEDGER EVOLVE [{}]: ID {} kept. ROI {:.2}% -> {:.2}%", 
                    new_opp.pair_name, 
                    // Use a short ID for cleaner logs
                    if id.len() > 8 { &id[..8] } else { &id }, 
                    existing.expected_roi(), 
                    new_opp.expected_roi()
                );
            }

            // Preserve creation time (Birth Time)
            new_opp.created_at = existing.created_at; 
            
            self.opportunities.insert(id.clone(), new_opp);
            (false, id)
        } else {
            // CASE B: GENESIS (New Trade)
            #[cfg(debug_assertions)]
            log::info!("LEDGER BIRTH [{}]: New Trade detected. ID: {} (Target: {:.2})", 
                new_opp.pair_name, 
                if id.len() > 8 { &id[..8] } else { &id },
                new_opp.target_price
            );
                
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