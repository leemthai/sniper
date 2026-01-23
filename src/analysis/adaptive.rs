use std::time::Duration;

use crate::utils::maths_utils::remap; // We will ensure remap is available
use crate::config::JourneySettings;

pub struct AdaptiveParameters;

impl AdaptiveParameters {

    /// Calculates Max Duration using Diffusive Market Physics (Random Walk).
    /// Formula: Candles = (Ratio + Bias)^2
    pub fn calculate_dynamic_journey_duration(ph_pct: f64, avg_volatility_pct: f64, interval_ms: i64, journey: &JourneySettings) -> Duration {
        // 1. Safety
        let vol = avg_volatility_pct.max(0.0001); 
        
        // 2. Ratio: How many "Volatility Units" is the target away?
        let ratio = ph_pct / vol;
        
        // 3. Diffusive Time with Bias
        // We add +3.0 to the ratio before squaring.
        // Effect:
        // - Scalp (Ratio 2): (2+3)^2 = 25 candles (vs 4 previously). Gives room to breathe.
        // - Swing (Ratio 100): (100+3)^2 = 10,609 candles (vs 10,000). Negligible change.
        let candles = (ratio + 3.0).powi(2);
        
        // 4. Convert to Time
        let total_ms = candles * interval_ms as f64;
        
        Duration::from_millis(total_ms as u64).clamp(
            journey.min_journey_duration, 
            journey.max_journey_time
        )
    }

    /// Maps Price Horizon % -> Time Decay Factor.
    /// Continuous curve, no steps.
    pub fn calculate_time_decay(ph_threshold: f64) -> f64 {
        if ph_threshold < 0.05 {
            // Sniper Zone (0% -> 5%): Decay 5.0 -> 2.0
            remap(ph_threshold, 0.0, 0.05, 5.0, 2.0)
        } else if ph_threshold < 0.15 {
            // Aggressive Zone (5% -> 15%): Decay 2.0 -> 1.5
            remap(ph_threshold, 0.05, 0.15, 2.0, 1.5)
        } else {
            // Macro Zone (15% -> 50%): Decay 1.5 -> 1.0
            remap(ph_threshold, 0.15, 0.50, 1.5, 1.0).max(1.0)
        }
    }

    /// Maps Price Horizon % -> Trend Lookback (Candles).
    pub fn calculate_trend_lookback_candles(ph_threshold: f64) -> usize {
        // 5m Candle Constants
        const DAY: f64 = 288.0;
        const WEEK: f64 = 2016.0;
        const MONTH: f64 = 8640.0; // 30 Days

        let result = if ph_threshold < 0.05 {
            // Scalp to Day Trade (2h -> 1 Day)
            remap(ph_threshold, 0.005, 0.05, 24.0, DAY)
        } else if ph_threshold < 0.15 {
            // Swing (1 Day -> 1 Week)
            remap(ph_threshold, 0.05, 0.15, DAY, WEEK)
        } else {
            // Macro (1 Week -> 1 Month at 50% PH, and beyond)
            // No cap. If user asks for 100% PH, they get ~2 Months lookback.
            remap(ph_threshold, 0.15, 0.50, WEEK, MONTH)
        };


        result.round() as usize
    }
}