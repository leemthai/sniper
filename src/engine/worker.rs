use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

// Only import thread on non-WASM target
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

use super::messages::{JobRequest, JobResult};

use crate::analysis::adaptive::AdaptiveParameters;
use crate::analysis::horizon_profiler::generate_profile;
use crate::analysis::market_state::MarketState;
use crate::analysis::pair_analysis::pair_analysis_pure;
use crate::analysis::scenario_simulator::{ScenarioSimulator, SimulationResult};

use crate::config::AnalysisConfig;

use crate::domain::price_horizon;

use crate::models::OhlcvTimeSeries;
use crate::models::cva::CVACore;
use crate::models::horizon_profile::HorizonProfile;
use crate::models::timeseries::find_matching_ohlcv;
use crate::models::trading_view::{SuperZone, TradeDirection, TradeOpportunity, TradeVariant};

use crate::TradingModel;



use crate::utils::maths_utils::duration_to_candles;
use crate::utils::time_utils::AppInstant;

#[cfg(debug_assertions)]
use {crate::ui::ui_text::UI_TEXT, crate::utils::maths_utils::{calculate_percent_diff, calculate_expected_roi_pct}};

/// NATIVE ONLY: Spawns a background thread to process jobs
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_worker_thread(rx: Receiver<JobRequest>, tx: Sender<JobResult>) {
    thread::spawn(move || {
        while let Ok(req) = rx.recv() {
            process_request_sync(req, tx.clone());
        }
    });
}

/// WASM ONLY: No-op.
/// The Engine holds the receiver and processes jobs manually in the update loop.
#[cfg(target_arch = "wasm32")]
pub fn spawn_worker_thread(_rx: Receiver<JobRequest>, _tx: Sender<JobResult>) {
    // Do nothing.
}

// Helper: Runs Ghost Runner simulations for all visible sticky zones
fn run_pathfinder_simulations(
    ohlcv: &OhlcvTimeSeries,
    sticky_zones: &[SuperZone],
    current_price: f64,
    config: &AnalysisConfig,
) -> Vec<TradeOpportunity> {
    let mut opportunities = Vec::new();

    // 1. Setup & Adaptive Lookback
    let ph_pct = config.price_horizon.threshold_pct;
    let trend_lookback = AdaptiveParameters::calculate_trend_lookback_candles(ph_pct);
    let current_idx = ohlcv.klines().saturating_sub(1);

    let interval_ms = ohlcv.pair_interval.interval_ms;

    // --- NEW: USE CENTRALIZED METHOD ---
    // Calculate volatility over the lookback period (Recent Context)
    let start_vol_idx = current_idx.saturating_sub(trend_lookback);
    let avg_volatility = ohlcv.calculate_volatility_in_range(start_vol_idx, current_idx);

    // B. Calculate Duration (Time) using Adaptive Logic
    let duration_time = AdaptiveParameters::calculate_dynamic_journey_duration(
        ph_pct,
        avg_volatility,
        ohlcv.pair_interval.interval_ms,
    );

    // C. Convert to Candles for Simulation
    let duration_candles = duration_to_candles(duration_time, interval_ms);
    // -------------------------------------

    // #[cfg(debug_assertions)]
    // log::info!(
    //     "PATHFINDER START for {}: Scanning {} zones. Lookback: {} candles. Max Duration: {:?} ({} candles, Vol: {:.3}%. PH: {:.2}%)",
    //     ohlcv.pair_interval.name().to_string(),
    //     sticky_zones.len(),
    //     trend_lookback,
    //     TimeUtils::format_duration(duration_time.as_millis() as i64),
    //     duration_candles,
    //     avg_volatility * 100.0,
    //     ph_pct * 100.
    // );

    // 2. Heavy Lift: Find Historical Matches ONCE
    let (historical_matches, current_market_state) =
        match ScenarioSimulator::find_historical_matches(
            ohlcv,
            current_idx,
            trend_lookback,
            duration_candles,
            config.journey.sample_count,
            &config.similarity,
        ) {
            Some((m, s)) if !m.is_empty() => (m, s),
            _ => {
                // #[cfg(debug_assertions)]
                // log::warn!(
                //     "PATHFINDER ABORT: Insufficient data or matches (Lookback {}).",
                //     trend_lookback
                // );
                return Vec::new();
            }
        };
    // ----------------

    // 3. Scan Zones
    for (i, zone) in sticky_zones.iter().enumerate() {
        // IDIOMATIC: Determine Setup using Option<(Price, Direction)>
        // CHANGED: Target is now zone.price_center for both cases
        let setup = if current_price < zone.price_bottom {
            Some((zone.price_center, TradeDirection::Long))
        } else if current_price > zone.price_top {
            Some((zone.price_center, TradeDirection::Short))
        } else {
            None
        };

        if let Some((target_price, direction)) = setup {
            // 4. Run Tournament to find best Stop Loss
            if let Some((best_result, best_stop, variants)) = run_stop_loss_tournament(
                ohlcv,
                &historical_matches,
                current_market_state,
                current_price,
                target_price,
                direction,
                duration_candles,
                config.journey.risk_reward_tests,
                config.journey.profile.min_roi,
                i, // zone index for debug logging
            ) {
                // Calculate Average Duration in MS
                let avg_candles = best_result.avg_duration;
                let avg_ms = (avg_candles * interval_ms as f64).round() as i64;

                opportunities.push(TradeOpportunity {
                    pair_name: ohlcv.pair_interval.name().to_string(),
                    start_price: current_price,
                    target_zone_id: zone.id,
                    direction,
                    target_price,
                    stop_price: best_stop,
                    max_duration_ms: duration_time.as_millis() as i64,
                    avg_duration_ms: avg_ms,
                    simulation: best_result,
                    variants,
                });
            }
        }
    }
    opportunities
}

