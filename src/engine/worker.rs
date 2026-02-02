use rayon::prelude::*;
use std::cmp::Ordering;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

// Only import thread crate on non-WASM target
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

use uuid::Uuid;

use super::messages::{JobMode, JobRequest, JobResult};

use crate::analysis::adaptive::AdaptiveParameters;
use crate::analysis::market_state::MarketState;
use crate::analysis::pair_analysis::pair_analysis_pure;
use crate::analysis::scenario_simulator::{ScenarioSimulator, SimulationResult};

use crate::config::{DF, DurationMs, OptimizationStrategy, PhPct, Price, StationId, StopPrice, TargetPrice, TradeProfile, TunerStation, constants, LowPrice, HighPrice, PriceLike};

use crate::data::timeseries::TimeSeriesCollection;

use crate::domain::price_horizon;

use crate::models::OhlcvTimeSeries;
use crate::models::cva::CVACore;
use crate::models::timeseries::find_matching_ohlcv;
use crate::models::trading_view::{TradeDirection, TradeOpportunity, TradeVariant, VisualFluff};

use crate::TradingModel;

use crate::utils::maths_utils::duration_to_candles;
use crate::utils::time_utils::{AppInstant, TimeUtils};

#[cfg(debug_assertions)]
use {crate::ui::ui_text::UI_TEXT};

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

/// Runs the "Scan & Fit" algorithm to find the optimal Price Horizon
/// that produces trades within the Station's target time range.
pub fn tune_to_station(
    ohlcv: &OhlcvTimeSeries,
    current_price: Price,
    station: &TunerStation,
    strategy: OptimizationStrategy,
) -> Option<PhPct> {

    let _t_start = AppInstant::now();

    #[cfg(debug_assertions)]
    {
        if DF.log_tuner {
            log::info!(
                "📻 TUNER START [{}]: Station '{}' (Target: {:.1}-{:.1}h) | Scan Range: {:.1}%-{:.1}% | Strategy: {}",
                ohlcv.pair_interval.name(),
                station.name,
                station.target_min_hours,
                station.target_max_hours,
                *station.scan_ph_min * 100.0,
                *station.scan_ph_max * 100.0,
                strategy
            );
        }
    }

    // 1. Generate Scan Points (Linear Interpolation)
    let steps = constants::TUNER_SCAN_STEPS;
    let mut scan_points = Vec::with_capacity(steps);
    if steps > 1 {
        let step_size = (*station.scan_ph_max - *station.scan_ph_min) / (steps - 1) as f64;
        for i in 0..steps {
            scan_points.push(*station.scan_ph_min + (i as f64 * step_size));
        }
    } else {
        scan_points.push(*station.scan_ph_min); // Fallback
    }

    // 2. Run Simulations
    // We store: (PH, Score, Duration_Hours, Candidate_Count)
    let mut results: Vec<(f64, f64, f64, usize)> = Vec::new();

    for &ph in &scan_points {
        // Run the Optimized Pathfinder
        let result =
            run_pathfinder_simulations(ohlcv, current_price, PhPct::new(ph), strategy, station.id, None);

        let count = result.opportunities.len();
        if count > 0 {
            // Calculate Average Duration of top results (in Hours)
            let duration_hours = result
                .opportunities
                .iter()
                .map(|o| *o.avg_duration_ms)
                .sum::<i64>() as f64
                / count as f64 / 3_600_000.0;

            // Calculate Representative Score (Top Score)
            let top_score = result.opportunities[0].calculate_quality_score();

            results.push((ph, top_score, duration_hours, count));

            #[cfg(debug_assertions)]
            if DF.log_tuner {
                log::info!(
                    "   📡 TUNER PROBE {:.2}%: Found {} ops | Top Score {:.2} | Avg Dur {:.1}h",
                    ph * 100.0,
                    count,
                    top_score,
                    duration_hours
                );
            }
        } else {
            #[cfg(debug_assertions)]
            if DF.log_tuner {
                log::info!(
                    "   📡 TUNER PROBE {:.2}%: No signals found (0 candidates).",
                    ph * 100.0
                );
            }
        }
    }

    // 3. The "Fit" Logic (Selection)
    if results.is_empty() {
        #[cfg(debug_assertions)]
        log::warn!("⚠️ TUNER FAILED: No candidates found across entire range.");
        return None;
    }

    // A. Filter: Must be within Target Duration Window
    let valid_fits: Vec<&(f64, f64, f64, usize)> = results
        .iter()
        .filter(|(_, _, dur, _)| {
            *dur >= station.target_min_hours && *dur <= station.target_max_hours
        })
        .collect();

    let best_match = if !valid_fits.is_empty() {
        // B. Selector: Best Score among valid fits
        valid_fits
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
            .unwrap()
    } else {
        // Fallback: Nothing fit the time window perfectly.
        // Pick the result closest to the target duration range (Center point).
        #[cfg(debug_assertions)]
        if DF.log_tuner {
            log::warn!("   ⚠️ No perfect time fit. Falling back to closest duration.");
        }
        let target_center = (station.target_min_hours + station.target_max_hours) / 2.0;

        results
            .iter()
            .min_by(|a, b| {
                let dist_a = (a.2 - target_center).abs();
                let dist_b = (b.2 - target_center).abs();
                dist_a.partial_cmp(&dist_b).unwrap_or(Ordering::Equal)
            })
            .unwrap()
    };

    #[cfg(debug_assertions)]
    {
        let elapsed = _t_start.elapsed();
        if DF.log_tuner {
            log::info!(
                "✅ TUNER LOCKED: {:.2}% (Score {:.2}, Duration {:.1}h) | Took {:?}",
                best_match.0 * 100.0,
                best_match.1,
                best_match.2,
                elapsed
            );
        }
    }

    Some(PhPct::new(best_match.0))
}

