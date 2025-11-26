//! Journey analysis module scaffolding.
//!
//! Provides core data structures for Historical Price Journey Analysis (HPJA)
//! as outlined in @docs/spec/spec.md.

use std::time::{Duration, Instant};

use crate::config::debug;
use crate::data::timeseries::TimeSeriesCollection;
use crate::models::timeseries::{OhlcvTimeSeries, find_matching_ohlcv};
use anyhow::{Result, anyhow};

const MILLIS_PER_DAY: f64 = 86_400_000.0;

/// Outcome classification for a historical journey attempt.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Target reached within the journey time budget.
    Success { days_elapsed: u16 },
    /// Journey timed out without catastrophic drawdown.
    TimedOut { final_price: f64 },
    /// Journey hit the stop-loss / drawdown threshold.
    StoppedOut { adverse_price: f64 },
}

/// Captures the result of replaying a historical journey from a start price.
#[derive(Debug, Clone)]
pub struct JourneyOutcome {
    pub start_timestamp_ms: i64,
    pub start_price: f64,
    pub outcome: Outcome,
    /// Time in days to reach the target (present for successful journeys).
    pub days_to_target: Option<u16>,
    /// Total elapsed journey time in days (success or failure).
    pub elapsed_days: f64,
    /// Worst percentage drawdown experienced during the journey.
    pub max_drawdown_pct: f64,
    /// Price at the end of the evaluation window.
    pub final_price: f64,
}

impl JourneyOutcome {
    /// Convenience predicate for successful journeys.
    pub fn is_success(&self) -> bool {
        matches!(self.outcome, Outcome::Success { .. })
    }
}

/// Summary metrics describing loss characteristics for failed journeys.
#[derive(Debug, Default, Clone)]
pub struct RiskMetrics {
    pub avg_loss_on_failure: f64,
    pub median_loss: f64,
    pub worst_case_loss: f64,
    pub avg_max_drawdown: f64,
}

/// Expected value metrics derived from historical journeys.
#[derive(Debug, Default, Clone)]
pub struct ExpectedValue {
    pub expected_annualized_return: f64,
    pub kelly_criterion: Option<f64>,
}

/// Aggregate metrics derived from a set of journey outcomes.
#[derive(Debug, Default, Clone)]
pub struct JourneyStats {
    pub total_attempts: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub success_rate: f64,
    pub probability_success: f64,
    pub probability_failure: f64,
    pub confidence_interval_success: (f64, f64),
    pub avg_success_roi: f64,
    pub avg_failure_roi: f64,
    pub avg_success_annualized_roi: f64,
    pub avg_failure_annualized_roi: f64,
    pub expected_annualized_return: f64,
    pub risk_metrics: RiskMetrics,
    pub expected_value: ExpectedValue,
}

/// Result wrapper combining raw outcomes with summary statistics.
#[derive(Debug, Default, Clone)]
pub struct JourneyAnalysisResult {
    pub outcomes: Vec<JourneyOutcome>,
    pub stats: JourneyStats,
}

/// Parameter bundle describing the evaluation context for a set of journeys.
#[derive(Debug, Clone)]
pub struct JourneyParams {
    pub pair: String,
    pub interval_ms: i64,
    pub start_price: f64,
    pub end_price: f64,
    pub max_journey_time: Duration,
    /// Percent tolerance used when matching historical start prices.
    pub start_price_tolerance_pct: f64,
    pub stop_loss_pct: f64,
    pub compute_kelly: bool,
}

/// Historical snapshot where price matched the requested start conditions.
#[derive(Debug, Clone)]
struct PriceMatch {
    timestamp_ms: i64,
    close_price: f64,
    candle_index: usize,
}

/// Zone or superzone descriptor used for journey targeting.
#[derive(Debug, Clone)]
pub struct ZoneTarget {
    pub index: usize,
    pub price_bottom: f64,
    pub price_top: f64,
}

