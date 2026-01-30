// src/models/ledger.rs

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;

#[cfg(debug_assertions)]
use crate::config::DF;

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
        let closest_match = self
            .opportunities
            .values()
            .filter(|op| op.pair_name == new_opp.pair_name && op.direction == new_opp.direction)
            .map(|op| {
                let diff = calculate_percent_diff(op.target_price, new_opp.target_price);
                (op.id.clone(), diff)
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        // 3. Evaluate Match using Configured Tolerance
        if let Some((id, diff)) = closest_match {
            if diff < tolerance_pct {
                // LOGGING (Drift Detection)
                #[cfg(debug_assertions)]
                {
                    if DF.log_ledger {
                        if id != new_opp.id {
                            log::info!(
                                "LEDGER FUZZY MATCH [{}]: New ID {} merged into Existing {}. Drift: {:.3}%",
                                new_opp.pair_name,
                                if new_opp.id.len() > 8 {
                                    &new_opp.id[..8]
                                } else {
                                    &new_opp.id
                                },
                                if id.len() > 8 { &id[..8] } else { &id },
                                diff
                            );
                        }
                    }
                }

                self.update_existing(&id, new_opp);
                return (false, id);
            }
        }

        // 4. No Match? It's a GENESIS (New Trade)
        let id = new_opp.id.clone();
        #[cfg(debug_assertions)]
        if DF.log_ledger {
            log::info!(
                "LEDGER BIRTH [{}]: New Trade detected. ID: {} (Target: {:.2})",
                new_opp.pair_name,
                if id.len() > 8 { &id[..8] } else { &id },
                new_opp.target_price
            );
        }
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

    pub fn find_first_for_pair(&self, pair_name: Option<String>) -> Option<&TradeOpportunity> {
        let name = pair_name?;
        self.opportunities
            .values()
            .find(|op| op.pair_name == name)
    }

    pub fn remove(&mut self, id: &str) {
        self.opportunities.remove(id);
    }

    /// Scans for overlapping trades.
    /// POLICY: Strategy Segregation.
    /// 1. Trades from different strategies (e.g. ROI vs AROI) NEVER merge. They coexist.
    /// 2. Trades from the SAME strategy with overlapping targets are merged.
    /// 3. The winner is decided by the Score of that specific strategy.
    pub fn prune_collisions(&mut self, tolerance_pct: f64) {
        let mut to_remove = Vec::new();
        let ops: Vec<_> = self.opportunities.values().cloned().collect();

        for i in 0..ops.len() {
            for j in (i + 1)..ops.len() {
                let a = &ops[i];
                let b = &ops[j];

                if to_remove.contains(&a.id) || to_remove.contains(&b.id) {
                    continue;
                }

                if a.pair_name == b.pair_name && a.direction == b.direction {
                    // SEGREGATION: Different strategies = Different trades.
                    if a.strategy != b.strategy {
                        continue;
                    }
                    if a.station_id != b.station_id {
                        continue;
                    }

                    // Same strategy and stationId so preserve the best one
                    let diff = calculate_percent_diff(a.target_price, b.target_price);

                    if diff < tolerance_pct {
                        let score_a = a.calculate_quality_score();
                        let score_b = b.calculate_quality_score();
                        let (_winner, loser) = if score_a >= score_b { (a, b) } else { (b, a) };

                        #[cfg(debug_assertions)]
                        if DF.log_ledger {
                            log::info!(
                                "🧹 LEDGER PRUNE [Strategy: {}]: Merging duplicate trade {} into {}. (Diff {:.3}%)",
                                a.strategy,
                                if loser.id.len() > 8 {
                                    &loser.id[..8]
                                } else {
                                    &loser.id
                                },
                                if _winner.id.len() > 8 {
                                    &_winner.id[..8]
                                } else {
                                    &_winner.id
                                },
                                diff
                            );
                        }
                        to_remove.push(loser.id.clone());
                    }
                }
            }
        }

        for id in to_remove {
            self.opportunities.remove(&id);
        }
    }

    /// Helper to update an existing opportunity while preserving its history
    fn update_existing(&mut self, existing_id: &str, mut new_opp: TradeOpportunity) {
        if let Some(existing) = self.opportunities.get(existing_id) {
            // LOGGING  EVOLVE
            #[cfg(debug_assertions)]
            if DF.log_ledger {
                if (*existing.expected_roi() - *new_opp.expected_roi()).abs() > 0.1 {
                    log::info!(
                        "LEDGER EVOLVE [{}]: ID {} kept. Target: {:.2} -> {:.2} | ROI {} -> {} (Win: {}->{}) | SL: {:.2} -> {:.2}",
                        new_opp.pair_name,
                        if existing_id.len() > 8 {
                            &existing_id[..8]
                        } else {
                            existing_id
                        },
                        existing.target_price,
                        new_opp.target_price,
                        existing.expected_roi(),
                        new_opp.expected_roi(),
                        existing.simulation.success_rate,
                        new_opp.simulation.success_rate,
                        existing.stop_price,
                        new_opp.stop_price
                    );
                }
            }

            // CRITICAL: Preserve Identity
            new_opp.id = existing.id.clone(); // Keep the OLD ID (so UI selection sticks)
            new_opp.created_at = existing.created_at; // Keep Birth Time (Age)

            // Insert (Overwrite)
            self.opportunities.insert(existing_id.to_string(), new_opp);
        }
    }
}
