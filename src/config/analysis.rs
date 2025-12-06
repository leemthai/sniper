//! Analysis and computation configuration

use crate::utils::TimeUtils;

/// Configuration for the Time Horizon UI Slider
pub struct TimeHorizonConfig {
    // Time Horizon slider configuration
    pub min_days: u64,
    pub max_days: u64,
    pub default_days: u64,
}

/// Settings specific to Journey Analysis
pub struct JourneySettings {
    // Tolerance when matching historical prices for journey analysis (percentage)
    pub start_price_tolerance_pct: f64,
    pub stop_loss_pct: f64,
}

/// Settings for CVA (Cumulative Volume Analysis)
pub struct CvaSettings {
    // Price change threshold (fractional) to trigger CVA recomputation
    // 0.01 corresponds to a 1% move from the anchor price
    pub price_recalc_threshold_pct: f64,
    // Minimum debounce window between CVA recalculations (in seconds)
    pub min_seconds_between_recalcs: u64,
    // Minimum number of candles required for valid CVA analysis
    // Below this threshold, the system lacks sufficient data for reliable zone detection
    pub min_candles_for_analysis: usize,
}

/// The Master Analysis Configuration
pub struct AnalysisConfig {
    // This defines the candle interval for all analysis (1h, 5m, 15m, etc.)
    pub interval_width_ms: i64,
    // Number of price zones for analysis (actually constant rn, never updated)
    pub default_zone_count: usize,

    // Sub-groups
    pub time_horizon: TimeHorizonConfig,
    pub journey: JourneySettings,
    pub cva: CvaSettings,
}

pub const ANALYSIS: AnalysisConfig = AnalysisConfig {
    interval_width_ms: TimeUtils::MS_IN_30_MIN,
    default_zone_count: 200, // Goldilocks number (see private project-3eed40f.md for explanation)

    time_horizon: TimeHorizonConfig {
        min_days: 1,
        max_days: 100,
        default_days: 7,
    },

    journey: JourneySettings {
        start_price_tolerance_pct: 0.5,
        // Stop-loss threshold (percentage move against position) for journey failures
        stop_loss_pct: 5.0,
    },

    cva: CvaSettings {
        price_recalc_threshold_pct: 0.01,
        min_seconds_between_recalcs: 60,
        min_candles_for_analysis: 100,
    },
};