/// Helper: Runs the simulation tournament for a specific target price.
fn simulate_target(
    ctx: &PathfinderContext,
    target_price: TargetPrice,
    source_id_suffix: &str,
    risk_tests: &[f64],
    limit_samples: usize,
) -> Option<CandidateResult> {
    crate::trace_time!("Worker: Simulate Target", 500, {
        // let profile = &ctx.config.journey.profile;

        let direction = if target_price.value() > ctx.current_price.value() {
            TradeDirection::Long
        } else {
            TradeDirection::Short
        };

        let best_sl_opt = run_stop_loss_tournament(
            ctx.ohlcv,
            &ctx.matches,
            ctx.current_state,
            ctx.current_price,
            target_price,
            direction,
            ctx.duration_candles,
            risk_tests,
            &constants::journey::DEFAULT.profile,
            ctx.strategy,
            constants::BASE_INTERVAL.as_millis() as i64,
            limit_samples,
            0,
        );

        if let Some((result, stop_price, variants)) = best_sl_opt {
            let avg_dur_ms = (result.avg_candle_count * constants::BASE_INTERVAL.as_millis() as i64 as f64) as i64;
            let score = ctx.strategy.calculate_score(
                result.avg_pnl_pct,
                DurationMs::new(avg_dur_ms),
            );

            let unique_string = format!("{}_{}_{}", ctx.pair_name, source_id_suffix, direction);
            let uuid = Uuid::new_v5(&Uuid::NAMESPACE_OID, unique_string.as_bytes()).to_string();

            // SAFE EXTRACTION:
            // If we have CVA data, clone the profile. If not, visuals are None.
            let visuals = ctx.cva.map(|core| VisualFluff {
                volume_profile: core.candle_bodies_vw.clone(),
            });

            let opp = TradeOpportunity {
                id: uuid,
                created_at: TimeUtils::now_utc(),
                source_ph_pct: ctx.ph_pct,
                pair_name: ctx.pair_name.to_string(),
                direction,
                start_price: ctx.current_price,
                target_price,
                stop_price,
                max_duration_ms: ctx.duration_ms,
                avg_duration_ms: DurationMs::new(avg_dur_ms),
                strategy: ctx.strategy,
                station_id: ctx.station_id,
                market_state: ctx.current_state,
                visuals,
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
    })
}

/// Phase C: Filters opportunities using the "Regional Championship" strategy.
/// 1. Divides the price range into N regions.
/// 2. Finds the best trade in each region (Local Winner).
/// 3. Filters Local Winners against the Global Best Score (Qualifying Round).
fn apply_diversity_filter(
    candidates: Vec<CandidateResult>,
    _pair_name: &str,
    range_min: f64,
    range_max: f64,
    _strategy: OptimizationStrategy,
) -> Vec<TradeOpportunity> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let regions = constants::journey::optimization::DIVERSITY_REGIONS;
    let cutoff_ratio = constants::journey::optimization::DIVERSITY_CUT_OFF;
    let max_results = constants::journey::optimization::MAX_RESULTS;

    // --- INSERT START: DEBUG SCOREBOARD ---
    // This creates a temporary view to log the top candidates without consuming the vector
    #[cfg(debug_assertions)]
    if DF.log_pathfinder {
        if _strategy == OptimizationStrategy::Balanced {
            // Create vector of references so we can sort them for display only
            let mut debug_view: Vec<&CandidateResult> = candidates.iter().collect();
            debug_view.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            log::info!("⚖️ BALANCED SCOREBOARD [{}] (Top 5 Inputs):", _pair_name);
            for (i, c) in debug_view.iter().take(5).enumerate() {
                let roi = c.opportunity.expected_roi();
                let dur_ms = c.opportunity.avg_duration_ms;
                let dur_str = TimeUtils::format_duration(*dur_ms);
                let aroi = TradeProfile::calculate_annualized_roi(roi, dur_ms);

                log::info!(
                    "   #{}: Score {:.1} | ROI {} | AROI {} | Time {}",
                    i + 1,
                    c.score,
                    roi,
                    aroi,
                    dur_str
                );
            }
        }
    }

    // Identify Global Max Score (The Gold Standard). We need this to determine the qualifying time.
    let global_best_score = candidates
        .iter()
        .map(|c| c.score)
        .fold(f64::NEG_INFINITY, f64::max);

    let qualifying_score = global_best_score * *cutoff_ratio;

    #[cfg(debug_assertions)]
    if DF.log_pathfinder {
        log::info!(
            "🏆 REGIONAL CHAMPIONSHIP [{}]: {} Candidates. Global Best: {:.2} | Qualifying: {:.2} ({})",
            _pair_name,
            candidates.len(),
            global_best_score,
            qualifying_score,
            cutoff_ratio
        );
    }

    // 2. Hold Local Tournaments
    // Vector of Options to hold the winner of each region
    let mut regional_winners: Vec<Option<CandidateResult>> = (0..regions).map(|_| None).collect();
    let total_range = range_max - range_min;
    let bucket_size = total_range / regions as f64;

    for cand in candidates {
        // Determine Region Index
        let offset = cand.opportunity.target_price.value() - range_min;
        // Clamp index to 0..regions-1 (handle edge cases where price == max)
        let idx = ((offset / bucket_size).floor() as usize).min(regions - 1);

        // Battle for the Region
        match &mut regional_winners[idx] {
            None => {
                // Uncontested (so far)
                regional_winners[idx] = Some(cand);
            }
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
                if DF.log_pathfinder {
                    #[cfg(debug_assertions)]
                    log::info!(
                        "   ✅ Region #{} Winner [{}]: Score {:.2} | Target ${:.2} (Qualified)",
                        _i,
                        winner.source_desc,
                        winner.score,
                        winner.opportunity.target_price
                    );
                }
                final_results.push(winner.opportunity);
            } else {
                #[cfg(debug_assertions)]
                if DF.log_pathfinder {
                    log::info!(
                        "   ❌ Region #{} Winner [{}]: Score {:.2} (Failed Qualifier < {:.2})",
                        _i,
                        winner.source_desc,
                        winner.score,
                        qualifying_score
                    );
                }
            }
        }
    }

    // 4. Final Sort (Best of the Best first)
    final_results.sort_by(|a, b| {
        // Recalculate score for sorting or store it?
        // TradeOpportunity doesn't store the raw score, but we can re-calc cheaply or trust the input order.
        // Let's re-calc to be safe and strict sort.
        let score_a = a.calculate_quality_score();
        let score_b = b.calculate_quality_score();
        score_b.partial_cmp(&score_a).unwrap_or(Ordering::Equal)
    });

    // Hard Limit (if configured lower than region count)
    if final_results.len() > max_results {
        final_results.truncate(max_results);
    }

    final_results
}

