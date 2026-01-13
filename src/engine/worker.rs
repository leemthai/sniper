use std::cmp::Ordering;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

// Only import thread crate on non-WASM target
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

use uuid::Uuid;

use super::messages::{JobMode, JobRequest, JobResult};

use crate::analysis::adaptive::AdaptiveParameters;
use crate::analysis::horizon_profiler;
use crate::analysis::market_state::MarketState;
use crate::analysis::pair_analysis::pair_analysis_pure;
use crate::analysis::scenario_simulator::{ScenarioSimulator, SimulationResult};

use crate::config::{AnalysisConfig, TradeProfile};

use crate::data::timeseries::TimeSeriesCollection;

use crate::domain::price_horizon;

use crate::models::OhlcvTimeSeries;
use crate::models::cva::CVACore;
use crate::models::horizon_profile::HorizonProfile;
use crate::models::timeseries::find_matching_ohlcv;
use crate::models::trading_view::{TradeDirection, TradeOpportunity, TradeVariant};

use crate::TradingModel;

use crate::utils::maths_utils::duration_to_candles;
use crate::utils::time_utils::{AppInstant, TimeUtils};

use crate::utils::maths_utils::{
    calculate_annualized_roi, is_opportunity_worthwhile,
};
#[cfg(debug_assertions)]
use {crate::ui::ui_text::UI_TEXT, crate::utils::maths_utils::{calculate_expected_roi_pct, calculate_percent_diff}};

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

struct CandidateResult {
    score: f64,
    opportunity: TradeOpportunity,
    #[allow(dead_code)]
    source_desc: String,
}

/// Helper: Runs the simulation tournament for a specific target price.
fn simulate_target(
    target_price: f64,
    source_id_suffix: &str,
    ohlcv: &OhlcvTimeSeries,
    matches: &[(usize, f64)],
    current_state: MarketState,
    current_price: f64,
    config: &AnalysisConfig,
    duration_candles: usize,
    duration_ms: u64,
    ph_pct: f64,
) -> Option<CandidateResult> {
    let profile = &config.journey.profile;

    // Determine Direction
    let direction = if target_price > current_price {
        TradeDirection::Long
    } else {
        TradeDirection::Short
    };

    // Run Stop Loss Tournament
    let best_sl_opt = run_stop_loss_tournament(
        ohlcv,
        matches,
        current_state,
        current_price,
        target_price,
        direction,
        duration_candles,
        config.journey.risk_reward_tests,
        profile,
        config.interval_width_ms,
        0,
    );

    if let Some((result, stop_price, variants)) = best_sl_opt {
        let avg_duration_ms = (result.avg_candle_count * config.interval_width_ms as f64) as i64;
        // Calculate Score (The Judge)
        // 2. Calculate Score (The Judge) using CORRECT UNITS (ms)
        let score = profile.goal.calculate_score(
            result.avg_pnl_pct * 100.0, // ROI
            avg_duration_ms as f64,
            profile.weight_roi,
            profile.weight_aroi,
        );

        // Generate ID
        let unique_string = format!(
            "{}_{}_{}_{}",
            ohlcv.pair_interval.name(),
            source_id_suffix,
            direction,
            profile.goal
        );
        let uuid = Uuid::new_v5(&Uuid::NAMESPACE_OID, unique_string.as_bytes()).to_string();

        let opp = TradeOpportunity {
            id: uuid,
            created_at: TimeUtils::now_timestamp_ms(),
            source_ph: ph_pct,
            pair_name: ohlcv.pair_interval.name().to_string(),
            target_zone_id: 0, // 0 indicates "Generated Target" (Not a Zone)
            direction,
            start_price: current_price,
            target_price,
            stop_price,
            max_duration_ms: duration_ms as i64,
            avg_duration_ms,
            simulation: result,
            variants,
        };

        return Some(CandidateResult {
            score,
            opportunity: opp,
            source_desc: source_id_suffix.to_string(),
        });
    }

    None
}

