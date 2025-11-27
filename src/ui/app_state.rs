use std::sync::Arc;

use crate::journeys::compute_zone_efficacy;
use crate::models::{TradingModel, find_matching_ohlcv};

use super::app::ZoneSniperApp;
#[cfg(debug_assertions)]
use crate::config::debug::PRINT_UI_INTERACTIONS;

impl ZoneSniperApp {
    pub(super) fn compute_slice_selection(
        &self,
        pair_name: &str,
        price: f64,
    ) -> Option<(Vec<(usize, usize)>, (f64, f64))> {
        let timeseries = find_matching_ohlcv(
            &self.data_state.timeseries_collection.series_data,
            pair_name,
            crate::config::INTERVAL_WIDTH_TO_ANALYSE_MS,
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

    pub(super) fn update_zone_efficacy(&mut self) {
        let Some(selected_pair) = self.selected_pair.clone() else {
            return;
        };

        let Some(cva_results) = self.data_state.cva_results.as_ref() else {
            return;
        };

        let timeseries = match find_matching_ohlcv(
            &self.data_state.timeseries_collection.series_data,
            &selected_pair,
            crate::config::INTERVAL_WIDTH_TO_ANALYSE_MS,
        ) {
            Ok(ts) => ts,
            Err(_) => return,
        };

        let (price_min, price_max) = cva_results.price_range.min_max();
        if !price_min.is_finite() || !price_max.is_finite() || price_max <= price_min {
            return;
        }

        let trading_model =
            TradingModel::from_cva(Arc::clone(cva_results), self.current_pair_price);
        if let Some(stats) = compute_zone_efficacy(
            timeseries,
            &trading_model.zones.sticky_superzones,
            self.computed_slice_indices.as_deref().unwrap_or(&[]),
            (price_min, price_max),
        ) {
            self.data_state.zone_efficacy = Some((selected_pair, stats));
        }
    }

    pub(super) fn apply_cached_results_for_pair(&mut self, pair: &str) -> bool {
        let Some(cva) = self.cva_results_by_pair.get(pair) else {
            return false;
        };

        self.data_state.cva_results = Some(Arc::clone(cva));

        if let Some(params) = self.last_calculated_params_by_pair.get(pair) {
            self.computed_slice_indices = Some(params.slice_ranges.clone());
            self.last_price_range = Some(params.price_range);
            self.last_calculated_params = Some(params.clone());
        }

        self.update_zone_efficacy();
        true
    }

    pub(super) fn handle_pair_selection(&mut self, new_pair: String) {
        if self.selected_pair.as_ref() == Some(&new_pair) {
            return;
        }

        self.selected_pair = Some(new_pair.clone());
        self.data_state.clear_zone_efficacy();

        if self.apply_cached_results_for_pair(&new_pair) {
            #[cfg(debug_assertions)]
            if PRINT_UI_INTERACTIONS {
                log::info!("[pair] Switched to {new_pair} using cached CVA results");
            }
            return;
        }

        self.data_state.cva_results = None;
        self.computed_slice_indices = None;
        self.last_price_range = None;
        self.last_calculated_params = None;

        let price_hint = self.get_display_price(&new_pair);
        self.mark_pair_trigger_stale(&new_pair, "first selection (no cached CVA)", price_hint);
    }
}