/// Request payload for running a journey toward a specific zone edge.
#[derive(Debug, Clone)]
pub struct JourneyRequest<'a> {
    pub pair: &'a str,
    pub interval_ms: i64,
    pub current_price: f64,
    pub target: &'a ZoneTarget,
    pub start_price_tolerance_pct: f64,
    pub max_journey_time: Duration,
    pub stop_loss_pct: f64,
    pub compute_kelly: bool,
}

/// Execution result for a single journey analysis run.
#[derive(Debug, Clone)]
pub struct JourneyExecution {
    pub zone_index: usize,
    pub zone_bottom: f64,
    pub zone_top: f64,
    pub target_price: f64,
    pub direction_up: bool,
    pub analysis: JourneyAnalysisResult,
    pub elapsed: Duration,
}

/// Skeleton analyzer struct that will own matching/tracking logic.
#[derive(Debug)]
pub struct JourneyAnalyzer<'a> {
    timeseries: &'a TimeSeriesCollection,
}

impl<'a> JourneyAnalyzer<'a> {
    /// Creates an analyzer over an existing time-series collection.
    pub fn new(timeseries: &'a TimeSeriesCollection) -> Self {
        Self { timeseries }
    }

    /// Executes the price-level matching + outcome tracking pipeline.
    pub fn analyze(&self, params: &JourneyParams) -> Result<JourneyAnalysisResult> {
        let timeseries = find_matching_ohlcv(
            &self.timeseries.series_data,
            &params.pair,
            params.interval_ms,
        )
        .map_err(|e| anyhow!("Failed to locate OHLCV data: {e}"))?;

        let price_matches = self.match_start_prices(timeseries, params)?;

        if price_matches.is_empty() {
            return Ok(JourneyAnalysisResult::default());
        }

        let outcomes = self.evaluate_price_matches(timeseries, &price_matches, params);
        let stats = self.compute_stats(&outcomes, params);

        Ok(JourneyAnalysisResult { outcomes, stats })
    }

    fn match_start_prices(
        &self,
        timeseries: &OhlcvTimeSeries,
        params: &JourneyParams,
    ) -> Result<Vec<PriceMatch>> {
        if params.start_price <= 0.0 {
            return Err(anyhow!("Start price must be positive"));
        }

        if timeseries.pair_interval.interval_ms <= 0 {
            return Err(anyhow!(
                "Invalid interval ({} ms) for pair {}",
                timeseries.pair_interval.interval_ms,
                timeseries.pair_interval.name
            ));
        }

        let tolerance_fraction = params.start_price_tolerance_pct / 100.0;
        if tolerance_fraction < 0.0 {
            return Err(anyhow!("Start price tolerance must be non-negative"));
        }

        let mut matches = Vec::new();

        for (idx, close_price) in timeseries.close_prices.iter().enumerate() {
            let price_delta = (close_price - params.start_price).abs() / params.start_price;
            if price_delta <= tolerance_fraction {
                let timestamp_ms = timeseries.first_kline_timestamp_ms
                    + (idx as i64 * timeseries.pair_interval.interval_ms);
                matches.push(PriceMatch {
                    timestamp_ms,
                    close_price: *close_price,
                    candle_index: idx,
                });
            }
        }

        Ok(matches)
    }

