use std::sync::Arc;

use crate::TradingModel;
use crate::config::ANALYSIS;
use crate::models::find_matching_ohlcv;

use super::app::ZoneSniperApp;

impl ZoneSniperApp {
    pub(super) fn compute_slice_selection(
        &self,
        pair_name: &str,
        price: f64,
    ) -> Option<(Vec<(usize, usize)>, (f64, f64))> {
        let timeseries = find_matching_ohlcv(
            &self.data_state.timeseries_collection.series_data,
            pair_name,
            ANALYSIS.interval_width_ms,
        )
        .ok()?;

        Some(crate::domain::auto_duration::auto_select_ranges(
            timeseries,
            price,
            &self.auto_duration_config,
        ))
    }

    pub(super) fn update_slices_if_changed(
        &mut self,
        ranges: Vec<(usize, usize)>,
        price_range: (f64, f64),
    ) -> bool {
        let same_ranges = self
            .computed_slice_indices
            .as_ref()
            .map(|existing| existing == &ranges)
            .unwrap_or(false);
        let same_price_range = self
            .last_price_range
            .map(|pr| {
                pr.0.to_bits() == price_range.0.to_bits()
                    && pr.1.to_bits() == price_range.1.to_bits()
            })
            .unwrap_or(false);

        if same_ranges && same_price_range {
            return false;
        }

        self.computed_slice_indices = Some(ranges);
        self.last_price_range = Some(price_range);
        true
    }

    pub(super) fn apply_cached_results_for_pair(&mut self, pair: &str) -> bool {
        // 1. Extraction Phase (Immutable Borrow)
        // We clone the Arc and Params immediately to release the borrow on `self`.
        // 'map(Arc::clone)' increments the ref count, it doesn't deep copy the CVA data.
        let cva_opt = self.cva_results_by_pair.get(pair).map(Arc::clone);
        let params_opt = self.last_calculated_params_by_pair.get(pair).cloned();

        // If no CVA, we can't do anything
        let Some(cva) = cva_opt else {
            return false;
        };

        // 2. Update Phase (Mutable Borrow)
        // Now that we own 'cva' and 'params_opt' independently, we can mutate 'self'.

        // Restore Raw Data
        self.data_state.cva_results = Some(Arc::clone(&cva));

        // Get Price (Safe to call now)
        let price = self.get_display_price(pair);

        // Rebuild Model immediately
        // We pass the owned 'cva' Arc here
        self.data_state.current_model = Some(TradingModel::from_cva(cva, price));

        // Restore Params
        if let Some(params) = params_opt {
            self.computed_slice_indices = Some(params.slice_ranges.clone());
            self.last_price_range = Some(params.price_range);
            self.last_calculated_params = Some(params);
        }

        true
    }

    pub(super) fn handle_pair_selection(&mut self, new_pair: String) {
        if self.selected_pair.as_ref() == Some(&new_pair) {
            return;
        }

        // DEBUG LOG: Confirm the switch happening
        log::info!(
            ">>> UI: Switching selected pair from {:?} to {}",
            self.selected_pair,
            new_pair
        );

        // 1. Wipe State (CRITICAL: Fixes "Ghost Data" bug)
        self.data_state.cva_results = None;
        self.data_state.current_model = None; // Clears old coverage stats immediately
        self.data_state.last_error = None;
        self.last_price_range = None;
        self.computed_slice_indices = None;
        self.last_calculated_params = None;

        self.selected_pair = Some(new_pair.clone());

        // 2. Try Cache
        if self.apply_cached_results_for_pair(&new_pair) {
            // Success: state is now populated with NEW pair data
            log::info!(">>> UI: Found cached results for {}", new_pair);
            return;
        }

        // 3. Force Immediate Recalculation (Bypass Debounce)
        // We manually reset 'last_run_at' to None so the system doesn't make us wait
        // if this pair was recently updated in the background.
        if let Some(trigger) = self.pair_triggers.get_mut(&new_pair) {
            trigger.last_run_at = None; // Kill the debounce
        }

        let price_hint = self.get_display_price(&new_pair);
        self.mark_pair_trigger_stale(&new_pair, "first selection (no cached CVA)", price_hint);
    }
}