struct PathfinderContext<'a> {
    pair_name: &'a str,
    ohlcv: &'a OhlcvTimeSeries,
    cva: Option<&'a CVACore>, // Optional because it is used only to produce VisualFluff
    matches: Vec<(usize, f64)>,
    current_state: MarketState,
    current_price: Price,
    strategy: OptimizationStrategy,
    station_id: StationId,
    duration_candles: usize,
    duration_ms: DurationMs,
    ph_pct: PhPct,
    price_min: LowPrice,
    price_max: HighPrice,
}

fn run_scout_phase(ctx: &PathfinderContext) -> Vec<CandidateResult> {
    
    let price_buffer_pct = constants::journey::optimization::PRICE_BUFFER_PCT;
    let steps = constants::journey::optimization::SCOUT_STEPS;
    let scout_risks = [2.5]; // Optimization: 1 variant

    // --- OPTIMIZATION #2: DIRECTIONAL BIAS (Pruning) ---
    // Analyze the historical outcomes of our matches to detect strong trends.
    let mut bias_long = true;
    let mut bias_short = true;

    if !ctx.matches.is_empty() {
        let mut up_votes = 0;
        let mut down_votes = 0;

        // Check the outcome of every historical match
        for (start_idx, _) in &ctx.matches {
            // Safety: Ensure we don't read past end of data
            let end_idx = (start_idx + ctx.duration_candles).min(ctx.ohlcv.close_prices.len() - 1);

            let start_price = ctx.ohlcv.close_prices[*start_idx];
            let end_price = ctx.ohlcv.close_prices[end_idx];

            if end_price.value() > start_price.value() {
                up_votes += 1;
            } else if end_price.value() < start_price.value() {
                down_votes += 1;
            }
        }

        // --- TEST CHEAT CODE (Uncomment to force a direction for debugging) ---
        // up_votes = 50; down_votes = 0; // Force BULLISH (Skip Shorts)
        // up_votes = 0; down_votes = 50; // Force BEARISH (Skip Longs)
        // ---------------------------------------------------------------------

        let total_votes = up_votes + down_votes;
        if total_votes > 0 {
            let up_ratio = up_votes as f64 / total_votes as f64;
            let down_ratio = down_votes as f64 / total_votes as f64;
            let threshold = 0.80; // 80% Consensus required to prune

            if up_ratio >= threshold {
                bias_short = false; // Market is STRONGLY BULLISH -> Skip Shorts
                #[cfg(debug_assertions)]
                if DF.log_pathfinder {
                    log::info!(
                        "🌊 DIRECTIONAL BIAS [{}]: Bullish Consensus ({:.0}%). Pruning SHORT Scouts.",
                        ctx.pair_name,
                        up_ratio * 100.0
                    );
                }
            } else if down_ratio >= threshold {
                bias_long = false; // Market is STRONGLY BEARISH -> Skip Longs
                #[cfg(debug_assertions)]
                if DF.log_pathfinder {
                    log::info!(
                        "🌊 DIRECTIONAL BIAS [{}]: Bearish Consensus ({:.0}%). Pruning LONG Scouts.",
                        ctx.pair_name,
                        down_ratio * 100.0
                    );
                }
            }
        }
    }

    // Pre-calculate ranges and booleans to avoid repeated math in the loop
    let long_start = ctx.current_price.value() * (1.0 + *price_buffer_pct);
    let short_end = ctx.current_price.value() * (1.0 - *price_buffer_pct);

    // Combine PH constraints with Bias constraints
    let long_active = (ctx.price_max.value() > long_start) && bias_long;
    let short_active = (ctx.price_min.value() < short_end) && bias_short;
    // log::error!("{}: long active is {} short active is {}", ctx.pair_name, long_active, short_active);

    // Calculate step sizes
    let long_step_size = if long_active {
        (ctx.price_max.value() - long_start) / steps as f64
    } else {
        0.0
    };

    let short_step_size = if short_active {
        (short_end - ctx.price_min.value()) / steps as f64
    } else {
        0.0
    };

    crate::trace_time!("Pathfinder: Phase A (Scouts)", 1000, {
        // Use Rayon to process Longs and Shorts in parallel (Single Batch)
        let results: Vec<CandidateResult> = (0..=steps)
            .into_par_iter()
            .flat_map(|i| {
                let mut local_results = Vec::with_capacity(2);

                // 1. Long Scout Logic
                if long_active {
                    let target = long_start + (i as f64 * long_step_size);
                    if let Some(res) = simulate_target(
                        ctx,
                        TargetPrice::new(target),
                        &format!("scout_long_{}", i),
                        &scout_risks,
                        20, // JIT Sample Count
                    ) {
                        local_results.push(res);
                    }
                }

                // 2. Short Scout Logic
                if short_active {
                    let target = ctx.price_min.value() + (i as f64 * short_step_size);
                    if let Some(res) = simulate_target(
                        ctx,
                        TargetPrice::new(target),
                        &format!("scout_short_{}", i),
                        &scout_risks,
                        20, // JIT Sample Count
                    ) {
                        local_results.push(res);
                    }
                }

                local_results
            })
            .collect();

        #[cfg(debug_assertions)]
        if DF.log_pathfinder {
            log::info!(
                "🔍 SCOUT PHASE [{}]: Found {} viable candidates (Parallel).",
                ctx.pair_name,
                results.len()
            );
        }

        results
    })
}