    fn evaluate_price_matches(
        &self,
        timeseries: &OhlcvTimeSeries,
        price_matches: &[PriceMatch],
        params: &JourneyParams,
    ) -> Vec<JourneyOutcome> {
        let interval_ms = timeseries.pair_interval.interval_ms as u64;
        let window_ms = params.max_journey_time.as_millis();
        let mut max_steps = if interval_ms == 0 || window_ms == 0 {
            0
        } else {
            window_ms.div_ceil(interval_ms as u128) as usize
        };

        if max_steps == 0 && window_ms > 0 {
            max_steps = 1;
        }

        let mut outcomes = Vec::with_capacity(price_matches.len());
        let stop_loss_fraction = (params.stop_loss_pct / 100.0).max(0.0);

        for (attempt_index, price_match) in price_matches.iter().enumerate() {
            let start_idx = price_match.candle_index;
            if start_idx >= timeseries.close_prices.len() {
                continue;
            }

            let mut outcome = Outcome::TimedOut {
                final_price: timeseries.close_prices[start_idx],
            };
            let mut final_price = timeseries.close_prices[start_idx];
            let mut days_to_target = None;
            let mut elapsed_days = 0.0;
            let mut steps_taken = 0usize;

            let mut worst_adverse_price = price_match.close_price;
            let target_is_above = params.end_price >= price_match.close_price;
            let stop_loss_price = if stop_loss_fraction > 0.0 {
                if target_is_above {
                    Some((price_match.close_price * (1.0 - stop_loss_fraction)).max(0.0))
                } else {
                    Some(price_match.close_price * (1.0 + stop_loss_fraction))
                }
            } else {
                None
            };

            let debug_this_attempt = cfg!(debug_assertions)
                && !debug::PRINT_JOURNEY_FOR_PAIR.is_empty()
                && debug::PRINT_JOURNEY_FOR_PAIR == params.pair
                && debug::PRINT_TRIGGER_UPDATES
                && debug::DEBUG_JOURNEY_ATTEMPT_INDEX >= 0
                && attempt_index == debug::DEBUG_JOURNEY_ATTEMPT_INDEX as usize;

            if debug_this_attempt {
                println!(
                    "\n=== Debug journey attempt #{:03} for pair {} ===\nstart_timestamp_ms: {}\nstart_price: {:.4}\ntarget_price: {:.4} (target_is_above = {})\nstop_loss_pct: {:.2}%\nmax_journey_time: {:?}\nmax_steps: {}\n",
                    attempt_index,
                    params.pair,
                    price_match.timestamp_ms,
                    price_match.close_price,
                    params.end_price,
                    target_is_above,
                    params.stop_loss_pct,
                    params.max_journey_time,
                    max_steps,
                );
            }

            for step in 1..=max_steps {
                let idx = start_idx + step;
                if idx >= timeseries.close_prices.len() {
                    break;
                }

                steps_taken = step;

                let high = timeseries.high_prices[idx];
                let low = timeseries.low_prices[idx];

                if debug_this_attempt {
                    let elapsed_ms = (step as u64 * interval_ms) as f64;
                    let elapsed_days_debug = (elapsed_ms / MILLIS_PER_DAY).max(1.0);
                    println!(
                        "step {:04} | idx {} | elapsed_days ~ {:.2} | high {:.4} | low {:.4}",
                        step, idx, elapsed_days_debug, high, low,
                    );
                }

                if target_is_above {
                    if low < worst_adverse_price {
                        worst_adverse_price = low;
                    }

                    if let Some(stop_price) = stop_loss_price.filter(|&price| low <= price) {
                        let elapsed_ms = (step as u64 * interval_ms) as f64;
                        elapsed_days = (elapsed_ms / MILLIS_PER_DAY).max(1.0);
                        outcome = Outcome::StoppedOut {
                            adverse_price: stop_price,
                        };
                        final_price = stop_price;

                        if debug_this_attempt {
                            println!(
                                "  -> STOPPED OUT at price {:.4} after {:.2} days (bull journey)",
                                stop_price, elapsed_days
                            );
                        }
                        break;
                    }

                    if high >= params.end_price {
                        let elapsed_ms = (step as u64 * interval_ms) as f64;
                        let success_days = (elapsed_ms / MILLIS_PER_DAY).max(1.0);
                        elapsed_days = success_days;
                        let elapsed_days_rounded = success_days.ceil() as u16;
                        outcome = Outcome::Success {
                            days_elapsed: elapsed_days_rounded,
                        };
                        days_to_target = Some(elapsed_days_rounded);
                        final_price = params.end_price;

                        if debug_this_attempt {
                            println!(
                                "  -> SUCCESS: target {:.4} reached after {:.2} days (bull journey)",
                                params.end_price, elapsed_days
                            );
                        }
                        break;
                    }
                } else {
                    if high > worst_adverse_price {
                        worst_adverse_price = high;
                    }

                    if let Some(stop_price) = stop_loss_price.filter(|&price| high >= price) {
                        let elapsed_ms = (step as u64 * interval_ms) as f64;
                        elapsed_days = (elapsed_ms / MILLIS_PER_DAY).max(1.0);
                        outcome = Outcome::StoppedOut {
                            adverse_price: stop_price,
                        };
                        final_price = stop_price;

                        if debug_this_attempt {
                            println!(
                                "  -> STOPPED OUT at price {:.4} after {:.2} days (bear journey)",
                                stop_price, elapsed_days
                            );
                        }
                        break;
                    }

                    if low <= params.end_price {
                        let elapsed_ms = (step as u64 * interval_ms) as f64;
                        let success_days = (elapsed_ms / MILLIS_PER_DAY).max(1.0);
                        elapsed_days = success_days;
                        let elapsed_days_rounded = success_days.ceil() as u16;
                        outcome = Outcome::Success {
                            days_elapsed: elapsed_days_rounded,
                        };
                        days_to_target = Some(elapsed_days_rounded);
                        final_price = params.end_price;

                        if debug_this_attempt {
                            println!(
                                "  -> SUCCESS: target {:.4} reached after {:.2} days (bear journey)",
                                params.end_price, elapsed_days
                            );
                        }
                        break;
                    }
                }

                final_price = timeseries.close_prices[idx];
            }

            if elapsed_days <= 0.0 {
                let elapsed_ms = (steps_taken as u64 * interval_ms) as f64;
                if elapsed_ms > 0.0 {
                    elapsed_days = (elapsed_ms / MILLIS_PER_DAY).max(1.0);
                } else {
                    let window_days = params.max_journey_time.as_secs_f64() / 86_400.0;
                    elapsed_days = window_days.max(1.0);
                }
            }

            let max_drawdown_pct = if price_match.close_price > 0.0 {
                if target_is_above {
                    ((price_match.close_price - worst_adverse_price) / price_match.close_price
                        * 100.0)
                        .max(0.0)
                } else {
                    ((worst_adverse_price - price_match.close_price) / price_match.close_price
                        * 100.0)
                        .max(0.0)
                }
            } else {
                0.0
            };

            if debug_this_attempt {
                println!(
                    "--- Attempt summary ---\nsteps_taken: {}\nelapsed_days: {:.2}\nmax_drawdown_pct: {:.2}%\nfinal_price: {:.4}\noutcome: {:?}\n========================\n",
                    steps_taken, elapsed_days, max_drawdown_pct, final_price, outcome,
                );
            }

            outcomes.push(JourneyOutcome {
                start_timestamp_ms: price_match.timestamp_ms,
                start_price: price_match.close_price,
                outcome,
                days_to_target,
                elapsed_days,
                max_drawdown_pct,
                final_price,
            });
        }

        outcomes
    }

