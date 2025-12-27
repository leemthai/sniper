use crate::utils::maths_utils::remap; // We will ensure remap is available

pub struct AdaptiveParameters;

impl AdaptiveParameters {
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