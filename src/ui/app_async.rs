use crate::utils::app_time::now;
use eframe::egui;
use poll_promise::Promise;
use std::sync::Arc;
use std::time::Duration;

use crate::analysis::pair_analysis::ZoneGenerator;
#[cfg(debug_assertions)]
use crate::config::DEBUG_FLAGS;
use crate::data::timeseries::TimeSeriesCollection;
use crate::models::{CVACore, PairContext, TradingModel};
use crate::ui::app::{AppError, DataParams, ZoneSniperApp};

pub(super) struct AsyncCalcResult {
    pub(super) result: Result<Arc<CVACore>, AppError>,
    pub(super) params: DataParams,
    elapsed_time: Duration,
}

impl AsyncCalcResult {
    pub(super) fn elapsed_time(&self) -> Duration {
        self.elapsed_time
    }
}

impl ZoneSniperApp {
    pub(super) fn start_async_calculation(&mut self, params: DataParams) {
        if self.calculation_promise.is_some() {
            return;
        }

        if let Err(e) = params.is_valid() {
            self.data_state.last_error = Some(e);
            return;
        }

        let generator = self.data_state.generator.clone();
        let timeseries = self.data_state.timeseries_collection.clone();
        let params_clone = params.clone();

        #[cfg(not(target_arch = "wasm32"))]
        let promise = Promise::spawn_thread("cva_calculation", move || {
            run_cva_calculation(generator, timeseries, params_clone)
        });

        #[cfg(target_arch = "wasm32")]
        let promise = {
            let result = run_cva_calculation(generator, timeseries, params_clone);
            Promise::from_ready(result)
        };

        self.calculation_promise = Some(promise);
    }