/// Phase C: Filters opportunities using the "Regional Championship" strategy.
/// 1. Divides the price range into N regions.
/// 2. Finds the best trade in each region (Local Winner).
/// 3. Filters Local Winners against the Global Best Score (Qualifying Round).
fn apply_diversity_filter(
    candidates: Vec<CandidateResult>, 
    config: &AnalysisConfig,
    _pair_name: &str,
    range_min: f64,
    range_max: f64,
) -> Vec<TradeOpportunity> {
    if candidates.is_empty() { return Vec::new(); }

    let opt_config = &config.journey.optimization;
    let regions = opt_config.diversity_regions;
    let cutoff_ratio = opt_config.diversity_cut_off;

    // 1. Identify Global Max Score (The Gold Standard)
    // We need this to determine the qualifying time.
    let global_best_score = candidates
        .iter()
        .map(|c| c.score)
        .fold(f64::NEG_INFINITY, f64::max);

    let qualifying_score = global_best_score * cutoff_ratio;

    #[cfg(debug_assertions)]
    log::info!(
        "🏆 REGIONAL CHAMPIONSHIP [{}]: {} Candidates. Global Best: {:.2} | Qualifying: {:.2} ({:.0}%)",
        _pair_name,
        candidates.len(),
        global_best_score,
        qualifying_score,
        cutoff_ratio * 100.0
    );

    // 2. Hold Local Tournaments
    // Vector of Options to hold the winner of each region
    let mut regional_winners: Vec<Option<CandidateResult>> = (0..regions).map(|_| None).collect();
    let total_range = range_max - range_min;
    let bucket_size = total_range / regions as f64;

    for cand in candidates {
        // Determine Region Index
        let offset = cand.opportunity.target_price - range_min;
        // Clamp index to 0..regions-1 (handle edge cases where price == max)
        let idx = ((offset / bucket_size).floor() as usize).min(regions - 1);

        // Battle for the Region
        match &mut regional_winners[idx] {
            None => {
                // Uncontested (so far)
                regional_winners[idx] = Some(cand);
            },
            Some(current_champ) => {
                // Challenger vs Champion
                // We respect the Strategy because 'score' is calculated using the user's goal
                if cand.score > current_champ.score {
                    regional_winners[idx] = Some(cand);
                }
            }
        }
    }

    // 3. The Qualifiers (Filter & Collect)
    let mut final_results = Vec::new();

    for (_i, winner_opt) in regional_winners.into_iter().enumerate() {
        if let Some(winner) = winner_opt {
            // Check if they beat the qualifying time
            if winner.score >= qualifying_score {
                #[cfg(debug_assertions)]
                log::info!(
                    "   ✅ Region #{} Winner [{}]: Score {:.2} | Target ${:.2} (Qualified)",
                    _i, winner.source_desc, winner.score, winner.opportunity.target_price
                );
                final_results.push(winner.opportunity);
            } else {
                #[cfg(debug_assertions)]
                log::debug!(
                    "   ❌ Region #{} Winner [{}]: Score {:.2} (Failed Qualifier < {:.2})",
                    _i, winner.source_desc, winner.score, qualifying_score
                );
            }
        }
    }

    // 4. Final Sort (Best of the Best first)
    final_results.sort_by(|a, b| {
        // Recalculate score for sorting or store it? 
        // TradeOpportunity doesn't store the raw score, but we can re-calc cheaply or trust the input order.
        // Let's re-calc to be safe and strict sort.
        let score_a = a.calculate_quality_score(&config.journey.profile);
        let score_b = b.calculate_quality_score(&config.journey.profile);
        score_b.partial_cmp(&score_a).unwrap_or(Ordering::Equal)
    });
    
    // Hard Limit (if configured lower than region count)
    if final_results.len() > opt_config.max_results {
        final_results.truncate(opt_config.max_results);
    }

    final_results
}