fn run_drill_phase(
    ctx: &PathfinderContext,
    mut candidates: Vec<CandidateResult>,
) -> Vec<CandidateResult> {
    if candidates.is_empty() {
        return candidates;
    }

    // let opt_config = &constants::journey::OPTIMIZATION;
    let steps = constants::journey::optimization::SCOUT_STEPS;
    let drill_offset_factor = constants::journey::optimization::DRILL_OFFSET_FACTOR;
    let drill_cutoff_pct = constants::journey::optimization::DRILL_CUTOFF_PCT;
    let drill_top_n = constants::journey::optimization::DRILL_TOP_N;

    // Sort Best First
    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));

    #[cfg(debug_assertions)]
    {
        if DF.log_pathfinder {
            log::info!(
                "🔍 SCOUT PHASE COMPLETE [{}]. Top 5 Candidates:",
                ctx.pair_name
            );
            for (i, c) in candidates.iter().take(5).enumerate() {
                log::info!(
                    "   #{}: [{}] Target ${:.4} | Score {:.2}",
                    i + 1,
                    c.source_desc,
                    c.opportunity.target_price,
                    c.score
                );
            }
        }
    }

    crate::trace_time!("Pathfinder: Phase B (Drill)", 2000, {
        let mut drill_targets = Vec::new();

        let grid_step_pct = (ctx.price_max.value() - ctx.price_min.value()) / ctx.current_price.value() / steps as f64;
        let drill_offset_pct = grid_step_pct * drill_offset_factor;
        let dedup_radius = grid_step_pct * 100.0;

        // NEW: Adaptive Cutoff Score
        let best_score = candidates[0].score;
        let score_threshold = best_score * *drill_cutoff_pct;

        #[cfg(debug_assertions)]
        if DF.log_pathfinder {
            log::info!(
                "🔍 PHASE B: Drill Selection (Radius: {:.3}%, Cutoff Score: {:.2})",
                dedup_radius,
                score_threshold
            );
        }

        // 1. Smart Selection (Dedup + Cutoff)
        for (idx, candidate) in candidates.iter().enumerate() {
            // Optimization #4: Adaptive Cutoff
            if candidate.score < score_threshold {
                #[cfg(debug_assertions)]
                if DF.log_pathfinder {
                    log::info!(
                        "   🛑 Cutting off Scout [{}]: Score {:.2} < Threshold",
                        candidate.source_desc,
                        candidate.score
                    );
                }
                break;
            }

            let mut is_distinct = true;
            for &picked_idx in &drill_targets {
                let picked: &CandidateResult = &candidates[picked_idx];
                let pct_diff = candidate.opportunity.target_price.percent_diff(&picked.opportunity.target_price);

                if candidate.opportunity.direction == picked.opportunity.direction
                    && pct_diff < dedup_radius
                {
                    is_distinct = false;
                    break;
                }
            }

            if is_distinct {
                drill_targets.push(idx);
            }

            if drill_targets.len() >= drill_top_n {
                break;
            }
        }

        #[cfg(debug_assertions)]
        if DF.log_pathfinder {
            log::info!(
                "⛏️ DRILL PHASE [{}] [Strategy: {}]: Drilling {} distinct scouts",
                ctx.pair_name,
                ctx.strategy,
                drill_targets.len()
            );
        }

        // 2. Drill Loop (Parallelized)
        let full_risks = constants::journey::RISK_REWARD_TESTS;
        let full_samples = constants::journey::SAMPLE_COUNT;

        // Use Rayon to calculate results in parallel
        let drill_results: Vec<CandidateResult> = drill_targets
            .par_iter()
            .flat_map(|&scout_idx| {
                let scout = &candidates[scout_idx];
                let base_target = scout.opportunity.target_price.value();
                let mut local_batch = Vec::with_capacity(3);

                // A. Promote Scout
                if let Some(res) = simulate_target(
                    ctx,
                    TargetPrice::new(base_target),
                    &scout.source_desc,
                    full_risks,
                    full_samples,
                ) {
                    if res.score > scout.score {
                        local_batch.push(res);
                    }
                }
                // B. Drill Up
                if let Some(res) = simulate_target(
                    ctx,
                    TargetPrice::new(base_target * (1.0 + drill_offset_pct)),
                    &format!("drill_{}_up", scout_idx),
                    full_risks,
                    full_samples,
                ) {
                    local_batch.push(res);
                }
                // C. Drill Down
                if let Some(res) = simulate_target(
                    ctx,
                    TargetPrice::new(base_target * (1.0 - drill_offset_pct)),
                    &format!("drill_{}_down", scout_idx),
                    full_risks,
                    full_samples,
                ) {
                    local_batch.push(res);
                }
                local_batch
            })
            .collect();

        if !drill_results.is_empty() {
            if DF.log_pathfinder {
                #[cfg(debug_assertions)]
                log::info!(
                    "   -> [{}] Drill generated {} refined candidates.",
                    ctx.pair_name,
                    drill_results.len()
                );
            }
            candidates.extend(drill_results);
        }
    });

    candidates
}

