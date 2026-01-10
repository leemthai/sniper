use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

// #[cfg(target_arch = "x86_64")]
// use std::arch::x86_64::*;
use crate::utils::time_utils::AppInstant;

use crate::analysis::market_state::MarketState;
use crate::config::SimilaritySettings;
use crate::models::OhlcvTimeSeries;
use crate::models::trading_view::TradeDirection;

/// Structure of Arrays (SoA) layout for AVX-512 processing.
/// Instead of [State, State, State], we have [All_Vols], [All_Moms], [All_Rels].
pub struct SimdHistory {
    pub indices: Vec<usize>, // Keep track of which candle index generated this data
    pub vol: Vec<f32>,
    pub mom: Vec<f32>,
    pub rel_vol: Vec<f32>,
}

impl SimdHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            indices: Vec::with_capacity(capacity),
            vol: Vec::with_capacity(capacity),
            mom: Vec::with_capacity(capacity),
            rel_vol: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, idx: usize, state: MarketState) {
        self.indices.push(idx);
        self.vol.push(state.volatility_pct as f32);
        self.mom.push(state.momentum_pct as f32);
        self.rel_vol.push(state.relative_volume as f32);
    }

    /// Padding ensures we don't segfault when loading chunks of 16 at the end
    pub fn pad_to_16(&mut self) {
        while self.vol.len() % 16 != 0 {
            self.vol.push(0.0);
            self.mom.push(0.0);
            self.rel_vol.push(0.0);
            // Index doesn't matter for padding, but keep lengths synced
            self.indices.push(0);
        }
    }
}

/// The Scalar Fallback (For non-AVX512 machines)
fn calculate_scores_scalar(
    history: &SimdHistory,
    current: &MarketState,
    weights: &SimilaritySettings,
) -> Vec<f32> {
    let mut results = Vec::with_capacity(history.vol.len());
    let c_vol = current.volatility_pct as f32;
    let c_mom = current.momentum_pct as f32;
    let c_rel = current.relative_volume as f32;

    let w_vol = weights.weight_volatility as f32;
    let w_mom = weights.weight_momentum as f32;
    let w_rel = weights.weight_volume as f32;

    for i in 0..history.vol.len() {
        let d_vol = (history.vol[i] - c_vol).abs();
        let d_mom = (history.mom[i] - c_mom).abs();
        let d_rel = (history.rel_vol[i] - c_rel).abs();

        results.push(d_vol * w_vol + d_mom * w_mom + d_rel * w_rel);
    }
    results
}

/// The AVX-512 Kernel (The Race Car)
#[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
unsafe fn calculate_scores_avx512(
    history: &SimdHistory,
    current: &MarketState,
    weights: &SimilaritySettings,
) -> Vec<f32> {
    use std::arch::x86_64::*;

    let len = history.vol.len();
    let mut results = vec![0.0f32; len];

    // Explicit unsafe block required for intrinsics and pointer arithmetic
    unsafe {
        let cur_vol = _mm512_set1_ps(current.volatility_pct as f32);
        let cur_mom = _mm512_set1_ps(current.momentum_pct as f32);
        let cur_rel = _mm512_set1_ps(current.relative_volume as f32);

        let w_vol = _mm512_set1_ps(weights.weight_volatility as f32);
        let w_mom = _mm512_set1_ps(weights.weight_momentum as f32);
        let w_rel = _mm512_set1_ps(weights.weight_volume as f32);

        for i in (0..len).step_by(16) {
            let h_vol = _mm512_loadu_ps(history.vol.as_ptr().add(i));
            let h_mom = _mm512_loadu_ps(history.mom.as_ptr().add(i));
            let h_rel = _mm512_loadu_ps(history.rel_vol.as_ptr().add(i));

            let d_vol = _mm512_abs_ps(_mm512_sub_ps(h_vol, cur_vol));
            let d_mom = _mm512_abs_ps(_mm512_sub_ps(h_mom, cur_mom));
            let d_rel = _mm512_abs_ps(_mm512_sub_ps(h_rel, cur_rel));

            let term1 = _mm512_mul_ps(d_vol, w_vol);
            let term2 = _mm512_fmadd_ps(d_mom, w_mom, term1);
            let total = _mm512_fmadd_ps(d_rel, w_rel, term2);

            _mm512_storeu_ps(results.as_mut_ptr().add(i), total);
        }
    }

    results
}

