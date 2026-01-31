use anyhow::{Result, anyhow};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};


use crate::config::{VolatilityPct, VolRatio};
use crate::domain::candle::Candle;
use crate::domain::pair_interval::PairInterval;

use crate::models::cva::{CVACore, ScoreType};

const RVOL_WINDOW: usize = 20;

// ============================================================================
// OhlcvTimeSeries: Raw time series data for a trading pair
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveCandle {
    pub symbol: String,
    pub open_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub quote_vol: f64,
    pub is_closed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OhlcvTimeSeries {
    pub pair_interval: PairInterval,
    pub first_kline_timestamp_ms: i64,

    pub timestamps: Vec<i64>,

    // Prices
    pub open_prices: Vec<f64>,
    pub high_prices: Vec<f64>,
    pub low_prices: Vec<f64>,
    pub close_prices: Vec<f64>,

    // Volumes
    pub base_asset_volumes: Vec<f64>,
    pub quote_asset_volumes: Vec<f64>,

    pub relative_volumes: Vec<VolRatio>,

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

    pub fn update_from_live(&mut self, candle: &LiveCandle) {
        if self.timestamps.is_empty() { return; }
        
        let last_idx = self.timestamps.len() - 1;
        let last_ts = self.timestamps[last_idx];

        // Logic: Is this an update to the current candle, or a new one?
        let is_update = candle.open_time == last_ts;

        if is_update {
            // Update current (forming) candle
            self.high_prices[last_idx] = candle.high;
            self.low_prices[last_idx] = candle.low;
            self.close_prices[last_idx] = candle.close;
            self.base_asset_volumes[last_idx] = candle.volume;
            self.quote_asset_volumes[last_idx] = candle.quote_vol;
            
            // Recalculate RVOL for this updating candle
            let rvol = self.calculate_rvol_at_index(last_idx);
            
            // Safety check for vector length sync
            if last_idx < self.relative_volumes.len() {
                self.relative_volumes[last_idx] = rvol;
            } else {
                self.relative_volumes.push(rvol);
            }

        } else {
            // New candle started
            self.timestamps.push(candle.open_time);
            self.open_prices.push(candle.open);
            self.high_prices.push(candle.high);
            self.low_prices.push(candle.low);
            self.close_prices.push(candle.close);
            self.base_asset_volumes.push(candle.volume);
            self.quote_asset_volumes.push(candle.quote_vol);
            
            // Calculate RVOL for the new candle
            let new_idx = self.timestamps.len() - 1;
            let rvol = self.calculate_rvol_at_index(new_idx);
            self.relative_volumes.push(rvol);
        }
    }


    /// Helper: Calculates Relative Volume for a specific index using existing data
    fn calculate_rvol_at_index(&self, idx: usize) -> VolRatio {
        let start = idx.saturating_sub(RVOL_WINDOW - 1);
        let slice = &self.base_asset_volumes[start..=idx];
        let sum: f64 = slice.iter().sum();
        let count = slice.len().max(1) as f64;
        let avg = sum / count;
        
        let current_vol = self.base_asset_volumes[idx];
        
        VolRatio::calculate(current_vol, avg)
    }

    // ... calculate_volatility_in_range implementation ...

    /// Create a TimeSeries from a list of Candles (Loaded from DB)
    pub fn from_candles(pair_interval: PairInterval, candles: Vec<Candle>) -> Self {
        if candles.is_empty() {
            return Self {
                pair_interval,
                first_kline_timestamp_ms: 0,
                timestamps: vec![],
                open_prices: vec![],
                high_prices: vec![],
                low_prices: vec![],
                close_prices: vec![],
                base_asset_volumes: vec![],
                quote_asset_volumes: vec![],
                relative_volumes: vec![],
            };
        }

        let len = candles.len();
        let first_ts = candles.first().map(|c| c.timestamp_ms).unwrap_or(0);

        // Pre-allocate everything
        let mut ts_vec = Vec::with_capacity(len);
        let mut open_vec = Vec::with_capacity(len);
        let mut high_vec = Vec::with_capacity(len);
        let mut low_vec = Vec::with_capacity(len);
        let mut close_vec = Vec::with_capacity(len);
        let mut base_vec = Vec::with_capacity(len);
        let mut quote_vec = Vec::with_capacity(len);
        let mut rvol_vec = Vec::with_capacity(len);

        // Optimization: Rolling Sum for RVOL
        let mut rolling_sum = 0.0;
        let window_size = RVOL_WINDOW; 

        for (i, c) in candles.iter().enumerate() {
            ts_vec.push(c.timestamp_ms);
            open_vec.push(c.open_price);
            high_vec.push(c.high_price);
            low_vec.push(c.low_price);
            close_vec.push(c.close_price);
            base_vec.push(c.base_asset_volume);
            quote_vec.push(c.quote_asset_volume);

            // --- RVOL Calculation (O(1) Rolling) ---
            rolling_sum += c.base_asset_volume;

            if i >= window_size {
                // Subtract the element that fell out of the window
                rolling_sum -= candles[i - window_size].base_asset_volume;
            }

            // Count is i+1 until we hit window_size, then it stays at window_size
            let count = (i + 1).min(window_size) as f64;
            let avg = rolling_sum / count;

            let rvol = VolRatio::calculate(c.base_asset_volume, avg);
            rvol_vec.push(rvol);
        }

        Self {
            pair_interval,
            first_kline_timestamp_ms: first_ts,
            timestamps: ts_vec,
            open_prices: open_vec,
            high_prices: high_vec,
            low_prices: low_vec,
            close_prices: close_vec,
            base_asset_volumes: base_vec,
            quote_asset_volumes: quote_vec,
            relative_volumes: rvol_vec,
        }
    }


    /// Calculates the Average Volatility ((High-Low)/Close) over a range of indices.
    /// Returns 0.0 if range is invalid or empty.
    pub fn calculate_volatility_in_range(&self, start_idx: usize, end_idx: usize) -> VolatilityPct {
        if start_idx >= end_idx || end_idx > self.close_prices.len() {
            return VolatilityPct::new(0.);
        }
        
        let mut sum_vol = 0.0;
        let mut count = 0;

        for i in start_idx..end_idx {
            let close = self.close_prices[i];
            if close > f64::EPSILON {
                let high = self.high_prices[i];
                let low = self.low_prices[i];
                sum_vol += (high - low) / close;
                count += 1;
            }
        }

        if count > 0 {
            VolatilityPct::new(sum_vol / count as f64)
        } else {
            VolatilityPct::new(0.)
        }
    }

    pub fn get_candle(&self, idx: usize) -> Candle {
        // Direct access since the vectors are already f64
        let open = self.open_prices[idx];
        let high = self.high_prices[idx];
        let low = self.low_prices[idx];
        let close = self.close_prices[idx];
        let base_vol = self.base_asset_volumes[idx];
        let quote_vol = self.quote_asset_volumes[idx];

        let timestamp = self.timestamps[idx];

        Candle::new(timestamp, open, high, low, close, base_vol, quote_vol)
    }

    pub fn klines(&self) -> usize {
        self.open_prices.len()
    }

    pub fn get_all_indices(&self) -> (usize, usize) {
        (0, self.open_prices.len())
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

        // Calculate total candles across all ranges
        let total_candles: usize = self.ranges.iter().map(|(start, end)| end - start).sum();

        // NEW: Calculate Volatility here or pass it in?
        // Let's calculate it here to keep pair_analysis cleaner,
        // iterating the ranges we already have.
        let mut volatility_sum = 0.0;
        for (start, end) in &self.ranges {
            for i in *start..*end {
                let candle = self.series_data.get_candle(i);
                if candle.close_price > f64::EPSILON {
                    volatility_sum += (candle.high_price - candle.low_price) / candle.close_price;
                }
            }
        }
        let volatility_pct = if total_candles > 0 {
            volatility_sum / total_candles as f64
        } else {
            0.0
        };

        let mut cva_core = CVACore::new(
            min_price,
            max_price,
            n_chunks,
            pair_name,
            time_decay_factor,
            self.series_data.open_prices.len(),
            total_candles,
            self.series_data.pair_interval.interval_ms,
            volatility_pct * 100.0,
        );

        // Process all candles across all ranges, maintaining temporal decay based on position
        let mut position = 0;
        crate::trace_time!("CVA Math Loop", 8000, {
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
                    let temporal_weight = decay_base.powf(progress); // powf() call takes around 30ns in release build. Fairly reasonable
                    self.process_candle_scores(&mut cva_core, &candle, temporal_weight);
                    position += 1;
                }
            }
        });

        cva_core
    }

    #[inline]
    fn process_candle_scores(&self, cva_core: &mut CVACore, candle: &Candle, temporal_weight: f64) {
        let (price_min, price_max) = cva_core.price_range.min_max();
        let clamp = |price: f64| price.max(price_min).min(price_max);

        // 1. FULL CANDLE (Sticky Zones) - Keep Volume Weighting
        let candle_low = clamp(candle.low_price);
        let candle_high = clamp(candle.high_price);
        cva_core.distribute_conserved_volume(
            ScoreType::FullCandleTVW,
            candle_low,
            candle_high,
            candle.base_asset_volume * temporal_weight,
        );

        // 2. LOW WICK - USE FLAT LOGIC
        let low_wick_start = clamp(candle.low_wick_low());
        let low_wick_end = clamp(candle.low_wick_high());

        cva_core.apply_rejection_impact( 
            ScoreType::LowWickCount,
            low_wick_start,
            low_wick_end,
            temporal_weight, 
        );

        // 3. HIGH WICK - USE FLAT LOGIC
        let high_wick_start = clamp(candle.high_wick_low());
        let high_wick_end = clamp(candle.high_wick_high());

        cva_core.apply_rejection_impact( 
            ScoreType::HighWickCount,
            high_wick_start,
            high_wick_end,
            temporal_weight,
        );

        // // 5. Quote Volume - Keep Spread
        // cva_core.distribute_conserved_volume(
        //     ScoreType::QuoteVolume,
        //     candle_low,
        //     candle_high,
        //     candle.quote_asset_volume,
        // );

    }
}

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
