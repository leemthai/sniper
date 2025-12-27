use crate::models::OhlcvTimeSeries;
use crate::analysis::market_state::MarketState;
use std::cmp::Ordering;
use rayon::prelude::*;

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
    TimedOut,         // Neither hit
}

#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub success_rate: f64,    // 0.0 to 1.0
    pub avg_duration: f64,    // Average candles to result
    pub risk_reward_ratio: f64, // Based on historical outcomes
    pub sample_size: usize,   // How many similar scenarios we found
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
    ) -> Vec<(usize, f64)> { // Returns (index, score)
        
        // 1. Calculate Current State
        let current_state = match MarketState::calculate(ts, current_idx, trend_lookback) {
            Some(s) => s,
            None => return vec![],
        };

        // 2. Define Scan Range (Don't scan the very end where we can't simulate forward)
        let end_scan = current_idx.saturating_sub(max_duration_candles);

        // 3. Parallel Scan (Rayon)
        let mut candidates: Vec<(usize, f64)> = (trend_lookback..end_scan)
            .into_par_iter()
            .filter_map(|i| {
                let hist_state = MarketState::calculate(ts, i, trend_lookback)?;
                let score = current_state.similarity_score(&hist_state);
                // Optimization: Filter out terrible matches early to save sort time
                if score < 100.0 {
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

        candidates
    }

    /// STEP 2: The Fast Replay.
    /// Runs the specific trade parameters on the pre-calculated matches.
    /// This runs MANY times (once per Zone).
    pub fn analyze_outcome(
        ts: &OhlcvTimeSeries,
        matches: &[(usize, f64)], // The candidates found in Step 1
        entry_price: f64,
        target_price: f64,
        stop_price: f64,
        max_duration: usize,
        direction: &str,
    ) -> Option<SimulationResult> {
        
        if matches.is_empty() {
            return None;
        }

        let mut wins = 0;
        let mut total_duration = 0.0;
        let mut valid_samples = 0;

        for &(start_idx, _score) in matches {
            let outcome = Self::replay_path(ts, start_idx, entry_price, target_price, stop_price, max_duration, direction);
            
            match outcome {
                Outcome::TargetHit(duration) => {
                    wins += 1;
                    total_duration += duration as f64;
                    valid_samples += 1;
                },
                Outcome::StopHit(_) => {
                    valid_samples += 1;
                },
                Outcome::TimedOut => {
                    // Treated as a loss or neutral? Currently counting as valid sample (Loss)
                    valid_samples += 1;
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

        Some(SimulationResult {
            success_rate,
            avg_duration,
            risk_reward_ratio: rr,
            sample_size: valid_samples,
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
        direction: &str
    ) -> Outcome {
        let start_candle = ts.get_candle(start_idx);
        let hist_entry = start_candle.close_price;
        
        // Calculate Target/Stop as % distance from entry
        let target_dist = (target - current_price_ref) / current_price_ref;
        let stop_dist = (stop - current_price_ref) / current_price_ref;

        // Iterate forward from the historical start point
        for i in 1..=duration {
            let idx = start_idx + i;
            if idx >= ts.klines() { return Outcome::TimedOut; }
            
            let c = ts.get_candle(idx);
            
            // Calculate % change from the historical entry price
            let low_change = (c.low_price - hist_entry) / hist_entry;
            let high_change = (c.high_price - hist_entry) / hist_entry;

            let hit_target;
            let hit_stop;

            if direction == "Long" {
                // Long: High >= Target, Low <= Stop
                hit_target = high_change >= target_dist;
                hit_stop = low_change <= stop_dist;
            } else {
                // Short: Low <= Target, High >= Stop
                hit_target = low_change <= target_dist;
                hit_stop = high_change >= stop_dist;
            }

            // Pessimistic: If both hit in same candle, assume Stop Hit first
            if hit_target && hit_stop { return Outcome::StopHit(i); } 
            if hit_stop { return Outcome::StopHit(i); }
            if hit_target { return Outcome::TargetHit(i); }
        }
        
        Outcome::TimedOut
    }
}