#[derive(Debug, Clone)]
pub struct ScenarioConfig {
    pub target_price: f64,
    pub stop_loss_price: f64,
    pub max_duration_candles: usize, // e.g. 7 days converted to candles
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    TargetHit(usize), // Succeeded in N candles
    StopHit(usize),   // Failed in N candles
    TimedOut(f64),    // Neither hit nor failed. Stores % change at timeout
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub success_rate: f64,      // 0.0 to 1.0
    pub avg_duration: f64,      // Average candles to result
    pub risk_reward_ratio: f64, // Based on historical outcomes
    pub sample_size: usize,     // How many similar scenarios we found
    pub avg_pnl_pct: f64,       // The True Expected Retun (Continuous)
    pub market_state: MarketState,
}

pub struct ScenarioSimulator;

impl ScenarioSimulator {
    /// STEP 1: The Heavy Lift.
    /// Scans history to find the Top N moments that look like "Now".
    /// This runs ONCE per job.
    ///
    /// /// Scans history to find the Top N moments that look like "Now".
    pub fn find_historical_matches(
        pair_name: &str,
        ts: &OhlcvTimeSeries,
        current_idx: usize,
        sim_config: &SimilaritySettings,
        sample_count: usize,
        trend_lookback: usize,
        max_duration_candles: usize,
    ) -> Option<(Vec<(usize, f64)>, MarketState)> {
        let t_start = AppInstant::now();

        // 1. Calculate Current Market State
        let current_market_state = MarketState::calculate(ts, current_idx, trend_lookback)?;

        // 2. Define Scan Range (Matches original logic exactly)
        let end_scan = current_idx.saturating_sub(max_duration_candles);

        // --- PHASE 1: DATA PREPARATION (SoA Construction) ---
        let t_prep_start = AppInstant::now();

        // Use parallel iterator to generate states
        let history_states: Vec<(usize, MarketState)> = (trend_lookback..end_scan)
            .into_par_iter()
            .filter_map(|i| MarketState::calculate(ts, i, trend_lookback).map(|ms| (i, ms)))
            .collect();

        // Flatten into SoA
        let mut simd_history = SimdHistory::new(history_states.len());
        for (i, state) in history_states {
            simd_history.push(i, state);
        }

        // Critical for SIMD safety: Pad float vectors only
        simd_history.pad_to_16();

        let t_prep = t_prep_start.elapsed();

        // --- PHASE 2: SCORING (The SIMD Kernel) ---
        let t_simd_start = AppInstant::now();

        #[allow(unused_assignments)]
        let mut raw_scores = Vec::new();

        #[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
        if is_x86_feature_detected!("avx512f") {
            // SAFETY: We verified the feature is detected and arrays are padded
            raw_scores = unsafe {
                calculate_scores_avx512(&simd_history, &current_market_state, sim_config)
            };
        } else {
            raw_scores = calculate_scores_scalar(&simd_history, &current_market_state, sim_config);
        }

        #[cfg(not(all(target_arch = "x86_64", target_feature = "avx512f")))]
        {
            raw_scores = calculate_scores_scalar(&simd_history, &current_market_state, sim_config);
        }

        let t_simd = t_simd_start.elapsed();

        // --- PHASE 3: SORT & SELECT ---
        let t_sort_start = AppInstant::now();

        // Zip indices back with scores
        // We iterate indices (which are NOT padded) so we ignore the float padding naturally
        let mut candidates: Vec<(usize, f64)> = simd_history
            .indices
            .iter()
            .zip(raw_scores.iter())
            .map(|(&idx, &score)| (idx, score as f64))
            .collect();

        // Partial Sort (The "Quickselect" Optimization)
        // If we have more than N items, we only care about the top N.
        if candidates.len() > sample_count {
            // This partitions the array: Smallest N items move to the front.
            // The rest are moved to the back. Order within the groups is undefined.
            // This is O(N) instead of O(N log N).
            candidates.select_nth_unstable_by(sample_count, |a, b| {
                a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal)
            });

            // Discard the trash immediately
            candidates.truncate(sample_count);
        }

        candidates.sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        let t_sort = t_sort_start.elapsed();
        let t_total = t_start.elapsed();

        // --- LOGGING ---
        log::error!(
            "PATHFINDER SIMD [{}]: Scanned {} items in {:.2?} (Prep: {:.2?} | SIMD: {:.2?} | Sort: {:.2?})",
            pair_name,
            simd_history.indices.len(),
            t_total,
            t_prep,
            t_simd,
            t_sort
        );