    fn compute_stats(&self, outcomes: &[JourneyOutcome], params: &JourneyParams) -> JourneyStats {
        let total_attempts = outcomes.len();
        if total_attempts == 0 {
            return JourneyStats::default();
        }

        let success_count = outcomes.iter().filter(|o| o.is_success()).count();
        let failure_count = total_attempts.saturating_sub(success_count);
        let success_rate = success_count as f64 / total_attempts as f64;

        let mut success_roi_sum = 0.0;
        let mut failure_roi_sum = 0.0;
        let mut success_ann_sum = 0.0;
        let mut failure_ann_sum = 0.0;
        let mut success_samples = 0usize;
        let mut failure_samples = 0usize;
        let mut failure_losses: Vec<f64> = Vec::new();
        let mut failure_drawdowns: Vec<f64> = Vec::new();

        for outcome in outcomes {
            let start_price = outcome.start_price;
            if start_price <= 0.0 {
                continue;
            }

            // Make ROI direction-aware: for bullish journeys (target above start),
            // profits come from price rising; for bearish journeys (target below start),
            // profits come from price falling.
            let direction_up = params.end_price >= params.start_price;
            let roi = if direction_up {
                (outcome.final_price - start_price) / start_price
            } else {
                (start_price - outcome.final_price) / start_price
            };

            match outcome.outcome {
                Outcome::Success { .. } => {
                    success_samples += 1;
                    success_roi_sum += roi;

                    let days = outcome.elapsed_days.max(1.0);
                    success_ann_sum += annualized_roi(roi, days);
                }
                _ => {
                    failure_samples += 1;
                    failure_roi_sum += roi;
                    let days = outcome.elapsed_days.max(1.0);
                    failure_ann_sum += annualized_roi(roi, days);
                    failure_losses.push(roi);
                    failure_drawdowns.push(outcome.max_drawdown_pct);
                }
            }
        }

        let (ci_lower, ci_upper) = wilson_interval(success_count, total_attempts);
        let probability_success = success_rate;
        let probability_failure = 1.0 - probability_success;

        let avg_success_roi = if success_samples > 0 {
            success_roi_sum / success_samples as f64
        } else {
            0.0
        };
        let avg_failure_roi = if failure_samples > 0 {
            failure_roi_sum / failure_samples as f64
        } else {
            0.0
        };
        let avg_success_annualized_roi = if success_samples > 0 {
            success_ann_sum / success_samples as f64
        } else {
            0.0
        };
        let avg_failure_annualized_roi = if failure_samples > 0 {
            failure_ann_sum / failure_samples as f64
        } else {
            0.0
        };

        let expected_annualized_return = (success_rate * avg_success_annualized_roi)
            + ((1.0 - success_rate) * avg_failure_annualized_roi);

        let risk_metrics = calculate_risk_metrics(&failure_losses, &failure_drawdowns);

        let mut expected_value = calculate_expected_value(
            probability_success,
            probability_failure,
            avg_success_annualized_roi,
            avg_failure_annualized_roi,
            avg_success_roi,
            avg_failure_roi,
        );

        if !params.compute_kelly {
            expected_value.kelly_criterion = None;
        }

        JourneyStats {
            total_attempts,
            success_count,
            failure_count,
            success_rate,
            probability_success,
            probability_failure,
            confidence_interval_success: (ci_lower, ci_upper),
            avg_success_roi,
            avg_failure_roi,
            avg_success_annualized_roi,
            avg_failure_annualized_roi,
            expected_annualized_return,
            risk_metrics,
            expected_value,
        }
    }