/// Helper function to find the optimal Stop Loss for a given target
fn run_stop_loss_tournament(
    ohlcv: &OhlcvTimeSeries,
    historical_matches: &[(usize, f64)],
    current_state: MarketState,
    current_price: f64,
    target_price: f64,
    direction: TradeDirection,
    duration_candles: usize,
    risk_tests: &[f64],
    min_roi_pct: f64,
    _zone_idx: usize,
) -> Option<(SimulationResult, f64, Vec<TradeVariant>)> {

    let mut best_roi = f64::NEG_INFINITY;
    let mut best_result: Option<(SimulationResult, f64, f64)> = None; // (Result, Stop, Ratio)
    let mut valid_variants= Vec::new();

    let target_dist_abs = (target_price - current_price).abs();

    // 1. Safety Check: Volatility Floor. Ensure sl is not triggered by random noise (2x Volatility)
    let vol_floor_pct = current_state.volatility_pct * 2.0;
    let min_stop_dist = current_price * vol_floor_pct;

    // Logging setup
    #[cfg(debug_assertions)]
    // let debug = _zone_idx < 3;
    let debug = false;// Turn debugging off for now
    #[cfg(debug_assertions)]
    if debug {
        log::info!(
            "🔍 Analyzing Zone {} ({}): Testing {} SL candidates. Volatility Floor: {:.2}% (${:.4})",
            _zone_idx,
            direction,
            risk_tests.len(),
            vol_floor_pct * 100.0,
            min_stop_dist
        );
    }

    for &ratio in risk_tests {
        // 2. Calculate Candidate Stop
        let stop_dist = target_dist_abs / ratio;

        if stop_dist < min_stop_dist {
            #[cfg(debug_assertions)]
            if debug {
                log::info!(
                    "   [R:R {:.1}] 🛑 SKIPPED: Stop distance {:.4} < Volatility Floor {:.4}",
                    ratio,
                    stop_dist,
                    min_stop_dist
                );
            }
            continue;
        }

        let candidate_stop = match direction {
            TradeDirection::Long => current_price - stop_dist,
            TradeDirection::Short => current_price + stop_dist,
        };

        // 3. Simulation
        if let Some(result) = ScenarioSimulator::analyze_outcome(
            ohlcv,
            historical_matches,
            current_state,
            current_price,
            target_price,
            candidate_stop,
            duration_candles,
            direction,
        ) {
            // New: Decision metric: use the real average PnL (include Timeouts)
            let true_roi_pct = result.avg_pnl_pct * 100.0;

            #[cfg(debug_assertions)]
            let binary_roi = calculate_expected_roi_pct(
                current_price,
                target_price,
                candidate_stop,
                result.success_rate,
            );

            // MWT CHECK: Does this variant meet the minimum threshold?
            let is_worthwhile = true_roi_pct >= min_roi_pct;

          if is_worthwhile {
                // Store this variant
                valid_variants.push(TradeVariant {
                    ratio,
                    stop_price: candidate_stop,
                    roi: true_roi_pct,
                    simulation: result.clone(),
                });
                
                // Track Best
                if true_roi_pct > best_roi {
                    best_roi = true_roi_pct;
                    best_result = Some((result.clone(), candidate_stop, ratio));
                }
            }

            #[cfg(debug_assertions)]
            if debug {
                let risk_pct = calculate_percent_diff(candidate_stop, current_price);
                let status_icon = if is_worthwhile { "✅" } else { "🔻" }; // Check vs Low
                log::debug!(
                    "   [R:R {:.1}] {} Stop: {:.4} | {}: {:.1}% | ROI: {:+.2}% (Bin: {:+.2}%) | Risk: {:.2}%",
                    ratio,
                    status_icon,
                    candidate_stop,
                    UI_TEXT.label_success_rate,
                    result.success_rate * 100.0,
                    true_roi_pct,
                    binary_roi,
                    risk_pct,
                );
            }

            // Optimize for True ROI, BUT only if it meets MWT
            if is_worthwhile && true_roi_pct > best_roi {
                best_roi = true_roi_pct;
                best_result = Some((result.clone(), candidate_stop, ratio));
            }

        }
    }

    // Return Winner + Count
    if let Some((res, stop, _ratio)) = best_result {
        #[cfg(debug_assertions)]
        if debug {
            log::info!(
                "   🏆 WINNER: R:R {:.1} with ROI {:+.2}% ({:?} variants)",
                _ratio,
                best_roi,
                valid_variants
            );
        }
        Some((res, stop, valid_variants))
    } else {
        None
    }
}

