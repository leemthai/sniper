// src/models/ledger.rs

use std::collections::HashMap;
use crate::models::trading_view::TradeOpportunity;

#[derive(Debug, Clone, Default)]
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

    /// Insert or Update an opportunity.
    /// Returns true if it was a new entry.
    pub fn upsert(&mut self, opp: TradeOpportunity) -> bool {
        // In the future, we can add logic here:
        // "If existing opp has better ROI, don't overwrite?" 
        // For now, newest calculation wins.
        self.opportunities.insert(opp.id.clone(), opp).is_none()
    }

    pub fn get_all(&self) -> Vec<&TradeOpportunity> {
        self.opportunities.values().collect()
    }
    
    pub fn remove(&mut self, id: &str) {
        self.opportunities.remove(id);
    }
}