use crate::utils::app_time::now;
use std::time::Duration;

use crate::journeys::{JourneyAnalyzer, JourneyExecution, ZoneTarget};
use crate::models::PairContext;

use super::app::{JourneySummaryUpdate, ZoneSniperApp};
use crate::config::debug::PRINT_TRIGGER_UPDATES;
use crate::ui::config::UI_TEXT;

struct JourneyContextResult {
    summary: JourneySummaryUpdate,
}

impl ZoneSniperApp {
    fn log_journey_execution(execution: &JourneyExecution) {
        let stats = &execution.analysis.stats;

        if stats.total_attempts == 0 {
            if PRINT_TRIGGER_UPDATES {
                log::info!(
                    "  Zone #{:03} [{:.4} - {:.4}] target {:.4}: no historical samples (elapsed {:.2?})",
                    execution.zone_index,
                    execution.zone_bottom,
                    execution.zone_top,
                    execution.target_price,
                    execution.elapsed
                );
            }
            return;
        }

        let ci = stats.confidence_interval_success;
        let expected = &stats.expected_value;
        let kelly_str = expected
            .kelly_criterion
            .map(|k| format!("{:.2}%", k * 100.0))
            .unwrap_or_else(|| "n/a".to_string());

        if PRINT_TRIGGER_UPDATES {
            log::info!(
                "  Zone #{:03} [{:.4} - {:.4}] target {:.4}\n    attempts: {} | successes: {} ({:.1}% | CI {:.1}% - {:.1}%)\n    expected annualized return: {:.2}% | avg ROI {:.2}% / {:.2}% (success/failure)\n    kelly: {} | compute time {:.2?}",
                execution.zone_index,
                execution.zone_bottom,
                execution.zone_top,
                execution.target_price,
                stats.total_attempts,
                stats.success_count,
                stats.success_rate * 100.0,
                ci.0 * 100.0,
                ci.1 * 100.0,
                stats.expected_annualized_return,
                stats.avg_success_roi * 100.0,
                stats.avg_failure_roi * 100.0,
                kelly_str,
                execution.elapsed
            );
        }
    }

    fn collect_zone_targets(context: &PairContext, should_log: bool) -> Option<Vec<ZoneTarget>> {
        let sticky_superzones = &context.trading_model.zones.sticky_superzones;
        if sticky_superzones.is_empty() {
            if should_log && PRINT_TRIGGER_UPDATES {
                log::info!(
                    "Journey analysis skipped for {}: no sticky zones available.",
                    context.pair_name
                );
            }
            return None;
        }

        let zone_targets: Vec<ZoneTarget> = sticky_superzones
            .iter()
            .map(|sz| ZoneTarget {
                index: sz.id,
                price_bottom: sz.price_bottom,
                price_top: sz.price_top,
            })
            .collect();

        if zone_targets.is_empty() {
            if should_log && PRINT_TRIGGER_UPDATES {
                log::info!(
                    "Journey analysis skipped for {}: no zone targets constructed.",
                    context.pair_name
                );
            }
            return None;
        }

        Some(zone_targets)
    }

    fn log_no_journey_executions(pair_name: &str, elapsed: Duration) {
        if PRINT_TRIGGER_UPDATES {
            log::info!(
                "Journeys for {} skipped: no executions produced (elapsed {:.2?})",
                pair_name,
                elapsed
            );
        }
    }

    fn current_time_horizon_seconds(&self) -> u64 {
        let days = self.time_horizon_days.max(1);
        days.saturating_mul(86_400)
    }

    fn analyze_journey_for_context(
        &self,
        context: &PairContext,
        analyzer: &JourneyAnalyzer,
        should_log_pair: bool,
    ) -> JourneyContextResult {
        let Some(zone_targets) = Self::collect_zone_targets(context, should_log_pair) else {
            return JourneyContextResult {
                summary: JourneySummaryUpdate::NoData(UI_TEXT.journey_status_no_zones.to_string()),
            };
        };

        let pair_start = now();

        let interval_ms = crate::config::INTERVAL_WIDTH_TO_ANALYSE_MS;
        let tolerance_pct = crate::config::JOURNEY_START_PRICE_TOLERANCE_PCT;
        let time_horizon = Duration::from_secs(self.current_time_horizon_seconds());
        let stop_loss_pct = crate::config::JOURNEY_STOP_LOSS_PCT;

        match analyzer.analyze_zones(
            &context.pair_name,
            interval_ms,
            context.current_price,
            &zone_targets,
            tolerance_pct,
            time_horizon,
            false,
            stop_loss_pct,
        ) {
            Ok(executions) => {
                let pair_elapsed = pair_start.elapsed();

                if should_log_pair && PRINT_TRIGGER_UPDATES {
                    log::info!(
                        "\n=== Journeys for {} (price {:.2}) — {} zones analysed in {:.2?}",
                        context.pair_name,
                        context.current_price,
                        executions.len(),
                        pair_elapsed
                    );
                }

                if executions.is_empty() {
                    if should_log_pair && PRINT_TRIGGER_UPDATES {
                        Self::log_no_journey_executions(&context.pair_name, pair_elapsed);
                    }

                    return JourneyContextResult {
                        summary: JourneySummaryUpdate::NoData(
                            UI_TEXT.journey_status_no_executions.to_string(),
                        ),
                    };
                }

                for execution in &executions {
                    if should_log_pair {
                        Self::log_journey_execution(execution);
                    }
                }

                JourneyContextResult {
                    summary: JourneySummaryUpdate::Success {
                        executions: executions.clone(),
                        elapsed: pair_elapsed,
                    },
                }
            }
            Err(err) => {
                if should_log_pair {
                    log::info!(
                        "⚠️ Journey analysis failed for {}: {}",
                        context.pair_name,
                        err
                    );
                }

                JourneyContextResult {
                    summary: JourneySummaryUpdate::Failure(format!(
                        "{}: {}",
                        UI_TEXT.journey_status_error_prefix, err
                    )),
                }
            }
        }
    }

    pub(super) fn run_journeys_for_pair(&mut self, pair_name: &str) {
        self.initialize_multi_pair_monitor();

        let Some(context) = self.multi_pair_monitor.get_context(pair_name) else {
            if !crate::config::debug::PRINT_JOURNEY_FOR_PAIR.is_empty()
                && crate::config::debug::PRINT_JOURNEY_FOR_PAIR == pair_name
                && PRINT_TRIGGER_UPDATES
            {
                log::info!(
                    "Journey analysis skipped for {}: no pair context available.",
                    pair_name
                );
            }
            return;
        };

        let should_log_pair = !crate::config::debug::PRINT_JOURNEY_FOR_PAIR.is_empty()
            && crate::config::debug::PRINT_JOURNEY_FOR_PAIR == context.pair_name;

        let analyzer = JourneyAnalyzer::new(&self.data_state.timeseries_collection);

        let JourneyContextResult { summary } =
            self.analyze_journey_for_context(context, &analyzer, should_log_pair);

        self.apply_journey_summary_updates(vec![(context.pair_name.clone(), summary)]);
    }
}