// Public so Audit can see it)
pub struct PathfinderResult {
    pub opportunities: Vec<TradeOpportunity>,
    pub trend_lookback: usize, // Trend_K
    pub sim_duration: usize,   // Sim_K
}

pub fn run_pathfinder_simulations(
    ohlcv: &OhlcvTimeSeries,
    current_price: Price,
    ph_pct: PhPct,
    strategy: OptimizationStrategy,
    station_id: StationId,
    cva_opt: Option<&CVACore>,
) -> PathfinderResult {

    if !current_price.is_positive() {
        return PathfinderResult {
            opportunities: Vec::new(),
            trend_lookback: 0,
            sim_duration: 0,
        };
    }

    // Volatility
    let max_idx = ohlcv.klines().saturating_sub(1);
    let vol_lookback = constants::journey::optimization::VOLATILITY_LOOKBACK.min(max_idx);
    let start_vol = ohlcv.klines().saturating_sub(vol_lookback);
    let avg_volatility = ohlcv.calculate_volatility_in_range(start_vol, ohlcv.klines());

    // Matches
    let trend_lookback = AdaptiveParameters::calculate_trend_lookback_candles(ph_pct);
    let duration = AdaptiveParameters::calculate_dynamic_journey_duration(
        ph_pct,
        avg_volatility,
        DurationMs::new(constants::BASE_INTERVAL.as_millis() as i64),
        &constants::journey::DEFAULT,
    );
    let duration_candles = duration_to_candles(duration, constants::BASE_INTERVAL.as_millis() as i64);

    let matches_opt = ScenarioSimulator::find_historical_matches(
        ohlcv.pair_interval.name(),
        ohlcv,
        max_idx,
        &constants::similarity::DEFAULT,
        constants::journey::SAMPLE_COUNT,
        trend_lookback,
        duration_candles,
    );

    let (matches, _current_state) = match matches_opt {
        Some(tuple) => (tuple.0, tuple.1),
        None => {
            return PathfinderResult {
                opportunities: Vec::new(),
                trend_lookback,
                sim_duration: duration_candles,
            };
        }
    };

    let (price_min, price_max) = price_horizon::calculate_price_range(current_price, ph_pct);

    // Build Context Object
    let ctx = PathfinderContext {
        pair_name: ohlcv.pair_interval.name(),
        ohlcv,
        cva: cva_opt,
        matches,
        current_state: _current_state,
        current_price,
        strategy,
        station_id,
        duration_candles,
        duration_ms: DurationMs::new(duration.as_millis() as i64),
        ph_pct,
        price_min,
        price_max,
    };

    #[cfg(debug_assertions)]
    if DF.log_pathfinder {
        log::info!(
            "🎯 PATHFINDER START [{}] Price: {} | PH Range: {} - {} ({}) | Vol: {}",
            ctx.pair_name,
            ctx.current_price,
            ctx.price_min,
            ctx.price_max,
            ctx.ph_pct,
            avg_volatility
        );
    }

    // 2. Run Pipeline
    let scouts = run_scout_phase(&ctx);
    let drill_results = run_drill_phase(&ctx, scouts);

    // 3. Final Filter
    let final_opps: Vec<TradeOpportunity> = apply_diversity_filter(
        drill_results,
        ctx.pair_name,
        ctx.price_min.value(),
        ctx.price_max.value(),
        strategy,
    );
    // Return everything
    PathfinderResult {
        opportunities: final_opps,
        trend_lookback,
        sim_duration: duration_candles,
    }
}

