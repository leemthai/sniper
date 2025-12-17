use anyhow::{Context, Result, bail};

use crate::config::ANALYSIS; // Use global config for defaults, or passed config
use crate::config::PriceHorizonConfig;
use crate::data::timeseries::TimeSeriesCollection;
use crate::domain::price_horizon;
use crate::models::cva::CVACore;
use crate::models::timeseries::{TimeSeriesSlice, find_matching_ohlcv};

// --- NEW PURE FUNCTION FOR THE ENGINE ---

/// Calculates CVA for a pair given a specific price and configuration.
/// This runs entirely isolated from the UI state.
pub fn pair_analysis_pure(
    pair_name: String,
    timeseries_data: &TimeSeriesCollection,
    current_price: f64,
    price_horizon_config: &PriceHorizonConfig,
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

    // 2. Price Horizon: Calculate relevant slices based on price
    // Note: The Engine calculates this fresh every time. No "Slice Caching".
    let (slice_ranges, price_range) =
        price_horizon::auto_select_ranges(ohlcv_time_series, current_price, price_horizon_config);

    // 3. Validation
    let total_candle_count: usize = slice_ranges.iter().map(|(start, end)| end - start).sum();

    if total_candle_count < ANALYSIS.cva.min_candles_for_analysis {
        let s = if total_candle_count == 1 { "" } else { "s" };

        bail!(
            "Insufficient data: {} has only {} candle{} (minimum: {}).",
            pair_name,
            total_candle_count,
            s,
            ANALYSIS.cva.min_candles_for_analysis
        );
    }

    // 4. Dynamic Decay Logic (Optimized & Accordion-Aware)
    let dynamic_decay_factor = if (time_decay_factor - 1.0).abs() < f64::EPSILON {
        // Optimization: If decay is 1.0, multiplier is 1.0. Skip math.
        1.0
    } else {
        let start_idx = slice_ranges.first().map(|r| r.0).unwrap_or(0);
        let end_idx = slice_ranges.last().map(|r| r.1).unwrap_or(0);

        // Safeguard indices
        let max_idx = ohlcv_time_series.klines().saturating_sub(1);
        let actual_start_idx = start_idx.min(max_idx);
        let actual_end_idx = end_idx.saturating_sub(1).min(max_idx);

        // FIX: Use Real Timestamps from the DB (Accordion Fix)
        let start_ts = ohlcv_time_series.get_candle(actual_start_idx).timestamp_ms;
        let end_ts = ohlcv_time_series.get_candle(actual_end_idx).timestamp_ms;

        let duration_ms = end_ts.saturating_sub(start_ts);
        let millis_per_year = 31_536_000_000.0;
        let duration_years = duration_ms as f64 / millis_per_year;

        if duration_years > 0.0 {
            let factor = time_decay_factor.powf(duration_years).max(1.0);
            log::info!("pair_analysis_pure(): time_decay_factor is not 1.0. It gets generated from duration_years being {:.2} therefore time_decay_factor is {:.2}", duration_years, factor);
            factor
        } else {
            1.0
        }
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
