//! Automatic duration selection based on price relevancy
//!
//! This module implements intelligent duration selection by analyzing which candles
//! have price action within a specified percentage range of the current price.
//! Unlike simple time-based lookback, this finds ALL candles within the price range,
//! potentially creating discontinuous slices that skip periods of high volatility.

use crate::models::OhlcvTimeSeries;
use serde::{Deserialize, Serialize};

/// Default relevancy threshold: data within ±15% of current price is considered relevant
pub const DEFAULT_RELEVANCY_THRESHOLD: f64 = 0.15;

/// Minimum lookback period in days (1 week) - ensures we include recent data even in low volatility
pub const MIN_LOOKBACK_DAYS: usize = 7;

/// Configuration for auto-duration selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoDurationConfig {
    /// Percentage threshold for price relevancy (e.g., 0.15 = ±15%)
    pub relevancy_threshold: f64,
    /// Minimum lookback period in days
    pub min_lookback_days: usize,
}

impl Default for AutoDurationConfig {
    fn default() -> Self {
        Self {
            relevancy_threshold: DEFAULT_RELEVANCY_THRESHOLD,
            min_lookback_days: MIN_LOOKBACK_DAYS,
        }
    }
}

/// Calculates the price range considered "relevant" to the current price
///
/// The relevancy window expands/contracts around the live price whenever
/// `current_price` changes. This means slice selection can drift slightly on
/// each poll even though the historical klines are immutable.
fn calculate_price_range(current_price: f64, threshold: f64) -> (f64, f64) {
    let range_multiplier = 1.0 + threshold;
    let price_min = current_price / range_multiplier;
    let price_max = current_price * range_multiplier;
    (price_min, price_max)
}

/// Find all discontinuous ranges of candles where price is within the relevancy range
///
/// Scans through the time series to find ALL candles that have price action within
/// the specified range of the current price. Returns a vector of (start, end) ranges
/// where each range is a continuous sequence of relevant candles.
///
/// Returns a vector of ranges [(start_idx, end_idx), ...] where end_idx is exclusive.
fn find_relevant_ranges(
    timeseries: &OhlcvTimeSeries,
    current_price: f64,
    config: &AutoDurationConfig,
) -> Vec<(usize, usize)> {
    let (price_min, price_max) = calculate_price_range(current_price, config.relevancy_threshold);

    let total_candles = timeseries.open_prices.len();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut range_start: Option<usize> = None;

    for i in 0..total_candles {
        let candle = timeseries.get_candle(i);

        // Check if candle overlaps with relevant price range
        let is_relevant = candle.low_price <= price_max && candle.high_price >= price_min;

        if is_relevant {
            // Start a new range if we're not in one
            if range_start.is_none() {
                range_start = Some(i);
            }
        } else {
            // End the current range if we were in one
            if let Some(start) = range_start {
                ranges.push((start, i)); // i is exclusive end
                range_start = None;
            }
        }
    }

    // Close any open range at the end
    if let Some(start) = range_start {
        ranges.push((start, total_candles));
    }

    ranges
}

/// Apply minimum lookback constraint to ranges
///
/// Ensures we have at least the minimum number of candles by potentially
/// extending backward in time even if those candles are outside the price range.
fn apply_min_lookback_constraint(
    ranges: Vec<(usize, usize)>,
    timeseries: &OhlcvTimeSeries,
    config: &AutoDurationConfig,
) -> Vec<(usize, usize)> {
    if ranges.is_empty() {
        return ranges;
    }

    // Calculate how many candles we currently have
    let total_relevant_candles: usize = ranges.iter().map(|(start, end)| end - start).sum();

    // Calculate minimum candles needed based on time
    let interval_minutes = timeseries.pair_interval.interval_ms / (1000 * 60);
    let min_lookback_minutes = config.min_lookback_days * 24 * 60;
    let min_lookback_candles = if interval_minutes > 0 {
        (min_lookback_minutes as i64 / interval_minutes) as usize
    } else {
        0
    };

    // If we already have enough, return as-is
    if total_relevant_candles >= min_lookback_candles {
        return ranges;
    }

    // Otherwise, extend the earliest range backward to meet minimum
    let deficit = min_lookback_candles - total_relevant_candles;
    let mut extended_ranges = ranges.clone();

    if let Some(first_range) = extended_ranges.first_mut() {
        let new_start = first_range.0.saturating_sub(deficit);
        first_range.0 = new_start;
    }

    extended_ranges
}

