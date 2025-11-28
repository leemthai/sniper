//! Analysis and computation configuration

#[allow(unused_imports)]
use crate::utils::time_utils::MS_IN_4_H;
#[allow(unused_imports)]
use crate::utils::time_utils::MS_IN_15_MIN;
#[allow(unused_imports)]
use crate::utils::time_utils::MS_IN_30_MIN;
#[allow(unused_imports)]
use crate::utils::time_utils::MS_IN_H;

/// Time interval width to analyze (in milliseconds)
/// This defines the candle interval for all analysis (1h, 5m, 15m, etc.)
pub const INTERVAL_WIDTH_TO_ANALYSE_MS: i64 = MS_IN_30_MIN;

/// Default number of price zones for analysis
pub const DEFAULT_PRICE_ZONE_COUNT: usize = 100;

/// Time Horizon slider configuration (in days)
pub const TIME_HORIZON_MIN_DAYS: u64 = 1;
pub const TIME_HORIZON_MAX_DAYS: u64 = 100; // 1365; // 90;
pub const TIME_HORIZON_DEFAULT_DAYS: u64 = 7;

/// Price change threshold (percentage) to trigger journey recomputation
pub const JOURNEY_PRICE_UPDATE_THRESHOLD_PCT: f64 = 1.0;

/// Price change threshold (fractional) to trigger CVA recomputation
/// 0.01 corresponds to a 1% move from the anchor price
pub const CVA_PRICE_RECALC_THRESHOLD_PCT: f64 = 0.01;

/// Minimum debounce window between CVA recalculations (in seconds)
pub const CVA_MIN_SECONDS_BETWEEN_RECALCS: u64 = 60;

/// Tolerance when matching historical prices for journey analysis (percentage)
pub const JOURNEY_START_PRICE_TOLERANCE_PCT: f64 = 0.5;

/// Stop-loss threshold (percentage move against position) for journey failures
pub const JOURNEY_STOP_LOSS_PCT: f64 = 5.0;

/// Minimum number of candles required for valid CVA analysis
/// Below this threshold, the system lacks sufficient data for reliable zone detection
pub const MIN_CANDLES_FOR_ANALYSIS: usize = 100;