pub fn process_request_sync(req: JobRequest, tx: Sender<JobResult>) {
    let ph_pct = req.config.price_horizon.threshold_pct * 100.0;
    let base_label = format!("{} @ {:.2}%", req.pair_name, ph_pct);

    crate::trace_time!(&format!("Total JOB [{}]", base_label), 5000, {
        let start = AppInstant::now();

        // 1. Resolve Inputs
        let price = resolve_analysis_price(&req);

        // 2. Exact Count (Range Logic)
        let count = crate::trace_time!(&format!("1. Exact Count [{}]", base_label), 500, {
            calculate_exact_candle_count(&req, price)
        });

        let full_label = format!("{} ({} candles)", base_label, count);

        // 3. CVA Logic
        let result_cva = crate::trace_time!(&format!("2. CVA Calc [{}]", full_label), 1000, {
            pair_analysis_pure(req.pair_name.clone(), &req.timeseries, price, &req.config)
        });

        // 4. Profiler (Smart Cache)
        let profile = crate::trace_time!(&format!("3. Profiler [{}]", full_label), 1000, {
            get_or_generate_profile(&req, price)
        });

        let elapsed = start.elapsed().as_millis();

        // 5. Final Assembly
        let response = match result_cva {
            Ok(cva) => build_success_result(&req, cva, profile, price, count, elapsed),
            Err(e) => JobResult {
                pair_name: req.pair_name.clone(),
                duration_ms: elapsed,
                result: Err(e.to_string()),
                cva: None,
                profile: Some(profile),
                candle_count: count,
            },
        };

        let _ = tx.send(response);
    });
}