// MAIN FUNCTION: Runs Scout & Drill optimization
fn run_pathfinder_simulations(
    ohlcv: &OhlcvTimeSeries,
    current_price: f64,
    config: &AnalysisConfig,
) -> Vec<TradeOpportunity> {

    if current_price <= 0.0 {
        return Vec::new();
    }

    let pair_name = ohlcv.pair_interval.name();

    let opt_config = &config.journey.optimization;

    // 1. Setup & Adaptive Parameters
    let ph_pct = config.price_horizon.threshold_pct;

    // Volatility Context: Use Configured Lookback
    let max_idx = ohlcv.klines().saturating_sub(1);
    let vol_lookback = opt_config.volatility_lookback.min(max_idx);
    let start_vol = ohlcv.klines().saturating_sub(vol_lookback);
    let avg_volatility = ohlcv.calculate_volatility_in_range(start_vol, ohlcv.klines());

    let trend_lookback = AdaptiveParameters::calculate_trend_lookback_candles(ph_pct);
    let duration = AdaptiveParameters::calculate_dynamic_journey_duration(
        ph_pct,
        avg_volatility,
        config.interval_width_ms,
    );
    let duration_candles = duration_to_candles(duration, config.interval_width_ms);
    let duration_ms = duration.as_millis() as u64;

    // 2. Find Matches (SIMD)
    // Correct usage: ScenarioSimulator imported at top
    let matches_opt = ScenarioSimulator::find_historical_matches(
        ohlcv.pair_interval.name(),
        ohlcv,
        ohlcv.klines().saturating_sub(1),
        &config.similarity,
        config.journey.sample_count,
        trend_lookback,
        duration_candles,
    );

    let (matches, current_state) = match matches_opt {
        Some((m, s)) if !m.is_empty() => (m, s),
        _ => return Vec::new(),
    };

    let (ph_min, ph_max) = price_horizon::calculate_price_range(current_price, ph_pct);
    #[cfg(debug_assertions)]
    log::info!(
        "🎯 PATHFINDER START [{}] Price: ${:.4} | PH Range: ${:.4} - ${:.4} ({:.2}%) | Vol: {:.3}%",
        pair_name,
        current_price,
        ph_min,
        ph_max,
        ph_pct * 100.0,
        avg_volatility * 100.0
    );

    let mut all_results: Vec<CandidateResult> = Vec::new();

    // --- PHASE A: THE SCOUTS (Coarse Grid) ---
    // Use Configured Steps
    let steps = opt_config.scout_steps;

    // 1. Long Scouts (Current -> Max)
    // Use Configured Buffer
    let long_start = current_price * (1.0 + opt_config.price_buffer_pct);
    if ph_max > long_start {
        let range = ph_max - long_start;
        let step_size = range / steps as f64;

        for i in 0..=steps {
            let target = long_start + (i as f64 * step_size);
            if let Some(res) = simulate_target(
                target,
                &format!("scout_long_{}", i),
                ohlcv,
                &matches,
                current_state,
                current_price,
                config,
                duration_candles,
                duration_ms,
                ph_pct,
            ) {
                all_results.push(res);
            }
        }
    }

    // 2. Short Scouts (Min -> Current)
    let short_end = current_price * (1.0 - opt_config.price_buffer_pct);
    if ph_min < short_end {
        let range = short_end - ph_min;
        let step_size = range / steps as f64;

        for i in 0..=steps {
            let target = ph_min + (i as f64 * step_size);
            if let Some(res) = simulate_target(
                target,
                &format!("scout_short_{}", i),
                ohlcv,
                &matches,
                current_state,
                current_price,
                config,
                duration_candles,
                duration_ms,
                ph_pct,
            ) {
                all_results.push(res);
            }
        }
    }

    #[cfg(debug_assertions)]
    log::info!(
        "🔍 SCOUT PHASE [{}]: Found {} viable candidates from grid search.",
        pair_name,
        all_results.len()
    );

    // --- PHASE B: THE DRILL (Local Optimization) ---
    // Identify the Top N Scouts and look around them for better peaks

    // Sort current results by score
    all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
    #[cfg(debug_assertions)]
    {
        log::info!("🔍 SCOUT PHASE COMPLETE [{}]. Top 5 Candidates:", pair_name);
        for (i, c) in all_results.iter().take(5).enumerate() {
            log::info!(
                "   #{}: [{}] Target ${:.4} | Score {:.2}",
                i + 1,
                c.source_desc,
                c.opportunity.target_price,
                c.score
            );
        }
    }

    // Take Top N Scouts to drill into
    let top_scouts_count = opt_config.drill_top_n.min(all_results.len());
    let mut drill_results: Vec<CandidateResult> = Vec::new();

    // Define Drill Parameters
    // Search +/- offset of the original grid step size
    let grid_step_pct = (ph_max - ph_min) / current_price / steps as f64;
    let drill_offset_pct = grid_step_pct * opt_config.drill_offset_factor;

    #[cfg(debug_assertions)]
    log::info!(
        "⛏️ DRILL PHASE [{}] [Strategy: {}]: Refining Top {} scouts (Offset: {:.4}%)",
        pair_name,
        config.journey.profile.goal,
        top_scouts_count,
        drill_offset_pct * 100.0
    );

    for i in 0..top_scouts_count {
        let scout = &all_results[i];
        let base_target = scout.opportunity.target_price;

        // Try slightly higher
        let target_high = base_target * (1.0 + drill_offset_pct);
        if let Some(res) = simulate_target(
            target_high,
            &format!("drill_{}_up", i),
            ohlcv,
            &matches,
            current_state,
            current_price,
            config,
            duration_candles,
            duration_ms,
            ph_pct,
        ) {
            drill_results.push(res);
        }

        // Try slightly lower
        let target_low = base_target * (1.0 - drill_offset_pct);
        if let Some(res) = simulate_target(
            target_low,
            &format!("drill_{}_down", i),
            ohlcv,
            &matches,
            current_state,
            current_price,
            config,
            duration_candles,
            duration_ms,
            ph_pct,
        ) {
            drill_results.push(res);
        }
    }

    // Add drill results to the pool
    #[cfg(debug_assertions)]
    if !drill_results.is_empty() {
        log::info!(
            "   -> [{}] Drill found {} better micro-adjustments.",
            pair_name, // <--- Add pair_name
            drill_results.len()
        );
    }

    // Add drill results to the pool
    all_results.extend(drill_results);

    // --- PHASE C: DIVERSITY FILTER (Select Winners) ---
    apply_diversity_filter(all_results, config, pair_name, ph_min, ph_max)
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
    profile: &TradeProfile,
    interval_ms: i64, // <--- NEW PARAMETER
    _zone_idx: usize,
) -> Option<(SimulationResult, f64, Vec<TradeVariant>)> {
    let mut best_score = f64::NEG_INFINITY; // Track Score, not just ROI
    let mut best_result: Option<(SimulationResult, f64, f64)> = None; // (Result, Stop, Ratio)
    let mut valid_variants = Vec::new();

    let target_dist_abs = (target_price - current_price).abs();

    // 1. Safety Check: Volatility Floor.
    let vol_floor_pct = current_state.volatility_pct * 2.0;
    let min_stop_dist = current_price * vol_floor_pct;

    // Logging setup
    #[cfg(debug_assertions)]
    let debug = false;
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
            // Metrics
            let roi = result.avg_pnl_pct * 100.0;

            // --- FIX: UNIT CONVERSION (Candles -> MS) ---
            let duration_real_ms = result.avg_candle_count * interval_ms as f64;

            // Calculate AROI for the Gatekeeper & Judge
            let aroi = calculate_annualized_roi(roi, duration_real_ms);

            #[cfg(debug_assertions)]
            let binary_roi = calculate_expected_roi_pct(
                result.success_rate,
                result.avg_pnl_pct,
                result.success_rate,
                (current_price - candidate_stop).abs() / current_price * 100.0,
            );

            // GATEKEEPER: Check both ROI and AROI against profile
            let is_worthwhile =
                is_opportunity_worthwhile(roi, aroi, profile.min_roi, profile.min_aroi);

            if is_worthwhile {
                // Store this variant
                valid_variants.push(TradeVariant {
                    ratio,
                    stop_price: candidate_stop,
                    roi,
                    simulation: result.clone(),
                });

                // JUDGE: Calculate Score based on Strategy (using CORRECT Time units)
                let score = profile.goal.calculate_score(
                    roi,
                    duration_real_ms,
                    profile.weight_roi,
                    profile.weight_aroi,
                );

                // Track Best
                if score > best_score {
                    best_score = score;
                    best_result = Some((result.clone(), candidate_stop, ratio));
                }
            }

            #[cfg(debug_assertions)]
            if debug {
                let risk_pct = calculate_percent_diff(candidate_stop, current_price);
                let status_icon = if is_worthwhile { "✅" } else { "🔻" };
                log::debug!(
                    "   [R:R {:.1}] {} Stop: {:.4} | {}: {:.1}% | ROI: {:+.2}% (Bin: {:+.2}%) | AROI: {:+.0}% | Risk: {:.2}%",
                    ratio,
                    status_icon,
                    candidate_stop,
                    UI_TEXT.label_success_rate,
                    result.success_rate * 100.0,
                    roi,
                    binary_roi,
                    aroi,
                    risk_pct,
                );
            }
        }
    }

    // Return Winner + Count
    if let Some((res, stop, _ratio)) = best_result {
        #[cfg(debug_assertions)]
        if debug {
            log::info!(
                "   🏆 WINNER: R:R {:.1} with Score {:.2} ({:?} variants)",
                _ratio,
                best_score,
                valid_variants.len()
            );
        }
        Some((res, stop, valid_variants))
    } else {
        None
    }
}