    /// Convenience wrapper to analyze a journey targeting the nearest edge of a zone.
    pub fn analyze_zone(&self, request: JourneyRequest<'_>) -> Result<JourneyExecution> {
        let target_price = nearest_zone_edge(
            request.current_price,
            request.target.price_bottom,
            request.target.price_top,
        );

        let params = JourneyParams {
            pair: request.pair.to_string(),
            interval_ms: request.interval_ms,
            start_price: request.current_price,
            end_price: target_price,
            max_journey_time: request.max_journey_time,
            start_price_tolerance_pct: request.start_price_tolerance_pct,
            stop_loss_pct: request.stop_loss_pct,
            compute_kelly: request.compute_kelly,
        };

        let start_time = Instant::now();
        let analysis = self.analyze(&params)?;
        let elapsed = start_time.elapsed();

        let direction_up = params.end_price >= params.start_price;

        Ok(JourneyExecution {
            zone_index: request.target.index,
            zone_bottom: request.target.price_bottom,
            zone_top: request.target.price_top,
            target_price,
            direction_up,
            analysis,
            elapsed,
        })
    }

    /// Analyze journeys to the nearest edge of each provided sticky zone.
    #[allow(clippy::too_many_arguments)]
    pub fn analyze_zones(
        &self,
        pair: &str,
        interval_ms: i64,
        current_price: f64,
        targets: &[ZoneTarget],
        tolerance_pct: f64,
        max_journey_time: Duration,
        compute_kelly: bool,
        stop_loss_pct: f64,
    ) -> Result<Vec<JourneyExecution>> {
        let mut executions = Vec::new();

        for target in targets {
            let request = JourneyRequest {
                pair,
                interval_ms,
                current_price,
                target,
                start_price_tolerance_pct: tolerance_pct,
                max_journey_time,
                compute_kelly,
                stop_loss_pct,
            };

            let execution = self.analyze_zone(request)?;
            executions.push(execution);
        }

        Ok(executions)
    }
}

