// src/models/ledger.rs

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;

#[cfg(debug_assertions)]
use crate::config::{DF, OptimizationStrategy};

use crate::config::{Pct, PriceLike};

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

    #[cfg(debug_assertions)]
    /// DEBUG: Summarizes how many TradeOpportunities exist per OptimizationStrategy.
    /// This inspects the ledger directly (ground truth), not the UI.
    pub fn debug_log_strategy_summary(&self) {
        if !DF.log_ledger {
            return;
        }

        use std::collections::BTreeMap;

        let ops: Vec<_> = self.opportunities.values().cloned().collect();

        if ops.is_empty() {
            log::info!("📒 LEDGER STRATEGY SUMMARY: Ledger is empty");
            return;
        }

        let mut counts: BTreeMap<OptimizationStrategy, usize> = BTreeMap::new();

        for op in &ops {
            *counts.entry(op.strategy).or_insert(0) += 1;
        }

        log::info!(
            "📒 LEDGER STRATEGY SUMMARY: {} total opportunities",
            ops.len()
        );

        for (strategy, count) in counts {
            log::info!("   • {:?}: {} ops", strategy, count);
        }

        // Optional: detailed per-op trace
        log::info!("📒 LEDGER STRATEGY DETAILS:");
        for op in ops {
            if op.pair_name == "PEPEUSDT" {
                log::info!(
                    "   [{}] {} {:?} @ {}",
                    op.id,
                    op.pair_name,
                    op.strategy,
                    op.target_price
                );
            }
        }
    }

    /// Intelligently updates the ledger.
    /// Returns: (IsNew, ActiveID)
    pub fn evolve(&mut self, new_opp: TradeOpportunity, tolerance_pct: Pct) -> (bool, String) {
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
            .filter(|op| op.is_comparable_to(&new_opp))
            .map(|op| {
                let pct_diff =
                    Pct::new(op.target_price.percent_diff_from_0_1(&new_opp.target_price));
                (op.id.clone(), pct_diff)
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        // 3. Evaluate Match using Configured Tolerance
        if let Some((id, diff_pct)) = closest_match {
            if diff_pct < tolerance_pct {
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
                                diff_pct
                            );
                        }
                    }
                    self.debug_log_strategy_summary();
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
        self.opportunities.values().find(|op| op.pair_name == name)
    }

    pub fn remove(&mut self, id: &str) {
        self.opportunities.remove(id);
    }

    /// Scans for overlapping trades and resolves collisions.
    ///
    /// POLICY: Comparable-Trade Collision Resolution.
    ///
    /// Only *comparable* trades are ever considered for merging.
    /// Two trades are comparable if and only if they share:
    ///   - the same trading pair (`pair_name`)
    ///   - the same direction (long / short)
    ///   - the same optimization strategy
    ///   - the same station (`station_id`)
    ///
    /// Collision Rules:
    /// 1. Trades from different strategies NEVER merge; they always coexist.
    /// 2. Trades from the same strategy *and* comparable context with overlapping
    ///    target prices (within tolerance) are considered colliding.
    /// 3. For colliding trades, the winner is selected using the quality score
    ///    defined by that specific strategy.
    /// 4. Non-winning trades are removed from the ledger; winners are preserved.
    pub fn prune_collisions(&mut self, tolerance_pct: Pct) {
        let mut to_remove: Vec<String> = Vec::new();

        // Snapshot for stable comparison
        let ops: Vec<_> = self.opportunities.values().cloned().collect();

        for i in 0..ops.len() {
            for j in (i + 1)..ops.len() {
                let a = &ops[i];
                let b = &ops[j];

                // Skip if either already marked for removal
                if to_remove.contains(&a.id) || to_remove.contains(&b.id) {
                    continue;
                }

                // 🔐 COMPARABILITY GATE
                // Non-comparable opportunities must never collide
                if !a.is_comparable_to(b) {
                    continue;
                }

                // Compute target drift
                let pct_diff = Pct::new(a.target_price.percent_diff_from_0_1(&b.target_price));

                #[cfg(debug_assertions)]
                if DF.log_ledger {
                    log::info!(
                        "LEDGER COLLISION CHECK [{} | {}]: drift={} tolerance={}",
                        a.strategy,
                        a.pair_name,
                        pct_diff,
                        tolerance_pct
                    );
                }

                // Only collide if within tolerance
                if pct_diff >= tolerance_pct {
                    continue;
                }

                // 🧨 DESTRUCTIVE DECISION BOUNDARY
                #[cfg(debug_assertions)]
                TradeOpportunity::assert_comparable_to(a, b);

                let score_a = a.calculate_quality_score();
                let score_b = b.calculate_quality_score();
                let (_winner, loser) = if score_a >= score_b { (a, b) } else { (b, a) };
                #[cfg(debug_assertions)]
                {
                    if DF.log_ledger {
                        log::info!(
                            "🧹 LEDGER PRUNE [Strategy: {} | Station: {:?}]: {} removed in favor of {} (Δ={})",
                            _winner.strategy,
                            _winner.station_id,
                            &loser.id[..loser.id.len().min(8)],
                            &_winner.id[.._winner.id.len().min(8)],
                            pct_diff
                        );
                        self.debug_log_strategy_summary();
                    }
                }

                to_remove.push(loser.id.clone());
            }
        }

        // Apply removals to the real ledger
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
                if (existing.expected_roi().value() - new_opp.expected_roi().value()).abs() > 0.1 {
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
                self.debug_log_strategy_summary();
            }

            // CRITICAL: Preserve Identity
            new_opp.id = existing.id.clone(); // Keep the OLD ID (so UI selection sticks)
            new_opp.created_at = existing.created_at; // Keep Birth Time (Age)

            // Insert (Overwrite)
            self.opportunities.insert(existing_id.to_string(), new_opp);
        }
    }
}
