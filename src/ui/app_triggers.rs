use crate::utils::app_time::AppInstant;
use std::time::Duration;

#[cfg(debug_assertions)]
use crate::config::debug::{PRINT_JOURNEY_SUMMARY, PRINT_TRIGGER_UPDATES};
use crate::config::{CVA_MIN_SECONDS_BETWEEN_RECALCS, CVA_PRICE_RECALC_THRESHOLD_PCT};
use crate::ui::app::{DataParams, ZoneSniperApp};

#[derive(Debug, Default, Clone)]
pub(super) struct PairTriggerState {
    pub(super) anchor_price: Option<f64>,
    pub(super) pending_price: Option<f64>,
    pub(super) active_price: Option<f64>,
    pub(super) last_run_at: Option<AppInstant>,
    pub(super) is_stale: bool,
    pub(super) in_progress: bool,
    pub(super) stale_reason: Option<String>,
}

/// Lightweight trigger state for journey analysis per pair.
#[derive(Debug, Default, Clone)]
pub(super) struct JourneyTriggerState {
    pub(super) is_stale: bool,
    pub(super) in_progress: bool,
    pub(super) last_run_at: Option<AppInstant>,
    pub(super) stale_reason: Option<String>,
}

impl JourneyTriggerState {
    pub(super) fn mark_stale(&mut self, reason: impl Into<String>) {
        self.is_stale = true;
        self.stale_reason = Some(reason.into());
    }

    pub(super) fn reset_waiting_for_fresh_cva(&mut self, reason: impl Into<String>) {
        self.is_stale = false;
        self.in_progress = false;
        self.stale_reason = Some(reason.into());
        self.last_run_at = None;
    }

    pub(super) fn ready_to_run(&self) -> bool {
        self.is_stale && !self.in_progress
    }

    pub(super) fn on_run_started(&mut self) {
        self.in_progress = true;
        self.is_stale = false;
    }

    pub(super) fn on_run_finished(&mut self) {
        self.in_progress = false;
        self.last_run_at = Some(AppInstant::now());
        self.stale_reason = None;
    }
}

impl PairTriggerState {
    pub(super) fn mark_stale(&mut self, reason: impl Into<String>, pending_price: Option<f64>) {
        self.is_stale = true;
        self.stale_reason = Some(reason.into());
        if let Some(price) = pending_price {
            self.pending_price = Some(price);
        }
    }

    pub(super) fn consider_price_move(&mut self, new_price: f64) -> bool {
        if self.in_progress {
            if let Some(anchor) = self.active_price.or(self.anchor_price) {
                let pct = (new_price - anchor).abs() / anchor.max(f64::EPSILON);
                if pct >= CVA_PRICE_RECALC_THRESHOLD_PCT {
                    self.pending_price = Some(new_price);
                }
            } else {
                self.pending_price = Some(new_price);
            }
            return false;
        }

        if let Some(anchor) = self.anchor_price {
            let pct = (new_price - anchor).abs() / anchor.max(f64::EPSILON);
            if pct >= CVA_PRICE_RECALC_THRESHOLD_PCT {
                let msg = format!(
                    "price move {:.2}% (anchor {:.4} → {:.4})",
                    pct * 100.0,
                    anchor,
                    new_price
                );
                self.mark_stale(msg, Some(new_price));
                return true;
            }
        } else {
            self.mark_stale("initial analysis", Some(new_price));
            return true;
        }

        false
    }

    pub(super) fn ready_to_schedule(&self) -> bool {
        self.is_stale && !self.in_progress
    }

    pub(super) fn on_job_scheduled(&mut self, price: f64) {
        self.in_progress = true;
        self.is_stale = false;
        self.active_price = Some(price);
        self.pending_price = None;
    }

    pub(super) fn on_job_success(&mut self) -> Option<f64> {
        let follow_up = self.pending_price;
        if let Some(price) = self.active_price {
            self.anchor_price = Some(price);
        }
        self.active_price = None;
        self.pending_price = None;
        self.in_progress = false;
        self.last_run_at = Some(AppInstant::now());
        self.stale_reason = None;
        follow_up
    }

    pub(super) fn on_job_failure(&mut self, reason: impl Into<String>) {
        self.in_progress = false;
        self.is_stale = false;
        self.active_price = None;
        self.pending_price = None;
        self.last_run_at = Some(AppInstant::now());
        self.stale_reason = Some(reason.into());
    }

    pub(super) fn recent_run(&self) -> bool {
        self.last_run_at
            .map(|ts| ts.elapsed() < Duration::from_secs(CVA_MIN_SECONDS_BETWEEN_RECALCS))
            .unwrap_or(false)
    }
}

impl ZoneSniperApp {
    pub(super) fn sync_pair_triggers(&mut self) -> Vec<String> {
        let available_pairs = self.data_state.timeseries_collection.unique_pair_names();

        self.pair_triggers
            .retain(|pair, _| available_pairs.contains(pair));

        for pair in &available_pairs {
            self.pair_triggers.entry(pair.clone()).or_default();
        }

        available_pairs
    }