pub fn process_request_sync(mut req: JobRequest, tx: Sender<JobResult>) {
    // 1. ACQUIRE DATA (Clone & Release)
    let ts_local = match fetch_local_timeseries(&req) {
        Ok(ts) => ts,
        Err(e) => {
            let _ = tx.send(JobResult {
                pair_name: req.pair_name.clone(),
                duration_ms: 0,
                result: Err(e),
                cva: None,
                profile: None,
                candle_count: 0,
            });
            return;
        }
    };

    // 2. AUDIT DATA (The Proof)
    // We check the *Cloned* data to ensure it has the latest updates
    // audit_worker_data(&req, &ts_local);

    // 3. AUTO-TUNE (Optional)
    if req.mode == JobMode::AutoTune {
        perform_auto_tune(&mut req, &ts_local);
    }

    // 4. EXECUTE ANALYSIS
    perform_standard_analysis(&req, &ts_local, tx);
}

fn fetch_local_timeseries(req: &JobRequest) -> Result<TimeSeriesCollection, String> {
    let ts_guard = req
        .timeseries
        .read()
        .map_err(|_| "Failed to acquire RwLock".to_string())?;

    let target_pair = &req.pair_name;
    let interval = req.config.interval_width_ms;

    // Find and clone specifically what we need
    if let Ok(series) = find_matching_ohlcv(&ts_guard.series_data, target_pair, interval) {
        Ok(TimeSeriesCollection {
            series_data: vec![series.clone()],
            name: "Worker Local Clone".to_string(),
            version: 1.0,
        })
    } else {
        Err(format!("Worker: No data found for {}", req.pair_name))
    }
}

