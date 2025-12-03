use anyhow::{Result, anyhow};
use chrono::{DateTime, Duration, Utc};

use crate::domain::candle::Candle;
use crate::domain::pair_interval::PairInterval;
use crate::models::cva::{CVACore, ScoreType};

#[cfg(not(target_arch = "wasm32"))]
use crate::data::timeseries::bnapi_version::OhlcvTimeSeriesTemp;
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

// ============================================================================
// OhlcvTimeSeries: Raw time series data for a trading pair
// ============================================================================

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OhlcvTimeSeries {
    pub pair_interval: PairInterval,
    pub first_kline_timestamp_ms: i64,

    // Prices
    pub open_prices: Vec<f64>,
    pub high_prices: Vec<f64>,
    pub low_prices: Vec<f64>,
    pub close_prices: Vec<f64>,

    // Volumes
    pub base_asset_volumes: Vec<f64>,
    pub quote_asset_volumes: Vec<f64>,

    // Stats
    pub pct_gaps: f64,
}

#[cfg(not(target_arch = "wasm32"))]
macro_rules! convert_ohlcv_field {
    ($old_struct:expr, $field:ident) => {
        $old_struct
            .$field
            .into_par_iter()
            .map(|val| val.expect(concat!("Missing ", stringify!($field), " data")))
            .collect()
    };
}

#[cfg(not(target_arch = "wasm32"))]
impl From<OhlcvTimeSeriesTemp> for OhlcvTimeSeries {
    fn from(old_struct: OhlcvTimeSeriesTemp) -> Self {
        OhlcvTimeSeries {
            open_prices: convert_ohlcv_field!(old_struct, open_prices),
            high_prices: convert_ohlcv_field!(old_struct, high_prices),
            low_prices: convert_ohlcv_field!(old_struct, low_prices),
            close_prices: convert_ohlcv_field!(old_struct, close_prices),
            base_asset_volumes: convert_ohlcv_field!(old_struct, base_asset_volumes),
            quote_asset_volumes: convert_ohlcv_field!(old_struct, quote_asset_volumes),
            pair_interval: old_struct.pair_interval,
            first_kline_timestamp_ms: old_struct.first_kline_timestamp_ms,
            pct_gaps: old_struct.pct_gaps.unwrap_or(0.0),
        }
    }
}

pub fn find_matching_ohlcv<'a>(
    timeseries_data: &'a [OhlcvTimeSeries],
    pair_name: &str,
    interval_ms: i64,
) -> Result<&'a OhlcvTimeSeries> {
    timeseries_data
        .iter()
        .find(|ohlcv| {
            ohlcv.pair_interval.name == pair_name && ohlcv.pair_interval.interval_ms == interval_ms
        })
        .ok_or_else(|| {
            anyhow!(
                "No matching OHLCV data found for pair {} with interval {} ms",
                pair_name,
                interval_ms
            )
        })
}

impl OhlcvTimeSeries {
    pub fn get_candle(&self, idx: usize) -> Candle {
        Candle::new(
            self.open_prices[idx],
            self.high_prices[idx],
            self.low_prices[idx],
            self.close_prices[idx],
            self.base_asset_volumes[idx],
            self.quote_asset_volumes[idx],
        )
    }

    #[allow(dead_code)]
    pub fn klines(&self) -> usize {
        self.open_prices.len()
    }

    #[allow(dead_code)]
    pub fn last_kline_timestamp_ms(&self) -> i64 {
        self.first_kline_timestamp_ms
            + (((self.high_prices.len() - 1) as i64) * self.pair_interval.interval_ms)
    }

    #[allow(dead_code)]
    pub fn get_indices_by_time_range(
        &self,
        start_date: impl Into<DateTimeInput>,
        end_date: Option<impl Into<DateTimeInput>>,
    ) -> Option<(usize, usize)> {
        let start_ts_ms = start_date.into().to_milliseconds();
        let end_ts_ms = end_date.map(|d| d.into().to_milliseconds());

        let start_index_f64 = (start_ts_ms - self.first_kline_timestamp_ms) as f64
            / self.pair_interval.interval_ms as f64;
        let start_index = start_index_f64.ceil() as usize;

        let end_index = if let Some(end_ts_ms) = end_ts_ms {
            let end_index_f64 = (end_ts_ms - self.first_kline_timestamp_ms) as f64
                / self.pair_interval.interval_ms as f64;
            end_index_f64.floor() as usize + 1
        } else {
            self.open_prices.len()
        };

        if start_index >= self.open_prices.len() || start_index >= end_index {
            return None;
        }
        if end_index > self.open_prices.len() {
            return Some((start_index, self.open_prices.len()));
        }

        Some((start_index, end_index))
    }

