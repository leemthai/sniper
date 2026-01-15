use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

use crate::analysis::market_state::MarketState;

use crate::config::DEBUG_FLAGS;
use crate::config::SimilaritySettings;

use crate::models::OhlcvTimeSeries;
use crate::models::trading_view::TradeDirection;

use crate::utils::time_utils::AppInstant;

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

    /// Padding ensures we don't segfault when loading chunks of 16 at the end
    pub fn pad_to_16(&mut self) {
        while self.vol.len() % 16 != 0 {
            self.vol.push(0.0);
            self.mom.push(0.0);
            self.rel_vol.push(0.0);
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

/// Generates Momentum using AVX-512 for (Close - OldClose) / OldClose
fn generate_momentum_optimized(
    ts: &OhlcvTimeSeries,
    start_idx: usize,
    end_idx: usize,
    lookback: usize,
) -> Vec<f32> {
    let len = end_idx.saturating_sub(start_idx);
    let mut results = vec![0.0f32; len];

    if start_idx < lookback {
        return results;
    }

    // AVX-512 Block (Processing 8 f64 -> 8 f32)
    #[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
    if is_x86_feature_detected!("avx512f") {
        unsafe {
            use std::arch::x86_64::*;
            let stride = 8;
            let loop_len = len - (len % stride);
            let close_ptr = ts.close_prices.as_ptr();

            for i in (0..loop_len).step_by(stride) {
                let curr_idx = start_idx + i;
                let prev_idx = curr_idx - lookback;

                let curr = _mm512_loadu_pd(close_ptr.add(curr_idx));
                let prev = _mm512_loadu_pd(close_ptr.add(prev_idx));

                let diff = _mm512_sub_pd(curr, prev);
                let mom_f64 = _mm512_div_pd(diff, prev);

                // Convert f64 -> f32 (256-bit result)
                let mom_f32 = _mm512_cvtpd_ps(mom_f64);

                _mm256_storeu_ps(results.as_mut_ptr().add(i), mom_f32);
            }
        }
    }

    // Scalar Fallback / Tail
    // FIX: Define 'processed' logic inside cfg blocks to avoid unused variables
    #[cfg(not(all(target_arch = "x86_64", target_feature = "avx512f")))]
    let processed = 0;

    #[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
    let processed = if is_x86_feature_detected!("avx512f") {
        len - (len % 8)
    } else {
        0
    };

    for i in processed..len {
        let curr_idx = start_idx + i;
        let prev_idx = curr_idx - lookback;
        let c = ts.close_prices[curr_idx];
        let p = ts.close_prices[prev_idx];
        if p > 0.0 {
            results[i] = ((c - p) / p) as f32;
        }
    }

    results
}

/// Generates Volatility using AVX-512 for Raw Vals + Rolling Sum
fn generate_volatility_optimized(
    ts: &OhlcvTimeSeries,
    start_idx: usize,
    end_idx: usize,
    lookback: usize,
) -> Vec<f32> {
    let len = end_idx.saturating_sub(start_idx);
    let mut results = vec![0.0f32; len];

    // 1. Generate Raw Volatility: (H - L) / C
    let raw_start = start_idx.saturating_sub(lookback);
    let raw_len = end_idx - raw_start;
    let mut raw_vols = vec![0.0f64; raw_len];

    #[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
    if is_x86_feature_detected!("avx512f") {
        unsafe {
            use std::arch::x86_64::*;
            let stride = 8;
            let loop_len = raw_len - (raw_len % stride);

            let h_ptr = ts.high_prices.as_ptr();
            let l_ptr = ts.low_prices.as_ptr();
            let c_ptr = ts.close_prices.as_ptr();

            for i in (0..loop_len).step_by(stride) {
                let idx = raw_start + i;
                let h = _mm512_loadu_pd(h_ptr.add(idx));
                let l = _mm512_loadu_pd(l_ptr.add(idx));
                let c = _mm512_loadu_pd(c_ptr.add(idx));

                let range = _mm512_sub_pd(h, l);
                let val = _mm512_div_pd(range, c);

                _mm512_storeu_pd(raw_vols.as_mut_ptr().add(i), val);
            }
        }
    }

    // Scalar Tail for Raw Vols
    // FIX: Define 'processed' logic inside cfg blocks
    #[cfg(not(all(target_arch = "x86_64", target_feature = "avx512f")))]
    let processed = 0;

    #[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
    let processed = if is_x86_feature_detected!("avx512f") {
        raw_len - (raw_len % 8)
    } else {
        0
    };

    for i in processed..raw_len {
        let idx = raw_start + i;
        let h = ts.high_prices[idx];
        let l = ts.low_prices[idx];
        let c = ts.close_prices[idx];
        if c > 0.0 {
            raw_vols[i] = (h - l) / c;
        }
    }

    // 2. Rolling Sum (SMA)
    let mut current_sum: f64 = raw_vols.iter().take(lookback).sum();
    let lookback_f = lookback as f64;

    if len > 0 {
        results[0] = (current_sum / lookback_f) as f32;
    }

    for i in 1..len {
        let leaving = raw_vols[i - 1];
        let entering = raw_vols[i + lookback - 1];
        current_sum = current_sum - leaving + entering;
        results[i] = (current_sum / lookback_f) as f32;
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
    pub avg_candle_count: f64,  // Average candles to result
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
        // --- PHASE 1: DATA PREPARATION (Optimized Generation) ---
        let t_prep_start = AppInstant::now();

        let start_idx = trend_lookback;
        let end_idx = end_scan;
        let count = end_idx.saturating_sub(start_idx);

        let mut simd_history = SimdHistory::new(count);

        if count > 0 {
            // A. Indices
            simd_history.indices = (start_idx..end_idx).collect();

            // B. Relative Volume (Copy & Cast)
            // Scalar copy is fast enough here
            if start_idx < ts.relative_volumes.len() {
                let safe_end = end_idx.min(ts.relative_volumes.len());
                simd_history.rel_vol = ts.relative_volumes[start_idx..safe_end]
                    .iter()
                    .map(|&v| v as f32)
                    .collect();
            }

            // C. Momentum (AVX Gen)
            simd_history.mom = generate_momentum_optimized(ts, start_idx, end_idx, trend_lookback);

            // D. Volatility (AVX Gen + Rolling Sum)
            simd_history.vol =
                generate_volatility_optimized(ts, start_idx, end_idx, trend_lookback);
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
        let mut candidates: Vec<(usize, f64)> = simd_history
            .indices
            .iter()
            .zip(raw_scores.iter())
            .map(|(&idx, &score)| (idx, score as f64))
            .collect();

        // Partial Sort (The "Quickselect" Optimization)
        if candidates.len() > sample_count {
            candidates.select_nth_unstable_by(sample_count, |a, b| {
                a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal)
            });
            candidates.truncate(sample_count);
        }

        candidates.sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        let t_sort = t_sort_start.elapsed();
        let t_total = t_start.elapsed();

        // Manual trace_time logic
        if DEBUG_FLAGS.enable_perf_logging {
            let t_threshold = 1000;
            if t_total.as_micros() > t_threshold {
                log::error!(
                    "TRACE: ScenarioSimulator [{}]: Total {:.2?} (Items: {} | Prep: {:.2?} | SIMD: {:.2?} | Sort: {:.2?}) (Threshold: {}ms)",
                    pair_name,
                    t_total,
                    simd_history.indices.len(),
                    t_prep,
                    t_simd,
                    t_sort,
                    t_threshold,
                );
            }
        }

        Some((candidates, current_market_state))
    }

    pub fn analyze_outcome(
        ts: &OhlcvTimeSeries,
        matches: &[(usize, f64)],
        current_market_state: MarketState,
        entry_price: f64,
        target_price: f64,
        stop_price: f64,
        max_duration_candles: usize, // Unit: Count
        direction: TradeDirection,
    ) -> Option<SimulationResult> {
        crate::trace_time!("Sim: Analyze Outcome (50 Matches)", 50, {
            if matches.is_empty() {
                return None;
            }

            let mut wins = 0;
            let mut accumulated_candle_count = 0.0; // Unit: Count (aggregated as float)
            let mut valid_samples = 0;
            let mut total_pnl_pct = 0.0; // Unit: Percentage

            // Pre-calculate theoretical max PnL for Hit/Stop
            let (win_pnl_pct, lose_pnl_pct) = match direction {
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
                    max_duration_candles,
                    direction,
                );

                match outcome {
                    Outcome::TargetHit(candles_taken) => {
                        wins += 1;
                        accumulated_candle_count += candles_taken as f64;
                        valid_samples += 1;
                        total_pnl_pct += win_pnl_pct;
                    }
                    Outcome::StopHit(candles_taken) => {
                        accumulated_candle_count += candles_taken as f64;
                        valid_samples += 1;
                        total_pnl_pct += lose_pnl_pct;
                    }
                    Outcome::TimedOut(final_drift_pct) => {
                        // Timeout means the full candle limit was exhausted
                        accumulated_candle_count += max_duration_candles as f64;
                        valid_samples += 1;
                        total_pnl_pct += final_drift_pct;
                    }
                }
            }

            if valid_samples == 0 {
                return None;
            }

            // Calculate Stats
            let success_rate = wins as f64 / valid_samples as f64;

            // Calculate Average Candle Count
            let avg_candle_count = accumulated_candle_count / valid_samples as f64;

            // R:R
            let risk = (entry_price - stop_price).abs();
            let reward = (target_price - entry_price).abs();
            let risk_reward_ratio = if risk > f64::EPSILON {
                reward / risk
            } else {
                0.0
            };

            // Real Average PnL (The "true" ROI of the sim)
            let avg_pnl_pct = total_pnl_pct / valid_samples as f64;

            Some(SimulationResult {
                success_rate,
                avg_candle_count,
                risk_reward_ratio,
                sample_size: valid_samples,
                avg_pnl_pct,
                market_state: current_market_state,
            })
        })
    }

    /// Wrapper: Dispatches to SIMD or Scalar and verifies consistency in Debug.
    fn replay_path(
        ts: &OhlcvTimeSeries,
        start_idx: usize,
        current_price_ref: f64,
        target: f64,
        stop: f64,
        duration: usize,
        direction: TradeDirection,
    ) -> Outcome {
        // 1. Run SIMD if available
        #[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
        let result = if is_x86_feature_detected!("avx512f") {
            unsafe {
                Self::replay_path_simd(ts, start_idx, current_price_ref, target, stop, duration, direction)
            }
        } else {
            Self::replay_path_scalar(ts, start_idx, current_price_ref, target, stop, duration, direction)
        };

        // Fallback for non-AVX builds
        #[cfg(not(all(target_arch = "x86_64", target_feature = "avx512f")))]
        let result = Self::replay_path_scalar(ts, start_idx, current_price_ref, target, stop, duration, direction);

        #[cfg(debug_assertions)]
        {
            // Always run scalar to compare
            let scalar_result = Self::replay_path_scalar(ts, start_idx, current_price_ref, target, stop, duration, direction);
            
            // Compare outcomes. 
            // FIX: Allow 1-candle slip for floating point rounding differences near the edge.
            let mismatch = match (&result, &scalar_result) {
                (Outcome::TargetHit(i1), Outcome::TargetHit(i2)) => i1.abs_diff(*i2) > 1,
                (Outcome::StopHit(i1), Outcome::StopHit(i2)) => i1.abs_diff(*i2) > 1,
                (Outcome::TimedOut(p1), Outcome::TimedOut(p2)) => (p1 - p2).abs() > 0.0000001,
                _ => true, // Different types (e.g. TargetHit vs StopHit) is always a failure
            };

            if mismatch {
                log::error!(
                    "SIMD REPLAY MISMATCH [Dir: {}]: SIMD {:?} vs SCALAR {:?}", 
                    direction, result, scalar_result
                );
                // We panic on significant mismatches to catch logic bugs
                panic!("CRITICAL: SIMD Simulation diverged significantly from Scalar Logic.");
            }
        }

        result
    }

    /// The Scalar Implementation (Your Authoritative Logic)
    fn replay_path_scalar(
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

            // Pessimistic assumption: If both hit, Stop hit first.
            if hit_stop {
                return Outcome::StopHit(i);
            }
            if hit_target {
                return Outcome::TargetHit(i);
            }
        }

        Outcome::TimedOut(final_pnl)
    }

/// The SIMD Implementation (AVX-512 Optimized)
    #[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
    unsafe fn replay_path_simd(
        ts: &OhlcvTimeSeries,
        start_idx: usize,
        current_price_ref: f64,
        target: f64,
        stop: f64,
        duration: usize,
        direction: TradeDirection,
    ) -> Outcome {
        use std::arch::x86_64::*;

        let len = ts.high_prices.len();
        if start_idx >= len {
            return Outcome::TimedOut(0.0);
        }

        let start_candle = ts.get_candle(start_idx);
        let hist_entry_price = start_candle.close_price;
        let scale = hist_entry_price / current_price_ref;
        
        let hist_target = target * scale;
        let hist_stop = stop * scale;

        let offset_start = start_idx + 1;
        let search_end = (start_idx + duration).min(len - 1) + 1; 
        let search_len = search_end.saturating_sub(offset_start);

        if search_len == 0 {
             return Outcome::TimedOut(0.0);
        }
        
        let stride = 8;
        let loop_len = search_len - (search_len % stride);
        let mut hit_idx_offset = None;

        let h_ptr = ts.high_prices.as_ptr();
        let l_ptr = ts.low_prices.as_ptr();

        // 1. AVX Scan Loop
        unsafe {
            let v_target = _mm512_set1_pd(hist_target);
            let v_stop = _mm512_set1_pd(hist_stop);

            for i in (0..loop_len).step_by(stride) {
                let curr = offset_start + i;
                
                let v_h = _mm512_loadu_pd(h_ptr.add(curr));
                let v_l = _mm512_loadu_pd(l_ptr.add(curr));

                let mask = match direction {
                    TradeDirection::Long => {
                        let m_win = _mm512_cmp_pd_mask(v_h, v_target, _CMP_GE_OQ);
                        let m_loss = _mm512_cmp_pd_mask(v_l, v_stop, _CMP_LE_OQ);
                        m_win | m_loss
                    },
                    TradeDirection::Short => {
                        let m_win = _mm512_cmp_pd_mask(v_l, v_target, _CMP_LE_OQ);
                        let m_loss = _mm512_cmp_pd_mask(v_h, v_stop, _CMP_GE_OQ);
                        m_win | m_loss
                    }
                };

                if mask != 0 {
                    hit_idx_offset = Some(i);
                    break; 
                }
            }
        }

        // 2. Scalar Processing (Hit Block or Tail)
        let scalar_start_offset = hit_idx_offset.unwrap_or(loop_len);
        
        // Explicit unsafe block for raw pointer dereferencing in the scalar tail
        unsafe {
            for i in scalar_start_offset..search_len {
                let idx = offset_start + i;
                let h = *h_ptr.add(idx);
                let l = *l_ptr.add(idx);
                let candle_count = i + 1; 

                match direction {
                    TradeDirection::Long => {
                        if l <= hist_stop { return Outcome::StopHit(candle_count); }
                        if h >= hist_target { return Outcome::TargetHit(candle_count); }
                    },
                    TradeDirection::Short => {
                        if h >= hist_stop { return Outcome::StopHit(candle_count); }
                        if l <= hist_target { return Outcome::TargetHit(candle_count); }
                    }
                }
            }
        }

        // 3. Time Out
        let final_idx = offset_start + search_len - 1;
        let final_close = ts.close_prices[final_idx];
        let close_change = (final_close - hist_entry_price) / hist_entry_price;
        
        let final_pnl = match direction {
            TradeDirection::Long => close_change,
            TradeDirection::Short => -close_change,
        };

        Outcome::TimedOut(final_pnl)
    }
}