fn perform_auto_tune(req: &mut JobRequest, ts_collection: &TimeSeriesCollection) {
    let candidates = vec![0.005, 0.020, 0.070, 0.200, 0.500];
    let mut best_ph = req.config.price_horizon.threshold_pct;
    let mut best_score = f64::NEG_INFINITY;

    #[cfg(debug_assertions)]
    log::info!("AUTO-TUNE [{}] Starting Spectrum Scan...", req.pair_name);

    for ph in candidates {
        req.config.price_horizon.threshold_pct = ph;

        if let Some((score, _model)) = run_test_analysis(req, ts_collection) {
            if score > best_score {
                best_score = score;
                best_ph = ph;
            }
        }
    }

    #[cfg(debug_assertions)]
    log::info!(
        "AUTO-TUNE [{}] Winner: {:.2}% (Score: {:.2})",
        req.pair_name,
        best_ph * 100.0,
        best_score
    );

    req.config.price_horizon.threshold_pct = best_ph;
    req.mode = JobMode::Standard;
}

fn perform_standard_analysis(
    req: &JobRequest,
    ts_collection: &TimeSeriesCollection,
    tx: Sender<JobResult>,
) {
    let ph_pct = req.config.price_horizon.threshold_pct * 100.0;
    let base_label = format!("{} @ {:.2}%", req.pair_name, ph_pct);

    crate::trace_time!(&format!("Total JOB [{}]", base_label), 10_000, {
        let start = AppInstant::now();

        // 1. Price
        let price = resolve_analysis_price(req, ts_collection);

        // 2. Count
        let count = crate::trace_time!(&format!("1. Exact Count [{}]", base_label), 2_000, {
            calculate_exact_candle_count(req, ts_collection, price)
        });

        let full_label = format!("{} ({} candles)", base_label, count);

        // 3. CVA
        let result_cva = crate::trace_time!(&format!("2. CVA Calc [{}]", full_label), 3_000, {
            pair_analysis_pure(req.pair_name.clone(), ts_collection, price, &req.config)
        });

        // 4. Profiler
        let profile = crate::trace_time!(&format!("3. Profiler [{}]", full_label), 2_000, {
            get_or_generate_profile(req, ts_collection, price)
        });

        let elapsed = start.elapsed().as_millis();

        // 5. Result Construction
        let response = match result_cva {
            Ok(cva) => {
                build_success_result(req, ts_collection, cva, profile, price, count, elapsed)
            }
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

// Used by AutoTune to score a specific PH setting
fn run_test_analysis(
    req: &JobRequest,
    ts_collection: &TimeSeriesCollection,
) -> Option<(f64, TradingModel)> {
    let price = resolve_analysis_price(req, ts_collection);

    if let Ok(cva) = pair_analysis_pure(req.pair_name.clone(), ts_collection, price, &req.config) {
        let cva_arc = Arc::new(cva);
        let profile = get_or_generate_profile(req, ts_collection, price);

        let ohlcv = find_matching_ohlcv(
            &ts_collection.series_data,
            &req.pair_name,
            req.config.interval_width_ms,
        )
        .ok()?;

        let mut model = TradingModel::from_cva(cva_arc, profile, ohlcv, &req.config);

        let opps = run_pathfinder_simulations(ohlcv, price, &req.config);
        model.opportunities = opps;

        let score: f64 = model
            .opportunities
            .iter()
            .map(|op| op.calculate_quality_score(&req.config.journey.profile))
            .sum();

        Some((score, model))
    } else {
        None
    }
}

fn resolve_analysis_price(req: &JobRequest, ts_collection: &TimeSeriesCollection) -> f64 {
    req.current_price.unwrap_or_else(|| {
        // FIX: Pass series_data, pair_name, interval_ms (i64)
        if let Ok(ts) = find_matching_ohlcv(
            &ts_collection.series_data,
            &req.pair_name,
            req.config.interval_width_ms,
        ) {
            ts.close_prices.last().copied().unwrap_or(0.0)
        } else {
            0.0
        }
    })
}

fn calculate_exact_candle_count(
    req: &JobRequest,
    ts_collection: &TimeSeriesCollection,
    price: f64,
) -> usize {
    let ohlcv_result = find_matching_ohlcv(
        &ts_collection.series_data,
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

fn get_or_generate_profile(
    req: &JobRequest,
    ts_collection: &TimeSeriesCollection,
    price: f64,
) -> HorizonProfile {
    // Check existing
    if let Some(existing) = &req.existing_profile {
        let price_match = (existing.base_price - price).abs() < f64::EPSILON;
        // Basic config check (reusing cached profile if valid)
        if price_match {
            return existing.clone();
        }
    }

    // Generate new
    horizon_profiler::generate_profile(
        &req.pair_name,
        ts_collection,
        price,
        &req.config.price_horizon,
    )
}

fn build_success_result(
    req: &JobRequest,
    ts_collection: &TimeSeriesCollection,
    cva: CVACore,
    profile: HorizonProfile,
    price: f64,
    count: usize,
    elapsed: u128,
) -> JobResult {
    let cva_arc = Arc::new(cva);

    let ohlcv = find_matching_ohlcv(
        &ts_collection.series_data,
        &req.pair_name,
        req.config.interval_width_ms,
    )
    .expect("OHLCV data missing despite CVA success");

    let mut model = TradingModel::from_cva(cva_arc.clone(), profile.clone(), ohlcv, &req.config);

    // Run Pathfinder
    let opps = run_pathfinder_simulations(ohlcv, price, &req.config);

    model.opportunities = opps.clone();

    JobResult {
        pair_name: req.pair_name.clone(),
        duration_ms: elapsed,
        result: Ok(Arc::new(model)),
        cva: Some(cva_arc),
        profile: Some(profile),
        candle_count: count,
    }
}
