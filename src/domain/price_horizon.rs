use crate::models::OhlcvTimeSeries;

/// Automatically select discontinuous slice ranges based on price relevancy.
/// Returns a tuple: (Vector of ranges [(start, end)], (price_min, price_max)).
pub fn auto_select_ranges(
    timeseries: &OhlcvTimeSeries,
    current_price: f64,
    ph_pct: f64,
) -> (Vec<(usize, usize)>, (f64, f64)) {
    // 1. Calculate the user-defined price range
    let (price_min, price_max) = calculate_price_range(current_price, ph_pct);

    // 2. Find all ranges where price is relevant
    let ranges = crate::trace_time!("Scan All Candles", 3_000, {
        find_relevant_ranges(timeseries, price_min, price_max)
    });

    (ranges, (price_min, price_max))
}

/// Calculates the price range considered "relevant" to the current price.
pub fn calculate_price_range(current_price: f64, threshold: f64) -> (f64, f64) {
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

    // FIX: The Safety Anchor
    // If we found NOTHING (price is totally out of range),
    // grab the most recent candle so we have something to expand from.
    if ranges.is_empty() {
        let last_idx = total_candles - 1;
        ranges.push((last_idx, total_candles)); // Range of length 1
    }

    ranges
}

// Calculate the earliest timestamp (in ms since epoch) where relevant data begins
// pub fn calculate_relevant_start_timestamp(
//     timeseries: &OhlcvTimeSeries,
//     current_price: f64,
//     ph_pct: f64,
// ) -> i64 {
//     let (ranges, _) = auto_select_ranges(timeseries, current_price, ph_pct);

//     if let Some((start_idx, _)) = ranges.first() {
//         // Calculate timestamp based on index and interval
//         let start_offset = *start_idx as i64 * timeseries.pair_interval.interval_ms;
//         timeseries.first_kline_timestamp_ms + start_offset
//     } else {
//         0
//     }
// }
