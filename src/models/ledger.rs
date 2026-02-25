use {
    crate::{
        config::{Pct, PriceLike},
        models::TradeOpportunity,
    },
    serde::{Deserialize, Serialize},
    std::{
        cmp::Ordering,
        collections::{HashMap, HashSet},
    },
};

#[cfg(debug_assertions)]
use {
    crate::config::{DF, OptimizationStrategy},
    std::collections::BTreeMap,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::data::load_ledger;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct OpportunityLedger {
    pub opportunities: HashMap<String, TradeOpportunity>,
}

impl OpportunityLedger {
    pub(crate) fn new() -> Self {
        Self {
            opportunities: HashMap::new(),
        }
    }

    #[cfg(debug_assertions)]
    pub(crate) fn debug_log_strategy_summary(&self) {
        if !DF.log_ledger {
            return;
        }

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

    /// Updates ledger with new opportunity using exact match or fuzzy matching within tolerance.
    /// Returns (is_new, active_id).
    pub(crate) fn evolve(
        &mut self,
        new_opp: TradeOpportunity,
        tolerance_pct: Pct,
    ) -> (bool, String) {
        let exact_id = new_opp.id.clone();
        if self.opportunities.contains_key(&exact_id) {
            self.update_existing(&exact_id, new_opp);
            return (false, exact_id);
        }

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

        if let Some((id, diff_pct)) = closest_match {
            if diff_pct < tolerance_pct {
                #[cfg(debug_assertions)]
                {
                    if DF.log_ledger && id != new_opp.id {
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
                    self.debug_log_strategy_summary();
                }

                self.update_existing(&id, new_opp);
                return (false, id);
            }
        }

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

    pub(crate) fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&String, &mut TradeOpportunity) -> bool,
    {
        self.opportunities.retain(f);
    }

    pub(crate) fn get_all(&self) -> Vec<&TradeOpportunity> {
        self.opportunities.values().collect()
    }

    pub(crate) fn remove_from_ledger(&mut self, id: &str) {
        self.opportunities.remove(id);
    }

    /// Resolves collisions between comparable trades (same pair/direction/strategy/station).
    /// Keeps higher quality trade, removes lower. Returns list of pruned IDs.
    pub(crate) fn prune_collisions(&mut self, tolerance_pct: Pct) -> Vec<String> {
        let mut to_remove: Vec<String> = Vec::new();
        let ops: Vec<_> = self.opportunities.values().cloned().collect();

        for i in 0..ops.len() {
            for j in (i + 1)..ops.len() {
                let a = &ops[i];
                let b = &ops[j];

                if to_remove.contains(&a.id) || to_remove.contains(&b.id) {
                    continue;
                }

                if !a.is_comparable_to(b) {
                    continue;
                }

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

                if pct_diff >= tolerance_pct {
                    continue;
                }

                #[cfg(debug_assertions)]
                TradeOpportunity::assert_comparable_to(a, b);

                let score_a = a.calculate_quality_score();
                let score_b = b.calculate_quality_score();
                let (_winner, loser) = if score_a >= score_b { (a, b) } else { (b, a) };

                #[cfg(debug_assertions)]
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

                to_remove.push(loser.id.clone());
            }
        }

        for id in to_remove.clone() {
            #[cfg(debug_assertions)]
            if DF.log_ledger {
                log::info!(
                    "LEDGER PRUNE PART II: Removing opportunity id {} from ledger",
                    id
                );
            }
            self.remove_from_ledger(&id);
        }
        to_remove
    }

    fn update_existing(&mut self, existing_id: &str, mut new_opp: TradeOpportunity) {
        if let Some(existing) = self.opportunities.get(existing_id) {
            #[cfg(debug_assertions)]
            if DF.log_ledger
                && (existing.expected_roi().value() - new_opp.expected_roi().value()).abs() > 0.1
            {
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
                self.debug_log_strategy_summary();
            }

            new_opp.id = existing.id.clone();
            new_opp.created_at = existing.created_at;
            self.opportunities.insert(existing_id.to_string(), new_opp);
        }
    }
}

pub(crate) fn restore_engine_ledger(valid_session_pairs: &HashSet<String>) -> OpportunityLedger {
    #[cfg(debug_assertions)]
    if DF.wipe_ledger_on_startup {
        log::info!("☢️ LEDGER NUKE: Wiping all historical trades from persistence.");
        return OpportunityLedger::new();
    }

    let mut ledger = {
        #[cfg(not(target_arch = "wasm32"))]
        {
            match load_ledger() {
                Ok(l) => {
                    #[cfg(debug_assertions)]
                    if DF.log_ledger {
                        log::info!("Loaded ledger with {} opportunities", l.opportunities.len());
                    }
                    l
                }
                Err(_e) => {
                    #[cfg(debug_assertions)]
                    log::error!("Failed to load ledger (starting fresh): {}", _e);
                    OpportunityLedger::new()
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            OpportunityLedger::new()
        }
    };

    let _count_before = ledger.opportunities.len();

    #[cfg(debug_assertions)]
    if DF.log_ledger {
        log::info!("The valid start-up set is {:?}", valid_session_pairs);
    }

    ledger.retain(|_id, op| valid_session_pairs.contains(&op.pair_name));

    #[cfg(debug_assertions)]
    {
        if DF.log_ledger {
            for op in ledger.opportunities.values() {
                debug_assert!(
                    valid_session_pairs.contains(&op.pair_name),
                    "Ledger contains invalid pair AFTER retain: {}",
                    op.pair_name
                );
            }
        }

        let count_after = ledger.opportunities.len();
        if _count_before != count_after && DF.log_ledger {
            log::info!(
                "START-UP CLEANUP: Culled {} orphan trades (Data not loaded).",
                _count_before - count_after
            );
        }
    }

    ledger
}