    pub(super) fn sync_journey_triggers(&mut self) {
        let available_pairs = self.data_state.timeseries_collection.unique_pair_names();

        self.journey_triggers
            .retain(|pair, _| available_pairs.contains(pair));

        for pair in &available_pairs {
            self.journey_triggers.entry(pair.clone()).or_default();
        }
    }

    pub(super) fn mark_pair_trigger_stale(
        &mut self,
        pair: &str,
        reason: impl Into<String>,
        price_hint: Option<f64>,
    ) {
        let trigger = self.pair_triggers.entry(pair.to_string()).or_default();
        trigger.mark_stale(reason, price_hint);
    }

    pub(super) fn mark_selected_pair_stale(&mut self, reason: impl Into<String>) {
        if let Some(pair) = self.selected_pair.clone() {
            self.mark_pair_trigger_stale(&pair, reason, self.current_pair_price);
        }
    }

    pub(super) fn schedule_recalc_now(&mut self, params: DataParams) {
        let Some(selected_pair) = params.selected_pair.clone() else {
            return;
        };

        if let Some(trigger) = self.pair_triggers.get_mut(&selected_pair) {
            if let Some(price) = trigger.pending_price.or(trigger.anchor_price) {
                trigger.on_job_scheduled(price);
            } else if let Some(price) = self.current_pair_price {
                trigger.on_job_scheduled(price);
            } else {
                let mid_price = (params.price_range.0 + params.price_range.1) * 0.5;
                trigger.on_job_scheduled(mid_price);
            }
        }

        self.start_async_calculation(params);
    }

    pub(super) fn schedule_selected_pair_recalc(&mut self, reason: impl Into<String>) {
        let reason = reason.into();
        let Some(pair) = self.selected_pair.clone() else {
            return;
        };

        let ready = {
            let trigger = self.pair_triggers.entry(pair.clone()).or_default();
            trigger.mark_stale(reason.clone(), self.current_pair_price);
            trigger.ready_to_schedule()
        };

        if ready {
            self.enqueue_recalc_for_pair(pair);
        } else if cfg!(debug_assertions) {
            if let Some(price) = self.current_pair_price {
                log::info!(
                    "[trigger] Marked {pair} stale ({reason}), waiting on debounce/availability @ {:.4}",
                    price
                );
            } else {
                log::info!("[trigger] Marked {pair} stale ({reason}), awaiting first price")
            }
        }
    }

    pub(super) fn enqueue_recalc_for_pair(&mut self, pair: String) {
        if self.is_calculating() {
            return;
        }

        let Some(current_price) = self.get_display_price(&pair) else {
            return;
        };

        let Some((ranges, price_range)) = self.compute_slice_selection(&pair, current_price) else {
            return;
        };

        if ranges.is_empty() {
            #[cfg(debug_assertions)]
            log::info!("[trigger] No slice ranges for {pair}");
            return;
        }

        if self.update_slices_if_changed(ranges.clone(), price_range) {
            self.data_state.clear_zone_efficacy();
        }

        let params = DataParams::from_app(
            &Some(pair.clone()),
            self.zone_count,
            self.time_decay_factor,
            ranges,
            price_range,
        );

        if let Some(last_failed) = self.last_failed_params_by_pair.get(&pair) {
            if last_failed == &params {
                #[cfg(debug_assertions)]
                if PRINT_TRIGGER_UPDATES {
                    log::info!(
                        "[trigger] skipping {} – params match last failed attempt",
                        pair
                    );
                }
                return;
            }
        }

        self.schedule_recalc_now(params);
    }

    pub(super) fn drain_trigger_queue(&mut self) {
        if self.is_calculating() {
            return;
        }

        let mut ready_pairs: Vec<String> = self
            .pair_triggers
            .iter()
            .filter_map(|(pair, trigger)| {
                if trigger.ready_to_schedule()
                    && (trigger.pending_price.is_some() || !trigger.recent_run())
                {
                    Some(pair.clone())
                } else {
                    None
                }
            })
            .collect();

        if let Some(selected) = &self.selected_pair {
            if let Some(idx) = ready_pairs.iter().position(|pair| pair == selected) {
                ready_pairs.swap(0, idx);
            } else {
                // Selected pair isn't ready yet; try to promote its trigger if possible.
                if let Some(trigger) = self.pair_triggers.get_mut(selected) {
                    if trigger.pending_price.is_some() || !trigger.recent_run() {
                        trigger.mark_stale("selected pair priority", None);
                        ready_pairs.insert(0, selected.clone());
                    }
                }
            }
        }

        for pair in ready_pairs {
            self.enqueue_recalc_for_pair(pair);
            if self.is_calculating() {
                break;
            }
        }
    }

    /// Mark a pair's journeys as stale and enqueue it for background analysis.
    pub(super) fn mark_journey_stale_and_enqueue(&mut self, pair: &str, reason: impl Into<String>) {
        let reason = reason.into();
        let trigger = self.journey_triggers.entry(pair.to_string()).or_default();
        trigger.mark_stale(reason.clone());

        if !self.journey_queue.iter().any(|queued| queued == pair) {
            self.journey_queue.push_back(pair.to_string());

            #[cfg(debug_assertions)]
            if PRINT_JOURNEY_SUMMARY {
                log::info!(
                    "[journey] queued {pair} as stale ({reason}); queue_len={}",
                    self.journey_queue.len()
                );
            }
        }
    }

