use crate::config::ANALYSIS; // Use global config for defaults, or passed config
use crate::data::timeseries::TimeSeriesCollection;
use crate::domain::auto_duration;
use crate::domain::auto_duration::AutoDurationConfig;
use crate::models::cva::CVACore;
use crate::models::timeseries::{TimeSeriesSlice, find_matching_ohlcv};
use anyhow::{Context, Result, bail};

// --- NEW PURE FUNCTION FOR THE ENGINE ---

/// Calculates CVA for a pair given a specific price and configuration.
/// This runs entirely isolated from the UI state.
pub fn pair_analysis_pure(
    pair_name: String,
    timeseries_data: &TimeSeriesCollection,
    current_price: f64,
    auto_duration_config: &AutoDurationConfig,
) -> Result<CVACore> {
    // Use Constants from Config
    let zone_count = ANALYSIS.zone_count;
    let time_decay_factor = ANALYSIS.time_decay_factor;

    // 1. Find the Data
    // find_matching_ohlcv returns Result, so we use with_context to add the error message
    let ohlcv_time_series = find_matching_ohlcv(
        &timeseries_data.series_data,
        &pair_name,
        ANALYSIS.interval_width_ms,
    )
    .with_context(|| format!("No OHLCV data found for {}", pair_name))?;

    // 2. Auto-Duration: Calculate relevant slices based on price
    // Note: The Engine calculates this fresh every time. No "Slice Caching".
    let (slice_ranges, price_range) =
        auto_duration::auto_select_ranges(ohlcv_time_series, current_price, auto_duration_config);

    // 3. Validation
    let total_candle_count: usize = slice_ranges.iter().map(|(start, end)| end - start).sum();
    if total_candle_count < ANALYSIS.cva.min_candles_for_analysis {
        bail!(
            "Insufficient data: {} has only {} candles (minimum: {}).",
            pair_name,
            total_candle_count,
            ANALYSIS.cva.min_candles_for_analysis
        );
    }

    // 4. Dynamic Decay Logic (Annualized)
    let start_idx = slice_ranges.first().map(|r| r.0).unwrap_or(0);
    let end_idx = slice_ranges.last().map(|r| r.1).unwrap_or(0);

    let duration_years = if end_idx > start_idx {
        let duration_ms = (end_idx - start_idx) as f64 * ANALYSIS.interval_width_ms as f64;
        let millis_per_year = 31_536_000_000.0;
        duration_ms / millis_per_year
    } else {
        0.0
    };

    let dynamic_decay_factor = if duration_years > 0.0 {
        time_decay_factor.powf(duration_years).max(1.0)
    } else {
        1.0
    };

    // 5. Generate CVA
    let timeseries_slice = TimeSeriesSlice {
        series_data: ohlcv_time_series,
        ranges: slice_ranges.clone(),
    };

    let mut cva_results = timeseries_slice.generate_cva_results(
        zone_count,
        pair_name.clone(),
        dynamic_decay_factor,
        price_range,
    );

    // 6. Add Metadata
    let first_kline_timestamp = ohlcv_time_series.first_kline_timestamp_ms;
    if let (Some((first_start, _)), Some((_, last_end))) =
        (slice_ranges.first(), slice_ranges.last())
    {
        cva_results.start_timestamp_ms =
            first_kline_timestamp + (*first_start as i64 * ANALYSIS.interval_width_ms);
        cva_results.end_timestamp_ms =
            first_kline_timestamp + (*last_end as i64 * ANALYSIS.interval_width_ms);
    }

    Ok(cva_results)
}
