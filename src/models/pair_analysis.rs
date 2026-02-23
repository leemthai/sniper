use {
    crate::{
        config::{BASE_INTERVAL, PhPct, Price, TIME_DECAY_FACTOR, ZONE_COUNT},
        data::TimeSeriesCollection,
        domain::auto_select_ranges,
        models::{CVACore, MIN_CANDLES_FOR_ANALYSIS, TimeSeriesSlice, find_matching_ohlcv},
    },
    anyhow::{Context, Result, bail},
};

/// Calculates CVA for a pair given price and configuration.
/// Runs isolated from UI state.
pub(crate) fn pair_analysis_pure(
    pair_name: String,
    timeseries_data: &TimeSeriesCollection,
    current_price: Price,
    ph_pct: PhPct,
) -> Result<CVACore> {
    let ohlcv_time_series = find_matching_ohlcv(
        &timeseries_data.series_data,
        &pair_name,
        BASE_INTERVAL.as_millis() as i64,
    )
    .with_context(|| format!("No OHLCV data found for {}", pair_name))?;

    let (slice_ranges, price_range) = auto_select_ranges(ohlcv_time_series, current_price, ph_pct);

    let total_candle_count: usize = slice_ranges.iter().map(|(start, end)| end - start).sum();

    if total_candle_count < MIN_CANDLES_FOR_ANALYSIS {
        let s = if total_candle_count == 1 { "" } else { "s" };
        bail!(
            "Insufficient data: {} has only {} candle{} (minimum: {}).",
            pair_name,
            total_candle_count,
            s,
            MIN_CANDLES_FOR_ANALYSIS
        );
    }

    // Calculate time-based decay factor using real timestamps (handles discontinuous ranges)
    let dynamic_decay_factor = if (TIME_DECAY_FACTOR - 1.0).abs() < f64::EPSILON {
        1.0
    } else {
        let start_idx = slice_ranges.first().map(|r| r.0).unwrap_or(0);
        let end_idx = slice_ranges.last().map(|r| r.1).unwrap_or(0);
        let max_idx = ohlcv_time_series.klines().saturating_sub(1);
        let actual_start_idx = start_idx.min(max_idx);
        let actual_end_idx = end_idx.saturating_sub(1).min(max_idx);

        let start_ts = ohlcv_time_series.get_candle(actual_start_idx).timestamp_ms;
        let end_ts = ohlcv_time_series.get_candle(actual_end_idx).timestamp_ms;
        let duration_ms = end_ts.saturating_sub(start_ts);
        let duration_years = duration_ms as f64 / 31_536_000_000.0;

        if duration_years > 0.0 {
            TIME_DECAY_FACTOR.powf(duration_years).max(1.0)
        } else {
            1.0
        }
    };

    let timeseries_slice = TimeSeriesSlice {
        series_data: ohlcv_time_series,
        ranges: slice_ranges.clone(),
    };

    let mut cva_results = timeseries_slice.generate_cva_results(
        ZONE_COUNT,
        pair_name.clone(),
        dynamic_decay_factor,
        price_range,
    );

    cva_results.included_ranges = slice_ranges.clone();
    cva_results.relevant_candle_count = total_candle_count;

    if let (Some((first_start, _)), Some((_, last_end))) =
        (slice_ranges.first(), slice_ranges.last())
    {
        let max_idx = ohlcv_time_series.klines().saturating_sub(1);
        let start_idx = (*first_start).min(max_idx);
        let end_idx = (last_end.saturating_sub(1)).min(max_idx); // end is exclusive

        cva_results.start_timestamp_ms = ohlcv_time_series.get_candle(start_idx).timestamp_ms;
        cva_results.end_timestamp_ms = ohlcv_time_series.get_candle(end_idx).timestamp_ms;
    }

    Ok(cva_results)
}