        Some((candidates, current_market_state))
    }

    /// STEP 2: The Fast Replay.
    /// Runs the specific trade parameters on the pre-calculated matches.
    /// This runs MANY times (once per Zone).
    pub fn analyze_outcome(
        ts: &OhlcvTimeSeries,
        matches: &[(usize, f64)], // The candidates found in Step 1
        current_market_state: MarketState,
        entry_price: f64,
        target_price: f64,
        stop_price: f64,
        max_duration: usize,
        direction: TradeDirection,
    ) -> Option<SimulationResult> {
        if matches.is_empty() {
            return None;
        }

        let mut wins = 0;
        let mut total_duration = 0.0;
        let mut valid_samples = 0;
        let mut total_pnl_pct = 0.0;

        // Pre-calculate theoretical max PnL for Hit/Stop
        // Long: Target > Entry (Positive), Stop < Entry (Negative)
        // Short: Target < Entry (Positive), Stop > Entry (Negative)
        let (win_pnl, lose_pnl) = match direction {
            TradeDirection::Long => (
                (target_price - entry_price) / entry_price,
                (stop_price - entry_price) / entry_price,
            ),
            TradeDirection::Short => (
                (entry_price - target_price) / entry_price,
                (entry_price - stop_price) / entry_price,
            ),
        };

        for &(start_idx, _score) in matches {
            let outcome = Self::replay_path(
                ts,
                start_idx,
                entry_price,
                target_price,
                stop_price,
                max_duration,
                direction,
            );

            match outcome {
                Outcome::TargetHit(duration) => {
                    wins += 1;
                    total_duration += duration as f64;
                    valid_samples += 1;
                    total_pnl_pct += win_pnl;
                }
                Outcome::StopHit(_) => {
                    valid_samples += 1;
                    total_pnl_pct += lose_pnl;
                }
                Outcome::TimedOut(final_pct) => {
                    valid_samples += 1;
                    total_pnl_pct += final_pct; // Add the actual drift
                }
            }
        }

        if valid_samples == 0 {
            return None;
        }

        // Calculate Stats
        let success_rate = wins as f64 / valid_samples as f64;

        let avg_duration = if wins > 0 {
            total_duration / wins as f64
        } else {
            0.0
        };

        // R:R
        let risk = (entry_price - stop_price).abs();
        let reward = (target_price - entry_price).abs();
        let rr = if risk > f64::EPSILON {
            reward / risk
        } else {
            0.0
        };

        // real Average pnl (The "true" ROI of the sim)
        let avg_pnl_pct = total_pnl_pct / valid_samples as f64;

        Some(SimulationResult {
            success_rate,
            avg_duration,
            risk_reward_ratio: rr,
            sample_size: valid_samples,
            avg_pnl_pct,
            market_state: current_market_state,
        })
    }

    /// Helper: Replays a single path using % moves to normalize price differences
    fn replay_path(
        ts: &OhlcvTimeSeries,
        start_idx: usize,
        current_price_ref: f64,
        target: f64,
        stop: f64,
        duration: usize,
        direction: TradeDirection,
    ) -> Outcome {
        let start_candle = ts.get_candle(start_idx);
        let hist_entry = start_candle.close_price;

        // Calculate Target/Stop as % distance from entry
        let target_dist = (target - current_price_ref) / current_price_ref;
        let stop_dist = (stop - current_price_ref) / current_price_ref;
        let mut final_pnl = 0.0;

        // Iterate forward from the historical start point
        for i in 1..=duration {
            let idx = start_idx + i;
            if idx >= ts.klines() {
                break;
            }

            let c = ts.get_candle(idx);

            let low_change = (c.low_price - hist_entry) / hist_entry;
            let high_change = (c.high_price - hist_entry) / hist_entry;
            let close_change = (c.close_price - hist_entry) / hist_entry;

            let (hit_target, hit_stop) = match direction {
                TradeDirection::Long => {
                    final_pnl = close_change;
                    (high_change >= target_dist, low_change <= stop_dist)
                }
                TradeDirection::Short => {
                    final_pnl = -close_change; // Invert PnL for shorts
                    (low_change <= target_dist, high_change >= stop_dist)
                }
            };

            if hit_target && hit_stop {
                return Outcome::StopHit(i);
            }
            if hit_stop {
                return Outcome::StopHit(i);
            }
            if hit_target {
                return Outcome::TargetHit(i);
            }
        }

        Outcome::TimedOut(final_pnl)
    }
}