    pub fn get_all_indices(&self) -> (usize, usize) {
        (0, self.open_prices.len())
    }

    /// Returns total duration covered by this timeseries in hours
    pub fn total_duration_hours(&self) -> usize {
        let num_candles = self.open_prices.len();
        let interval_hours = self.pair_interval.interval_ms / (1000 * 60 * 60);
        (num_candles as i64 * interval_hours) as usize
    }

    pub fn get_indices_most_recent(
        &self,
        most_recent_intervals: MostRecentIntervals,
    ) -> (usize, usize) {
        let final_num_intervals: usize = match most_recent_intervals {
            MostRecentIntervals::Count(n) => n,
            MostRecentIntervals::Duration(duration) => {
                let duration_ms = duration.num_milliseconds();
                let interval_ms = self.pair_interval.interval_ms;
                assert!(interval_ms > 0, "Interval interva ms must be positive");
                (duration_ms / self.pair_interval.interval_ms) as usize
            }
        };

        let total_intervals = self.open_prices.len();
        let start_index = total_intervals.saturating_sub(final_num_intervals);
        let end_index = total_intervals;

        (start_index, end_index)
    }
}

// ============================================================================
// TimeSeriesSlice: Windowed view into OhlcvTimeSeries with CVA generation
// ============================================================================

pub struct TimeSeriesSlice<'a> {
    pub series_data: &'a OhlcvTimeSeries,
    pub ranges: Vec<(usize, usize)>, // Vector of (start_idx, end_idx) where end_idx is exclusive
}

impl TimeSeriesSlice<'_> {
    /// Generate CVA results from this time slice (potentially discontinuous ranges)
    pub fn generate_cva_results(
        &self,
        n_chunks: usize,
        pair_name: String,
        time_decay_factor: f64,
        price_range: (f64, f64), // User-defined price range
    ) -> CVACore {
        let (min_price, max_price) = price_range;

        let mut cva_core =
            CVACore::new(min_price, max_price, n_chunks, pair_name, time_decay_factor);

        // Calculate total candles across all ranges
        let total_candles: usize = self.ranges.iter().map(|(start, end)| end - start).sum();

        // Process all candles across all ranges, maintaining temporal decay based on position
        let mut position = 0;
        for (start_idx, end_idx) in &self.ranges {
            for idx in *start_idx..*end_idx {
                let candle = self.series_data.get_candle(idx);

                // Exponential temporal decay based on position within relevant candles
                let progress = if total_candles > 1 {
                    position as f64 / (total_candles - 1) as f64
                } else {
                    1.0
                };

                let decay_base = if time_decay_factor < 0.01 {
                    0.01
                } else {
                    time_decay_factor
                };
                let temporal_weight = decay_base.powf(1.0 - progress);
                self.process_candle_scores(&mut cva_core, &candle, temporal_weight);
                position += 1;
            }
        }

        cva_core
    }


    fn process_candle_scores(&self, cva_core: &mut CVACore, candle: &Candle, temporal_weight: f64) {
        let (price_min, price_max) = cva_core.price_range.min_max();

        // Helper to clamp to analysis range
        let clamp = |price: f64| price.max(price_min).min(price_max);

        // 1. FULL CANDLE ANALYSIS (The new Sticky Logic)
        // We now use Low to High to capture the full range of price exploration
        let candle_low = clamp(candle.low_price);
        let candle_high = clamp(candle.high_price);

        // We use Base Asset Volume weighted by Time Decay.
        // The CVA function `increase_score_multi_zones_spread` handles the "Density" logic
        // by automatically dividing this weight by the number of zones covered.
        let weight = candle.base_volume * temporal_weight;

        cva_core.increase_score_multi_zones_spread(
            ScoreType::FullCandleTVW, 
            candle_low, 
            candle_high, 
            weight
        );

        // 2. Low wick volume-weighted (Reversal Zones - for later)
        let low_wick_start = clamp(candle.low_wick_low());
        let low_wick_end = clamp(candle.low_wick_high());
        // Note: You might want to apply the same base_volume * temporal_weight logic here too
        // but we can leave wicks as-is for now until you tackle Reversal Zones specifically.
        cva_core.increase_score_multi_zones_spread(
            ScoreType::LowWickVW,
            low_wick_start,
            low_wick_end,
            candle.base_volume * temporal_weight // Consistency: apply weighting here too?
        );

        // 3. High wick volume-weighted (Reversal Zones - for later)
        let high_wick_start = clamp(candle.high_wick_low());
        let high_wick_end = clamp(candle.high_wick_high());
        cva_core.increase_score_multi_zones_spread(
            ScoreType::HighWickVW,
            high_wick_start,
            high_wick_end,
            candle.base_volume * temporal_weight // Consistency: apply weighting here too?
        );

        // 4. Quote volume spread (Optional - keep if you use it for borders, otherwise remove)
        // If you keep it, use quote volume here as it's specifically for "QuoteVolume" score type
        let candle_start = clamp(candle.low_price);
        let candle_end = clamp(candle.high_price);
        cva_core.increase_score_multi_zones_spread(
            ScoreType::QuoteVolume,
            candle_start,
            candle_end,
            candle.quote_volume // No temporal weight? (Keep as is for now)
        );
    }
}

    
//     fn process_candle_scores_old(&self, cva_core: &mut CVACore, candle: &Candle, temporal_weight: f64) {
//         let (price_min, price_max) = cva_core.price_range.min_max();