    pub(super) fn poll_async_calculation(&mut self, ctx: &egui::Context) {
        let outcome = self.calculation_promise.as_ref().and_then(|promise| {
            promise.ready().map(|calc_result| {
                let result = calc_result
                    .result
                    .as_ref()
                    .map(Arc::clone)
                    .map_err(|err| err.clone());
                let params = calc_result.params.clone();
                let elapsed = calc_result.elapsed_time();
                (result, params, elapsed)
            })
        });

        if let Some((result, params, elapsed)) = outcome {
            self.calculation_promise = None;

            let completed_pair = params.selected_pair.clone();

            // Detect whether this run represents a real parameter change versus
            // a no-op rerun (e.g., repeatedly focusing the same pair and
            // hitting a warm CVA cache). We only auto-run journeys when the
            // effective DataParams differ from the last accepted set **for that pair**.
            let params_changed = if let Some(pair_name) = completed_pair.as_deref() {
                self.last_calculated_params_by_pair
                    .get(pair_name)
                    .map(|prev| prev != &params)
                    .unwrap_or(true)
            } else {
                true
            };

            match result {
                Ok(cva_results) => {
                    let is_selected = self.selected_pair.as_deref() == completed_pair.as_deref();

                    if let Some(pair_name) = completed_pair.as_deref() {
                        self.last_failed_params_by_pair.remove(pair_name);
                        self.last_calculated_params_by_pair
                            .insert(pair_name.to_string(), params.clone());
                        self.cva_results_by_pair
                            .insert(pair_name.to_string(), Arc::clone(&cva_results));
                    }

                    if is_selected {
                        self.last_calculated_params = Some(params.clone());
                        self.data_state.cva_results = Some(Arc::clone(&cva_results));
                        self.data_state.last_error = None;
                        self.update_zone_efficacy();
                    }

                    if let Some(pair_name) = completed_pair.as_deref() {
                        if let Some(price) = self.get_display_price(pair_name) {
                            let trading_model =
                                TradingModel::from_cva(Arc::clone(&cva_results), Some(price));
                            let context = PairContext::new(trading_model, price);
                            self.multi_pair_monitor.add_pair(context);

                            #[cfg(debug_assertions)]
                            if DEBUG_FLAGS.print_monitor_progress {
                                log::info!("✨ Updated {} in multi-pair monitor", pair_name);
                            }
                        }
                    }

                    // Automatically schedule journeys for the completed pair using the
                    // updated model, but only when the effective parameters have
                    // actually changed. Cache hits on identical params should not
                    // retrigger journeys unnecessarily.
                    if params_changed {
                        if let Some(pair_name) = completed_pair.as_deref() {
                            #[cfg(debug_assertions)]
                            if DEBUG_FLAGS.print_trigger_updates {
                                log::info!(
                                    "⚙️  Marking journeys stale after CVA completion for {} (params changed)",
                                    pair_name
                                );
                            }
                            self.mark_journey_stale_and_enqueue(pair_name, "CVA params changed");
                        }
                    }

                    if let Some(pair_name) = completed_pair.clone() {
                        let follow_up = {
                            let trigger = self.pair_triggers.entry(pair_name.clone()).or_default();
                            trigger.on_job_success()
                        };

                        if let Some(next_price) = follow_up {
                            let reason = format!("follow-up price move @ {:.4}", next_price);
                            self.mark_pair_trigger_stale(&pair_name, reason, Some(next_price));

                            #[cfg(debug_assertions)]
                            if DEBUG_FLAGS.print_trigger_updates {
                                log::info!(
                                    "[trigger] queued follow-up for {} @ {:.4}",
                                    pair_name,
                                    next_price
                                );
                            }
                        }
                    }

                    if elapsed.as_millis() > 100 {
                        #[cfg(debug_assertions)]
                        log::info!(
                            "✅ Async calculation completed in {:.2}s",
                            elapsed.as_secs_f32()
                        );
                    }
                }
                Err(error) => {
                    let is_selected = self.selected_pair.as_deref() == completed_pair.as_deref();

                    if is_selected {
                        self.data_state.cva_results = None;
                        self.data_state.last_error = Some(error.clone());
                    }

                    if let Some(pair_name) = completed_pair.clone() {
                        let msg = error.to_string();
                        self.pair_triggers
                            .entry(pair_name.clone())
                            .or_default()
                            .on_job_failure(msg.clone());

                        if params.selected_pair.as_deref() == Some(&pair_name) {
                            self.last_failed_params_by_pair
                                .insert(pair_name.clone(), params.clone());
                        }

                        #[cfg(debug_assertions)]
                        if DEBUG_FLAGS.print_trigger_updates {
                            log::info!(
                                "[trigger] {} marked stale due to failure: {}",
                                pair_name,
                                msg
                            );
                        }
                    }

                    #[cfg(debug_assertions)]
                    log::error!("❌ Async calculation failed: {}", error);
                }
            }

            self.drain_trigger_queue();
        } else if self.calculation_promise.is_some() {
            ctx.request_repaint();
        }
    }

    pub(super) fn is_calculating(&self) -> bool {
        self.calculation_promise.is_some()
    }
}

fn run_cva_calculation(
    generator: ZoneGenerator,
    timeseries: TimeSeriesCollection,
    params_clone: DataParams,
) -> AsyncCalcResult {
    let calc_start = now();

    let result = match params_clone.pair() {
        Ok(pair) => {
            if !timeseries.unique_pair_names().iter().any(|s| s == pair) {
                Err(AppError::InvalidPair(pair.to_string()))
            } else {
                generator
                    .get_cva_results(
                        pair,
                        params_clone.zone_count,
                        params_clone.time_decay_factor,
                        &timeseries,
                        params_clone.slice_ranges.clone(),
                        params_clone.price_range,
                    )
                    .map_err(|e| AppError::CalculationFailed(e.to_string()))
            }
        }
        Err(e) => Err(e),
    };

    AsyncCalcResult {
        result,
        params: params_clone,
        elapsed_time: calc_start.elapsed(),
    }
}