fn resolve_analysis_price(req: &JobRequest) -> f64 {
    req.current_price.unwrap_or_else(|| {
        if let Ok(ts) = find_matching_ohlcv(
            &req.timeseries.series_data,
            &req.pair_name,
            req.config.interval_width_ms,
        ) {
            ts.close_prices.last().copied().unwrap_or(0.0)
        } else {
            0.0
        }
    })
}

fn calculate_exact_candle_count(req: &JobRequest, price: f64) -> usize {
    let ohlcv_result = find_matching_ohlcv(
        &req.timeseries.series_data,
        &req.pair_name,
        req.config.interval_width_ms,
    );

    if let Ok(ohlcv) = ohlcv_result {
        let (ranges, _) =
            price_horizon::auto_select_ranges(ohlcv, price, &req.config.price_horizon);
        ranges.iter().map(|(s, e)| e - s).sum()
    } else {
        0
    }
}

fn get_or_generate_profile(req: &JobRequest, price: f64) -> HorizonProfile {
    if let Some(existing) = &req.existing_profile {
        let cfg = &req.config.price_horizon;
        let price_match = (existing.base_price - price).abs() < f64::EPSILON;
        let min_match = (existing.min_pct - cfg.min_threshold_pct).abs() < f64::EPSILON;
        let max_match = (existing.max_pct - cfg.max_threshold_pct).abs() < f64::EPSILON;

        if price_match && min_match && max_match {
            #[cfg(debug_assertions)]
            log::info!("Worker: Reusing Cached Profile for {}", req.pair_name);
            return existing.clone();
        }
    }

    generate_profile(
        &req.pair_name,
        &req.timeseries,
        price,
        &req.config.price_horizon,
    )
}

fn build_success_result(
    req: &JobRequest,
    cva: CVACore,
    profile: HorizonProfile,
    price: f64,
    count: usize,
    elapsed: u128,
) -> JobResult {
    // 1. Fetch OHLCV (Must exist if CVA succeeded)
    let ohlcv = find_matching_ohlcv(
        &req.timeseries.series_data,
        &req.pair_name,
        req.config.interval_width_ms,
    )
    .expect("OHLCV data missing despite CVA success");

    let cva_arc = Arc::new(cva);

    // 2. Create Model
    let mut model = TradingModel::from_cva(cva_arc.clone(), profile.clone(), ohlcv, &req.config);

    // 3. Run Pathfinder
    // (Note: Requires the helper function 'run_pathfinder_simulations' we added earlier in this file)
    let opps = crate::trace_time!(&format!("5. Pathfinder [{}]", req.pair_name), 500, {
        run_pathfinder_simulations(ohlcv, &model.zones.sticky_superzones, price, &req.config)
    });

    // --- DEBUG LOGGING ---
    // #[cfg(debug_assertions)]
    // if !opps.is_empty() {
    //     // Just log the top one to check sanity
    //     if let Some(best) = opps
    //         .iter()
    //         .max_by(|a, b| a.expected_roi().partial_cmp(&b.expected_roi()).unwrap())
    //     {
    //         // log::info!(
    //         //     "🎯 PATHFINDER [{}]: Found {} opps. Best: {} to {:.2} ({}: {:.1}% | R:R: {:.2} | Samples: {})",
    //         //     req.pair_name,
    //         //     opps.len(),
    //         //     best.direction,
    //         //     best.target_price,
    //         //     UI_TEXT.label_success_rate,
    //         //     best.simulation.success_rate * 100.0,
    //         //     best.simulation.risk_reward_ratio,
    //         //     best.simulation.sample_size
    //         // );
    //     }
    // } else {
    //     // log::info!(
    //     //     "PATHFINDER [{}]: No valid setups found (Price might be inside a zone/mud).",
    //     //     req.pair_name
    //     // );
    // }

    model.opportunities = opps;

    JobResult {
        pair_name: req.pair_name.clone(),
        duration_ms: elapsed,
        result: Ok(Arc::new(model)),
        cva: Some(cva_arc),
        profile: Some(profile),
        candle_count: count,
    }
}