/// Automatically select discontinuous slice ranges based on price relevancy
///
/// This finds ALL candles where the price action falls within the relevancy threshold
/// of the current price, returning a vector of ranges. This means candles during periods
/// of high volatility (outside the price range) are excluded from analysis.
///
/// # Arguments
/// * `timeseries` - Historical OHLCV data
/// * `current_price` - Current market price (typically from WebSocket stream)
/// * `config` - Configuration for relevancy threshold and minimum lookback
///
/// # Returns
/// `Vec<(start_idx, end_idx)>` - Vector of slice ranges where end_idx is exclusive
///
/// # Example
/// ```ignore
/// let ranges = auto_select_ranges(&timeseries, 99500.0, &Default::default());
/// // Might return [(100, 200), (350, 450), (500, 600)]
/// // This means analyze candles 100-199, 350-449, and 500-599
/// // (skipping 200-349 and 450-499 because prices were outside range)
/// ```
pub fn auto_select_ranges(
    timeseries: &OhlcvTimeSeries,
    current_price: f64,
    config: &AutoDurationConfig,
) -> (Vec<(usize, usize)>, (f64, f64)) {
    // Edge case: no data
    if timeseries.open_prices.is_empty() {
        #[cfg(debug_assertions)]
        log::error!("Auto-slice: No data available, returning empty ranges");
        return (Vec::new(), (0.0, 0.0));
    }

    // Calculate the user-defined price range
    let (price_min, price_max) = calculate_price_range(current_price, config.relevancy_threshold);

    // Find all ranges where price is relevant
    let mut ranges = find_relevant_ranges(timeseries, current_price, config);

    // Apply minimum lookback constraint
    ranges = apply_min_lookback_constraint(ranges, timeseries, config);

    (ranges, (price_min, price_max))
}

/// Calculate the earliest timestamp (in ms since epoch) where relevant data begins
///
/// This is useful for debugging or displaying to the user when the "relevant window" starts.
#[allow(dead_code)]
pub fn calculate_relevant_start_timestamp(
    timeseries: &OhlcvTimeSeries,
    current_price: f64,
    config: &AutoDurationConfig,
) -> i64 {
    let (ranges, _) = auto_select_ranges(timeseries, current_price, config);

    if let Some((start_idx, _)) = ranges.first() {
        timeseries.first_kline_timestamp_ms
            + (*start_idx as i64 * timeseries.pair_interval.interval_ms)
    } else {
        timeseries.first_kline_timestamp_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pair_interval::PairInterval;

    #[test]
    fn test_price_range_calculation() {
        let (min, max) = calculate_price_range(100.0, 0.15);
        assert!((min - 86.96).abs() < 0.01); // 100 / 1.15
        assert!((max - 115.0).abs() < 0.01); // 100 * 1.15
    }

    #[test]
    fn test_discontinuous_ranges() {
        // Create a mock timeseries with candles at different price levels
        // Current price: 100.0, threshold: 0.15 means range is ~87-115
        let timeseries = OhlcvTimeSeries {
            pair_interval: PairInterval {
                name: "BTCUSDT".to_string(),
                interval_ms: 3600000, // 1 hour
            },
            first_kline_timestamp_ms: 0,
            open_prices: vec![95.0, 96.0, 120.0, 125.0, 98.0, 99.0, 100.0],
            high_prices: vec![96.0, 97.0, 125.0, 130.0, 99.0, 100.0, 101.0],
            low_prices: vec![94.0, 95.0, 119.0, 124.0, 97.0, 98.0, 99.0],
            close_prices: vec![96.0, 95.0, 124.0, 126.0, 98.0, 100.0, 101.0],
            base_asset_volumes: vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            quote_asset_volumes: vec![100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0],
            pct_gaps: 0.0,
        };

        let config = AutoDurationConfig {
            relevancy_threshold: 0.15,
            min_lookback_days: 0, // Disable minimum lookback for this test
        };

        let ranges = find_relevant_ranges(&timeseries, 100.0, &config);

        // Should find two discontinuous ranges:
        // Range 1: indices 0-2 (candles at ~95-97, within range)
        // Skip: indices 2-4 (candles at ~120-130, outside range)
        // Range 2: indices 4-7 (candles at ~97-101, within range)
        assert_eq!(ranges.len(), 2, "Should find 2 discontinuous ranges");
        assert_eq!(ranges[0], (0, 2), "First range should be [0, 2)");
        assert_eq!(ranges[1], (4, 7), "Second range should be [4, 7)");
    }

    #[test]
    fn test_continuous_range() {
        // All candles within range - should return single continuous range
        let timeseries = OhlcvTimeSeries {
            pair_interval: PairInterval {
                name: "BTCUSDT".to_string(),
                interval_ms: 3600000,
            },
            first_kline_timestamp_ms: 0,
            open_prices: vec![95.0, 96.0, 97.0, 98.0, 99.0],
            high_prices: vec![96.0, 97.0, 98.0, 99.0, 100.0],
            low_prices: vec![94.0, 95.0, 96.0, 97.0, 98.0],
            close_prices: vec![96.0, 97.0, 98.0, 99.0, 100.0],
            base_asset_volumes: vec![1.0, 1.0, 1.0, 1.0, 1.0],
            quote_asset_volumes: vec![100.0, 100.0, 100.0, 100.0, 100.0],
            pct_gaps: 0.0,
        };

        let config = AutoDurationConfig {
            relevancy_threshold: 0.15,
            min_lookback_days: 0,
        };

        let ranges = find_relevant_ranges(&timeseries, 100.0, &config);

        assert_eq!(ranges.len(), 1, "Should find 1 continuous range");
        assert_eq!(ranges[0], (0, 5), "Range should span all candles");
    }
}