/// Helper function to find the optimal Stop Loss for a given target
fn run_stop_loss_tournament(
    ohlcv: &OhlcvTimeSeries,
    historical_matches: &[(usize, f64)],
    current_state: MarketState,
    current_price: Price,
    target_price: TargetPrice,
    direction: TradeDirection,
    duration_candles: usize,
    risk_tests: &[f64],
    profile: &TradeProfile,
    strategy: OptimizationStrategy,
    interval_ms: i64,
    limit_samples: usize,
    _zone_idx: usize,
) -> Option<(SimulationResult, StopPrice, Vec<TradeVariant>)> {
    
    crate::trace_time!("Worker: SL Tournament", 1500, {
        let mut best_score = f64::NEG_INFINITY; // Track Score, not just ROI
        let mut best_result: Option<(SimulationResult, StopPrice, f64)> = None; // (Result, Stop, Ratio)
        let mut valid_variants = Vec::new();

        let target_dist_abs = (target_price.value() - current_price.value()).abs();

        // 1. Safety Check: Volatility Floor.
        let vol_floor_pct = *current_state.volatility_pct * 2.0;
        let min_stop_dist = current_price.value() * vol_floor_pct;

        // Logging setup
        #[cfg(debug_assertions)]
        if DF.log_pathfinder {
            log::info!(
                "🔍 Analyzing Zone {} ({}): Testing {} SL candidates. Volatility Floor: {:.2}% (${:.4})",
                _zone_idx,
                direction,
                risk_tests.len(),
                vol_floor_pct * 100.0,
                min_stop_dist
            );
        }

        // OPTIMIZATION: Slice the matches based on the phase (Scout vs Drill)
        let effective_matches = if limit_samples < historical_matches.len() {
            &historical_matches[0..limit_samples]
        } else {
            historical_matches
        };

        for &ratio in risk_tests {
            // 2. Calculate Candidate Stop
            let stop_dist = target_dist_abs / ratio;

            if stop_dist < min_stop_dist {
                #[cfg(debug_assertions)]
                if DF.log_pathfinder {
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
                TradeDirection::Long => StopPrice::new(current_price.value() - stop_dist),
                TradeDirection::Short => StopPrice::new(current_price.value() + stop_dist),
            };

            // 3. Simulation
            if let Some(result) = ScenarioSimulator::analyze_outcome(
                ohlcv,
                effective_matches,
                current_state,
                current_price,
                target_price,
                candidate_stop,
                duration_candles,
                direction,
            ) {
                // Metrics
                let roi_pct = result.avg_pnl_pct;

                let duration_real_ms = ((result.avg_candle_count * interval_ms as f64) as f64) as i64;

                // Calculate AROI for the Gatekeeper & Judge
                let aroi_pct = TradeProfile::calculate_annualized_roi(roi_pct, DurationMs::new(duration_real_ms));

                // GATEKEEPER: Check both ROI and AROI against profile
                let is_worthwhile = profile.is_worthwhile(roi_pct, aroi_pct);

                if is_worthwhile {
                    // Store this variant
                    valid_variants.push(TradeVariant {
                        ratio,
                        stop_price: candidate_stop,
                        roi_pct,
                        simulation: result.clone(),
                    });

                    // JUDGE: Calculate Score based on Strategy (using CORRECT Time units)
                    let score = strategy.calculate_score(roi_pct, DurationMs::new(duration_real_ms));

                    // Track Best
                    if score > best_score {
                        best_score = score;
                        best_result = Some((result.clone(), candidate_stop, ratio));
                    }
                }

                #[cfg(debug_assertions)]
                if DF.log_pathfinder {
                    let risk_pct = candidate_stop.percent_diff(&current_price);
                    let status_icon = if is_worthwhile { "✅" } else { "🔻" };
                    log::info!(
                        "   [R:R {:.1}] {} Stop: {} | {}: {} | ROI: {} | AROI: {} | Risk: {:.2}%",
                        ratio,
                        status_icon,
                        candidate_stop,
                        UI_TEXT.label_success_rate,
                        result.success_rate,
                        roi_pct,
                        aroi_pct,
                        risk_pct,
                    );
                }
            }
        }

        // Return Winner + Count
        if let Some((res, stop, _ratio)) = best_result {
            #[cfg(debug_assertions)]
            if DF.log_pathfinder {
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
    })
}

pub fn process_request_sync(req: JobRequest, tx: Sender<JobResult>) {
    // 1. ACQUIRE DATA (Clone & Release)
    let ts_local = match fetch_local_timeseries(&req) {
        Ok(ts) => ts,
        Err(e) => {
            let _ = tx.send(JobResult {
                pair_name: req.pair_name.clone(),
                duration_ms: 0,
                result: Err(e),
                cva: None,
                candle_count: 0,
            });
            return;
        }
    };
    perform_standard_analysis(&req, &ts_local, tx);
}

fn fetch_local_timeseries(req: &JobRequest) -> Result<TimeSeriesCollection, String> {
    let ts_guard = req
        .timeseries
        .read()
        .map_err(|_| "Failed to acquire RwLock".to_string())?;

    let target_pair = &req.pair_name;
    let interval = constants::BASE_INTERVAL.as_millis() as i64;

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

fn perform_standard_analysis(
    req: &JobRequest,
    ts_collection: &TimeSeriesCollection,
    tx: Sender<JobResult>,
) {
    let ph_pct = req.ph_pct;
    let base_label = format!("{} @ {:.2}%", req.pair_name, *ph_pct * 100.0);

    crate::trace_time!(&format!("Total JOB [{}]", base_label), 10_000, {
        let start = AppInstant::now();

        // 1. Price
        let price = match resolve_analysis_price(req, ts_collection) {
            Ok(p) => p,
            Err(e) => {
                let _ = tx.send(build_error_result(req, start.elapsed().as_millis(), e, 0));
                return;
            }
        };

        // 2. Count
        let count = crate::trace_time!(&format!("1. Exact Count [{}]", base_label), 4_000, {
            calculate_exact_candle_count(req, ts_collection, price)
        });

        let full_label = format!("{} ({} candles)", base_label, count);

        // 3. CVA
        let result_cva = crate::trace_time!(&format!("2. CVA Calc [{}]", full_label), 10_000, {
            pair_analysis_pure(req.pair_name.clone(), ts_collection, price, ph_pct)
        });

        let elapsed = start.elapsed().as_millis();

        // 5. Result Construction
        let response = match result_cva {
            Ok(cva) => {
                // BRANCH: Check Mode
                if req.mode == JobMode::ContextOnly {
                    // Fast Return: No Simulations
                    JobResult {
                        pair_name: req.pair_name.clone(),
                        duration_ms: elapsed,
                        // Return the Model with CVA, but EMPTY opportunities
                        result: Ok(Arc::new(TradingModel::from_cva(
                            Arc::new(cva),
                            find_matching_ohlcv(
                                &ts_collection.series_data,
                                &req.pair_name,
                                constants::BASE_INTERVAL.as_millis() as i64,
                            )
                            .unwrap(),
                            // &req.config
                        ))),
                        cva: None, // Legacy field (optional)
                        candle_count: count,
                    }
                } else {
                    // Full Analysis: Pass CVA to Pathfinder
                    build_success_result(req, ts_collection, cva, price, count, elapsed)
                }
            }
            Err(e) => build_error_result(req, elapsed, e.to_string(), count),
        };

        let _ = tx.send(response);
    });
}

fn build_error_result(
    req: &JobRequest,
    duration_ms: u128,
    error_msg: String,
    candle_count: usize,
) -> JobResult {
    JobResult {
        pair_name: req.pair_name.clone(),
        duration_ms,
        result: Err(error_msg),
        cva: None,
        candle_count,
    }
}

fn resolve_analysis_price(
    req: &JobRequest,
    ts_collection: &TimeSeriesCollection,
) -> Result<Price, String> {
    if let Some(p) = req.current_price {
        return Ok(p);
    }

    if let Ok(ts) = find_matching_ohlcv(
        &ts_collection.series_data,
        &req.pair_name,
        constants::BASE_INTERVAL.as_millis() as i64,
    ) {
        ts.close_prices
            .last()
            .copied()
            .map(|p| Price::new(p.value()))
            .ok_or_else(|| "Worker: Data exists but prices are empty".to_string())
    } else {
        Err(format!("Worker: No data found for {}", req.pair_name))
    }
}

fn calculate_exact_candle_count(
    req: &JobRequest,
    ts_collection: &TimeSeriesCollection,
    price: Price,
) -> usize {
    let ohlcv_result = find_matching_ohlcv(
        &ts_collection.series_data,
        &req.pair_name,
        constants::BASE_INTERVAL.as_millis() as i64,
    );

    if let Ok(ohlcv) = ohlcv_result {
        let (ranges, _) = price_horizon::auto_select_ranges(ohlcv, price, req.ph_pct);
        ranges.iter().map(|(s, e)| e - s).sum()
    } else {
        0
    }
}

fn build_success_result(
    req: &JobRequest,
    ts_collection: &TimeSeriesCollection,
    cva: CVACore,
    price: Price,
    count: usize,
    elapsed: u128,
) -> JobResult {
    let cva_arc = Arc::new(cva);

    let ohlcv = find_matching_ohlcv(
        &ts_collection.series_data,
        &req.pair_name,
        constants::BASE_INTERVAL.as_millis() as i64,
    )
    .expect("OHLCV data missing despite CVA success");

    let mut model = TradingModel::from_cva(cva_arc.clone(), ohlcv);

    // Run Pathfinder
    let pf_result = run_pathfinder_simulations(
        ohlcv,
        price,
        req.ph_pct,
        req.strategy,
        req.station_id,
        Some(&cva_arc),
    );

    model.opportunities = pf_result.opportunities;

    JobResult {
        pair_name: req.pair_name.clone(),
        duration_ms: elapsed,
        result: Ok(Arc::new(model)),
        cva: Some(cva_arc),
        candle_count: count,
    }
}
