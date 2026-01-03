use std::cmp::Ordering;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};


use crate::analysis::market_state::MarketState;
use crate::config::SimilaritySettings;
use crate::models::OhlcvTimeSeries;
use crate::models::trading_view::TradeDirection;


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
    pub success_rate: f64,    // 0.0 to 1.0
    pub avg_duration: f64,    // Average candles to result
    pub risk_reward_ratio: f64, // Based on historical outcomes
    pub sample_size: usize,   // How many similar scenarios we found
    pub avg_pnl_pct: f64, // The True Expected Retun (Continuous)
    pub market_state: MarketState,
}

pub struct ScenarioSimulator;

impl ScenarioSimulator {
    /// STEP 1: The Heavy Lift.
    /// Scans history to find the Top N moments that look like "Now".
    /// This runs ONCE per job.
    pub fn find_historical_matches(
        ts: &OhlcvTimeSeries,
        current_idx: usize,
        trend_lookback: usize,
        max_duration_candles: usize,
        sample_count: usize,
        sim_config: &SimilaritySettings,
    ) -> Option<(Vec<(usize, f64)>, MarketState)> { // Returns (index, score)
        
        // 1. Calculate Current Market State
        let current_market_state = MarketState::calculate(ts, current_idx, trend_lookback)?;

        // 2. Define Scan Range (Don't scan the very end where we can't simulate forward)
        let end_scan = current_idx.saturating_sub(max_duration_candles);

        // 3. Parallel Scan (Rayon)
        let mut candidates: Vec<(usize, f64)> = (trend_lookback..end_scan)
            .into_par_iter()
            .filter_map(|i| {
                let hist_state = MarketState::calculate(ts, i, trend_lookback)?;
                
                // USE CONFIG
                let score = current_market_state.similarity_score(&hist_state, sim_config);
                
                if score < sim_config.cutoff_score { 
                    Some((i, score)) 
                } else { 
                    None 
                }
            })
            .collect();

        // 4. Sort by similarity (Lowest score is best match)
        candidates.sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        // 5. Keep Top N
        if candidates.len() > sample_count {
            candidates.truncate(sample_count);
        }

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
                (stop_price - entry_price) / entry_price
            ),
            TradeDirection::Short => (
                (entry_price - target_price) / entry_price,
                (entry_price - stop_price) / entry_price
            ),
        };


        for &(start_idx, _score) in matches {
            let outcome = Self::replay_path(ts, start_idx, entry_price, target_price, stop_price, max_duration, direction);
            
            match outcome {
                Outcome::TargetHit(duration) => {
                    wins += 1;
                    total_duration += duration as f64;
                    valid_samples += 1;
                    total_pnl_pct += win_pnl;
                },
                Outcome::StopHit(_) => {
                    valid_samples += 1;
                    total_pnl_pct += lose_pnl;
                },
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
        let rr = if risk > f64::EPSILON { reward / risk } else { 0.0 };

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
            if idx >= ts.klines() { break; }
            
            let c = ts.get_candle(idx);
            
            let low_change = (c.low_price - hist_entry) / hist_entry;
            let high_change = (c.high_price - hist_entry) / hist_entry;
            let close_change = (c.close_price - hist_entry) / hist_entry;

            let (hit_target, hit_stop) = match direction {
                TradeDirection::Long => {
                    final_pnl = close_change;
                    (high_change >= target_dist, low_change <= stop_dist)
                },
                TradeDirection::Short => {
                    final_pnl = -close_change; // Invert PnL for shorts
                    (low_change <= target_dist, high_change >= stop_dist)
                }
            };

            if hit_target && hit_stop { return Outcome::StopHit(i); }
            if hit_stop { return Outcome::StopHit(i); }
            if hit_target { return Outcome::TargetHit(i); }
        }
        
        Outcome::TimedOut(final_pnl)
    }
}