//         // Skip candles entirely outside the CVA price range
//         if candle.high_price < price_min || candle.low_price > price_max {
//             return;
//         }

//         // Helper to clamp ranges to CVA bounds
//         let clamp = |price: f64| price.max(price_min).min(price_max);

//         // 1. Candle body volume-weighted (for sticky/slippy detection)
//         let body_start = clamp(candle.open_price.min(candle.close_price));
//         let body_end = clamp(candle.open_price.max(candle.close_price));
//         if body_start != body_end {
//             cva_core.increase_score_multi_zones_spread(
//                 ScoreType::CandleBodyVW,
//                 body_start,
//                 body_end,
//                 candle.quote_volume * temporal_weight,
//             );
//         }

//         // 2. Low wick volume-weighted (reversal zones)
//         let low_wick_start = clamp(candle.low_wick_low());
//         let low_wick_end = clamp(candle.low_wick_high());
//         if low_wick_start != low_wick_end {
//             let low_wick_score = candle.quote_volume * temporal_weight;
//             cva_core.increase_score_multi_zones_spread(
//                 ScoreType::LowWickVW,
//                 low_wick_start,
//                 low_wick_end,
//                 low_wick_score,
//             );
//         }

//         // 3. High wick volume-weighted (reversal zones)
//         let high_wick_start = clamp(candle.high_wick_low());
//         let high_wick_end = clamp(candle.high_wick_high());
//         if high_wick_start != high_wick_end {
//             let high_wick_score = candle.quote_volume * temporal_weight;
//             cva_core.increase_score_multi_zones_spread(
//                 ScoreType::HighWickVW,
//                 high_wick_start,
//                 high_wick_end,
//                 high_wick_score,
//             );
//         }

//         // 4. Quote volume spread across full candle range (for border detection)
//         let candle_start = clamp(candle.low_price);
//         let candle_end = clamp(candle.high_price);
//         if candle_start != candle_end {
//             cva_core.increase_score_multi_zones_spread(
//                 ScoreType::QuoteVolume,
//                 candle_start,
//                 candle_end,
//                 candle.quote_volume * temporal_weight,
//             );
//         }
//     }
// }

// ============================================================================
// Helper types
// ============================================================================

pub enum MostRecentIntervals {
    Count(usize),
    Duration(Duration),
}

pub enum DateTimeInput {
    TimestampMs(i64),
    ChronoDateTime(DateTime<Utc>),
}

impl DateTimeInput {
    pub fn to_milliseconds(&self) -> i64 {
        match self {
            DateTimeInput::TimestampMs(ts) => *ts,
            DateTimeInput::ChronoDateTime(dt) => dt.timestamp_millis(),
        }
    }
}

impl From<i64> for DateTimeInput {
    fn from(ts_ms: i64) -> Self {
        DateTimeInput::TimestampMs(ts_ms)
    }
}

impl From<DateTime<Utc>> for DateTimeInput {
    fn from(dt: DateTime<Utc>) -> Self {
        DateTimeInput::ChronoDateTime(dt)
    }
}