    pub(super) fn mark_all_journeys_stale(&mut self, reason: impl Into<String>) {
        let reason = reason.into();
        let available_pairs = self.data_state.timeseries_collection.unique_pair_names();
        self.sync_journey_triggers();

        for pair in available_pairs {
            self.mark_journey_stale_and_enqueue(&pair, reason.clone());
        }
    }

    /// Clear pending journey work and mark every pair's CVA trigger stale so a
    /// global parameter change (e.g. zone count, price range) cascades through
    /// all pairs before journeys resume.
    pub(super) fn invalidate_all_pairs_for_global_change(&mut self, reason: impl Into<String>) {
        let reason = reason.into();

        let available_pairs = self.sync_pair_triggers();
        self.sync_journey_triggers();

        self.journey_queue.clear();

        for trigger in self.journey_triggers.values_mut() {
            trigger.reset_waiting_for_fresh_cva(format!("{reason} (awaiting fresh CVA)"));
        }

        for pair in available_pairs {
            if let Some(trigger) = self.pair_triggers.get_mut(&pair) {
                trigger.mark_stale(reason.clone(), None);
                trigger.last_run_at = None;
            }
            self.last_calculated_params_by_pair.remove(&pair);
        }

        #[cfg(debug_assertions)]
        if PRINT_JOURNEY_SUMMARY {
            log::info!("[journey] global change -> cleared queue; awaiting fresh CVA ({reason})");
        }
    }

    /// Drain the journey queue, running at most one journey batch per frame
    /// to avoid large UI stalls. Journeys only run when no CVA calculation is
    /// in progress.
    pub(super) fn drain_journey_queue(&mut self) {
        if self.is_calculating() {
            #[cfg(debug_assertions)]
            if PRINT_JOURNEY_SUMMARY && !self.journey_queue.is_empty() {
                log::info!(
                    "[journey] CVA in progress; deferring {} queued journey(s)",
                    self.journey_queue.len()
                );
            }
            return;
        }

        let Some(pair) = self.journey_queue.pop_front() else {
            return;
        };

        {
            let Some(trigger) = self.journey_triggers.get_mut(&pair) else {
                return;
            };

            if !trigger.ready_to_run() {
                #[cfg(debug_assertions)]
                if PRINT_JOURNEY_SUMMARY {
                    log::info!("[journey] skipped {pair} – not ready_to_run");
                }
                return;
            }

            trigger.on_run_started();
        }

        #[cfg(debug_assertions)]
        let started_at = if PRINT_JOURNEY_SUMMARY {
            Some(AppInstant::now())
        } else {
            None
        };

        self.run_journeys_for_pair(&pair);

        if let Some(trigger) = self.journey_triggers.get_mut(&pair) {
            trigger.on_run_finished();
        }

        #[cfg(debug_assertions)]
        if PRINT_JOURNEY_SUMMARY {
            if let Some(ts) = started_at {
                let elapsed = ts.elapsed();
                log::info!(
                    "[journey] completed {pair} in {:.3}s",
                    elapsed.as_secs_f32()
                );
            } else {
                log::info!("[journey] completed {pair}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < f64::EPSILON
    }

    #[test]
    fn initial_price_marks_stale_and_ready() {
        let mut trigger = PairTriggerState::default();

        assert!(trigger.consider_price_move(10_000.0));
        assert!(trigger.ready_to_schedule());
        assert_eq!(trigger.pending_price, Some(10_000.0));
        assert!(trigger.stale_reason.is_some());
    }

    #[test]
    fn follow_up_price_is_queued_during_in_progress_run() {
        let mut trigger = PairTriggerState {
            anchor_price: Some(100.0),
            ..Default::default()
        };

        assert!(trigger.consider_price_move(101.5));
        assert!(trigger.ready_to_schedule());

        trigger.on_job_scheduled(101.5);
        assert!(trigger.in_progress);
        assert_eq!(trigger.pending_price, None);

        assert!(!trigger.consider_price_move(103.0));
        assert_eq!(trigger.pending_price, Some(103.0));

        let follow_up = trigger.on_job_success();
        assert_eq!(follow_up, Some(103.0));
        assert!(!trigger.in_progress);
        assert!(approx_eq(trigger.anchor_price.unwrap(), 101.5));
    }

    #[test]
    fn recent_run_prevents_immediate_reschedule_without_follow_up() {
        let mut trigger = PairTriggerState {
            anchor_price: Some(200.0),
            ..Default::default()
        };

        assert!(trigger.consider_price_move(204.0));
        trigger.on_job_scheduled(204.0);
        trigger.on_job_success();

        trigger.last_run_at = Some(AppInstant::now());
        trigger.mark_stale("within debounce", Some(204.0));

        assert!(trigger.ready_to_schedule());
        assert!(trigger.recent_run());
    }
}
