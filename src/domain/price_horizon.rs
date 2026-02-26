use crate::{
    config::{HighPrice, LowPrice, PhPct, Price},
    models::OhlcvTimeSeries,
};

/// Select discontinuous slice ranges based on price relevancy.
pub fn auto_select_ranges(
    timeseries: &OhlcvTimeSeries,
    current_price: Price,
    ph_pct: PhPct,
) -> (Vec<(usize, usize)>, (LowPrice, HighPrice)) {
    let (price_min, price_max) = calculate_price_range(current_price, ph_pct);
    let ranges = crate::trace_time!("Scan All Candles", 3_000, {
        find_relevant_ranges(timeseries, price_min, price_max)
    });
    (ranges, (price_min, price_max))
}

/// Calculates the price range considered relevant to the current price.
pub fn calculate_price_range(current_price: Price, threshold: PhPct) -> (LowPrice, HighPrice) {
    let min = current_price * (1.0 - threshold.value());
    let max = current_price * (1.0 + threshold.value());
    (LowPrice::from(min), HighPrice::from(max))
}

/// Find all discontinuous ranges of candles where price is within the relevancy range.
fn find_relevant_ranges(
    timeseries: &OhlcvTimeSeries,
    price_min: LowPrice,
    price_max: HighPrice,
) -> Vec<(usize, usize)> {
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut range_start: Option<usize> = None;
    let total_candles = timeseries.klines();

    for i in 0..total_candles {
        let candle = timeseries.get_candle(i);
        let is_relevant = candle.low_price <= price_max && candle.high_price >= price_min;
        if is_relevant {
            if range_start.is_none() {
                range_start = Some(i);
            }
        } else {
            if let Some(start) = range_start {
                ranges.push((start, i));
                range_start = None;
            }
        }
    }
    if let Some(start) = range_start {
        ranges.push((start, total_candles));
    }
    if ranges.is_empty() {
        let last_idx = total_candles - 1;
        ranges.push((last_idx, total_candles));
    }
    ranges
}
