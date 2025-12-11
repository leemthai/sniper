use serde::{Deserialize, Serialize};
use crate::models::OhlcvTimeSeries;

/// Configuration for the Price Horizon.
/// Determines the vertical price range of interest relative to the current price.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceHorizonConfig {
    /// Percentage threshold for price relevancy (e.g., 0.15 = ±15%)
    pub threshold_pct: f64,
    /// Minimum lookback period in days
    pub min_lookback_days: usize,
}

/// Automatically select discontinuous slice ranges based on price relevancy.
/// Returns a tuple: (Vector of ranges [(start, end)], (price_min, price_max)).
pub fn auto_select_ranges(
    timeseries: &OhlcvTimeSeries,
    current_price: f64,
    config: &PriceHorizonConfig,
) -> (Vec<(usize, usize)>, (f64, f64)) {
    // 1. Calculate the user-defined price range
    let (price_min, price_max) = calculate_price_range(current_price, config.threshold_pct);

    // 2. Find all ranges where price is relevant
    let mut ranges = find_relevant_ranges(timeseries, price_min, price_max);

    // 3. Apply minimum lookback constraint
    ranges = apply_min_lookback_constraint(ranges, timeseries, config.min_lookback_days);

    (ranges, (price_min, price_max))
}

/// Calculates the price range considered "relevant" to the current price.
fn calculate_price_range(current_price: f64, threshold: f64) -> (f64, f64) {
    let min = current_price * (1.0 - threshold);
    let max = current_price * (1.0 + threshold);
    (min, max)
}

/// Find all discontinuous ranges of candles where price is within the relevancy range.
fn find_relevant_ranges(
    timeseries: &OhlcvTimeSeries,
    price_min: f64,
    price_max: f64,
) -> Vec<(usize, usize)> {
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut range_start: Option<usize> = None;
    let total_candles = timeseries.klines();

    for i in 0..total_candles {
        let candle = timeseries.get_candle(i);
        
        // Check if candle overlaps with relevant price range.
        // Overlap exists if candle_low <= range_max AND candle_high >= range_min.
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

/// Apply minimum lookback constraint to ranges.
/// Ensures we have at least the minimum number of candles by extending backward.
fn apply_min_lookback_constraint(
    ranges: Vec<(usize, usize)>,
    timeseries: &OhlcvTimeSeries,
    min_lookback_days: usize,
) -> Vec<(usize, usize)> {
    if ranges.is_empty() {
        return ranges;
    }

    // Calculate how many candles we currently have
    let total_relevant_candles: usize = ranges.iter().map(|(start, end)| end - start).sum();
    
    // Calculate minimum candles needed based on time
    // (min_days * 24 * 60 * 60 * 1000) / interval_ms
    let interval_ms = timeseries.pair_interval.interval_ms;
    if interval_ms == 0 { return ranges; } // Safety check

    let ms_needed = min_lookback_days as u128 * 24 * 60 * 60 * 1000;
    let min_candles_needed = (ms_needed / interval_ms as u128) as usize;

    // If we already have enough, return as-is
    if total_relevant_candles >= min_candles_needed {
        return ranges;
    }

    // Otherwise, extend the earliest range backward to meet minimum
    let deficit = min_candles_needed - total_relevant_candles;
    
    let mut extended_ranges = ranges.clone();
    
    // We extend the *first* range (chronologically earliest) backwards
    if let Some(first_range) = extended_ranges.first_mut() {
        // saturating_sub ensures we don't go below index 0
        let new_start = first_range.0.saturating_sub(deficit);
        first_range.0 = new_start;
    }

    extended_ranges
}

/// Calculate the earliest timestamp (in ms since epoch) where relevant data begins
pub fn calculate_relevant_start_timestamp(
    timeseries: &OhlcvTimeSeries,
    current_price: f64,
    config: &PriceHorizonConfig,
) -> i64 {
    let (ranges, _) = auto_select_ranges(timeseries, current_price, config);
    
    if let Some((start_idx, _)) = ranges.first() {
        // Calculate timestamp based on index and interval
        let start_offset = *start_idx as i64 * timeseries.pair_interval.interval_ms;
        timeseries.first_kline_timestamp_ms + start_offset
    } else {
        0
    }
}