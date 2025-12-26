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
    /// The Core "Ghost Runner" Logic.
    /// 1. Fingerprint the current moment.
    /// 2. Scan history for matches.
    /// 3. Replay history to see if it hit Target or Stop.
    pub fn simulate(
        timeseries: &OhlcvTimeSeries,
        current_idx: usize,
        current_state: &MarketState,
        config: &ScenarioConfig,
        trend_lookback: usize,
    ) -> Option<SimulationResult> {

        // 2. Scan History
        let end_scan = current_idx.saturating_sub(config.max_duration_candles);
        let start_scan = trend_lookback + 1;
        let similarity_threshold = 2.0; 
        
        // OPTIMIZATION: Use Rayon to scan history in parallel.
        // This splits the 500k candle check across all cores.
        let mut candidates: Vec<(usize, f64)> = (start_scan..end_scan)
            .into_par_iter() // <--- PARALLEL ITERATOR
            .filter_map(|i| {
                // Determine if this historical moment matches "Now"
                if let Some(hist_state) = MarketState::calculate(timeseries, i, trend_lookback) {
                    let score = current_state.similarity_score(&hist_state);
                    if score < similarity_threshold {
                        return Some((i, score));
                    }
                }
                None
            })
            .collect();
        
        // Sort by similarity (Lowest score is best match) and take Top 50
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        if candidates.len() > 50 {
            candidates.truncate(50);
        }
        
        if candidates.is_empty() {
            return None;
        }

        // 3. Run the Ghost Runners
        let mut wins = 0;
        let mut losses = 0;
        let mut timeouts = 0;
        let mut total_duration = 0;

        // Current Price Context (to normalize % moves)
        let current_price = timeseries.close_prices[current_idx];
        let target_dist_pct = (config.target_price - current_price) / current_price;
        let stop_dist_pct = (config.stop_loss_price - current_price) / current_price;

        for (start_idx, _) in &candidates {
            let result = Self::run_single_path(
                timeseries, 
                *start_idx, 
                config.max_duration_candles, 
                target_dist_pct, 
                stop_dist_pct
            );

            match result {
                Outcome::TargetHit(d) => { wins += 1; total_duration += d; },
                Outcome::StopHit(d) => { losses += 1; total_duration += d; },
                Outcome::TimedOut => { timeouts += 1; },
            }
        }

        // 4. Compile Results
        let total_runs = wins + losses + timeouts;
        let success_rate = wins as f64 / total_runs as f64;
        let valid_runs = wins + losses;
        let avg_dur = if valid_runs > 0 { total_duration as f64 / valid_runs as f64 } else { 0.0 };

        Some(SimulationResult {
            success_rate,
            avg_duration: avg_dur,
            risk_reward_ratio: if losses > 0 { wins as f64 / losses as f64 } else { wins as f64 },
            sample_size: total_runs,
        })
    }

    /// Replays a single historical path.
    /// Uses % distance to apply the current trade logic to the historical price.
    fn run_single_path(
        ts: &OhlcvTimeSeries,
        start_idx: usize,
        max_steps: usize,
        target_pct: f64,
        stop_pct: f64,
    ) -> Outcome {
        let entry_price = ts.close_prices[start_idx];
        let target_price = entry_price * (1.0 + target_pct);
        let stop_price = entry_price * (1.0 + stop_pct);

        let is_long = target_pct > 0.0;
        
        // Scan forward
        for i in 1..=max_steps {
            let idx = start_idx + i;
            if idx >= ts.klines() { return Outcome::TimedOut; } // End of data

            let high = ts.high_prices[idx];
            let low = ts.low_prices[idx];

            if is_long {
                // Check Low first (conservative assumption: stopped out before hitting target in same candle?)
                // Or check High? Usually depends on strategy. Let's check both range intersection.
                let hit_stop = low <= stop_price;
                let hit_target = high >= target_price;

                if hit_stop && hit_target {
                    // Ambiguous candle (hit both). Pessimistic: Stop Hit.
                    return Outcome::StopHit(i);
                } else if hit_stop {
                    return Outcome::StopHit(i);
                } else if hit_target {
                    return Outcome::TargetHit(i);
                }
            } else {
                // Short
                let hit_stop = high >= stop_price;
                let hit_target = low <= target_price;

                if hit_stop && hit_target {
                    return Outcome::StopHit(i);
                } else if hit_stop {
                    return Outcome::StopHit(i);
                } else if hit_target {
                    return Outcome::TargetHit(i);
                }
            }
        }

        Outcome::TimedOut
    }
}