fn annualized_roi(roi: f64, days: f64) -> f64 {
    if !roi.is_finite() || !days.is_finite() {
        return 0.0;
    }

    if roi <= -1.0 {
        return -100.0;
    }

    let days = days.max(1.0);
    let annualized = roi * (365.0 / days);
    (annualized * 100.0).clamp(-10_000.0, 10_000.0)
}

fn calculate_risk_metrics(failure_losses: &[f64], failure_drawdowns: &[f64]) -> RiskMetrics {
    if failure_losses.is_empty() {
        return RiskMetrics::default();
    }

    let mut losses_pct: Vec<f64> = failure_losses.iter().map(|roi| roi * 100.0).collect();
    losses_pct.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mut drawdowns = failure_drawdowns.to_vec();
    drawdowns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let avg_loss = losses_pct.iter().copied().sum::<f64>() / losses_pct.len() as f64;
    let median_loss = percentile(&losses_pct, 0.5);
    let worst_case_loss = *losses_pct.last().unwrap_or(&0.0);
    let avg_drawdown = drawdowns.iter().copied().sum::<f64>() / drawdowns.len() as f64;

    RiskMetrics {
        avg_loss_on_failure: avg_loss,
        median_loss,
        worst_case_loss,
        avg_max_drawdown: avg_drawdown,
    }
}

fn percentile(sorted_values: &[f64], fraction: f64) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }

    let clamped_fraction = fraction.clamp(0.0, 1.0);
    let idx = ((sorted_values.len() - 1) as f64 * clamped_fraction).round() as usize;
    sorted_values.get(idx).copied().unwrap_or(0.0)
}

fn wilson_interval(successes: usize, total: usize) -> (f64, f64) {
    if total == 0 {
        return (0.0, 0.0);
    }

    let z = 1.96_f64;
    let n = total as f64;
    let p = successes as f64 / n;

    let denominator = 1.0 + (z * z / n);
    let center = (p + (z * z) / (2.0 * n)) / denominator;
    let margin = (z * ((p * (1.0 - p) / n) + (z * z) / (4.0 * n * n)).sqrt()) / denominator;

    (center - margin, center + margin)
}

fn calculate_expected_value(
    probability_success: f64,
    probability_failure: f64,
    avg_success_annualized_roi: f64,
    avg_failure_annualized_roi: f64,
    avg_success_roi: f64,
    avg_failure_roi: f64,
) -> ExpectedValue {
    let expected_annualized_return = (probability_success * avg_success_annualized_roi)
        + (probability_failure * avg_failure_annualized_roi);

    let kelly = if probability_success > 0.0
        && probability_success < 1.0
        && avg_success_roi > 0.0
        && avg_failure_roi < 0.0
    {
        let win_loss_ratio = avg_success_roi / avg_failure_roi.abs();
        if win_loss_ratio > 0.0 {
            let q = 1.0 - probability_success;
            let fraction = (probability_success * win_loss_ratio - q) / win_loss_ratio;
            Some(fraction.max(0.0))
        } else {
            None
        }
    } else {
        None
    };

    ExpectedValue {
        expected_annualized_return,
        kelly_criterion: kelly,
    }
}

fn nearest_zone_edge(current_price: f64, price_bottom: f64, price_top: f64) -> f64 {
    if current_price <= price_bottom {
        price_bottom
    } else if current_price >= price_top {
        price_top
    } else {
        let dist_to_bottom = (current_price - price_bottom).abs();
        let dist_to_top = (price_top - current_price).abs();
        if dist_to_bottom <= dist_to_top {
            price_bottom
        } else {
            price_top
        }
    }
}
