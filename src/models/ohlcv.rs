use {
    crate::{
        config::{
            BaseVol, ClosePrice, HighPrice, LowPrice, OpenPrice, Price, PriceLike, QuoteVol,
            VolRatio, VolatilityPct,
        },
        domain::{Candle, PairInterval},
        models::{CVACore, ScoreType},
    },
    anyhow::{Result, anyhow},
    serde::{Deserialize, Serialize},
};

const RVOL_WINDOW: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveCandle {
    pub symbol: String,
    pub open_time: i64,
    pub open: OpenPrice,
    pub high: HighPrice,
    pub low: LowPrice,
    pub close: ClosePrice,
    pub volume: BaseVol,
    pub quote_vol: QuoteVol,
    pub is_closed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OhlcvTimeSeries {
    pub pair_interval: PairInterval,
    pub first_kline_timestamp_ms: i64,
    pub timestamps: Vec<i64>,
    pub open_prices: Vec<OpenPrice>,
    pub high_prices: Vec<HighPrice>,
    pub low_prices: Vec<LowPrice>,
    pub close_prices: Vec<ClosePrice>,
    pub base_asset_volumes: Vec<BaseVol>,
    pub quote_asset_volumes: Vec<QuoteVol>,
    pub relative_volumes: Vec<VolRatio>,
}

pub(crate) fn find_matching_ohlcv<'a>(
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
    pub(crate) fn update_from_live(&mut self, candle: &LiveCandle) {
        if self.timestamps.is_empty() {
            return;
        }

        let last_idx = self.timestamps.len() - 1;
        let last_ts = self.timestamps[last_idx];
        let is_update = candle.open_time == last_ts;

        if is_update {
            self.high_prices[last_idx] = candle.high;
            self.low_prices[last_idx] = candle.low;
            self.close_prices[last_idx] = candle.close;
            self.base_asset_volumes[last_idx] = candle.volume;
            self.quote_asset_volumes[last_idx] = candle.quote_vol;

            let rvol = self.calculate_rvol_at_index(last_idx);
            if last_idx < self.relative_volumes.len() {
                self.relative_volumes[last_idx] = rvol;
            } else {
                self.relative_volumes.push(rvol);
            }
        } else {
            self.timestamps.push(candle.open_time);
            self.open_prices.push(candle.open);
            self.high_prices.push(candle.high);
            self.low_prices.push(candle.low);
            self.close_prices.push(candle.close);
            self.base_asset_volumes.push(candle.volume);
            self.quote_asset_volumes.push(candle.quote_vol);

            let new_idx = self.timestamps.len() - 1;
            let rvol = self.calculate_rvol_at_index(new_idx);
            self.relative_volumes.push(rvol);
        }
    }

    fn calculate_rvol_at_index(&self, idx: usize) -> VolRatio {
        let start = idx.saturating_sub(RVOL_WINDOW - 1);
        let slice = &self.base_asset_volumes[start..=idx];
        let sum: f64 = slice.iter().map(|v| v.value()).sum();
        let count = slice.len().max(1) as f64;
        let avg = sum / count;
        let current_vol = self.base_asset_volumes[idx].value();

        VolRatio::calculate(current_vol, avg)
    }

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

        let mut ts_vec = Vec::with_capacity(len);
        let mut open_vec = Vec::with_capacity(len);
        let mut high_vec = Vec::with_capacity(len);
        let mut low_vec = Vec::with_capacity(len);
        let mut close_vec = Vec::with_capacity(len);
        let mut base_vec = Vec::with_capacity(len);
        let mut quote_vec = Vec::with_capacity(len);
        let mut rvol_vec = Vec::with_capacity(len);

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

            rolling_sum += c.base_asset_volume.value();

            if i >= window_size {
                rolling_sum -= candles[i - window_size].base_asset_volume.value();
            }

            let count = (i + 1).min(window_size) as f64;
            let avg = rolling_sum / count;
            let rvol = VolRatio::calculate(c.base_asset_volume.value(), avg);
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

    /// Calculates average volatility ((High-Low)/Close) over range.
    /// Returns 0 if range is invalid or empty.
    pub(crate) fn calculate_volatility_in_range(
        &self,
        start_idx: usize,
        end_idx: usize,
    ) -> VolatilityPct {
        if start_idx >= end_idx || end_idx > self.close_prices.len() {
            return VolatilityPct::new(0.);
        }

        let mut sum_vol = 0.0;
        let mut count = 0;

        for i in start_idx..end_idx {
            let close = self.close_prices[i];
            if close.is_positive() {
                let high = self.high_prices[i].value();
                let low = self.low_prices[i].value();
                sum_vol += (high - low) / close.value();
                count += 1;
            }
        }

        if count > 0 {
            VolatilityPct::new(sum_vol / count as f64)
        } else {
            VolatilityPct::new(0.)
        }
    }
    pub(crate) fn get_candle(&self, idx: usize) -> Candle {
        Candle::new(
            self.timestamps[idx],
            self.open_prices[idx],
            self.high_prices[idx],
            self.low_prices[idx],
            self.close_prices[idx],
            self.base_asset_volumes[idx],
            self.quote_asset_volumes[idx],
        )
    }

    pub(crate) fn klines(&self) -> usize {
        self.open_prices.len()
    }
}

/// Windowed view into OhlcvTimeSeries for CVA generation.
/// Supports discontinuous ranges.
pub(crate) struct TimeSeriesSlice<'a> {
    pub series_data: &'a OhlcvTimeSeries,
    pub ranges: Vec<(usize, usize)>,
}

impl TimeSeriesSlice<'_> {
    pub(crate) fn generate_cva_results(
        &self,
        n_chunks: usize,
        pair_name: String,
        time_decay_factor: f64,
        price_range: (LowPrice, HighPrice),
    ) -> CVACore {
        let (min_price, max_price) = price_range;
        let total_candles: usize = self.ranges.iter().map(|(start, end)| end - start).sum();

        let mut volatility_sum = 0.0;
        for (start, end) in &self.ranges {
            for i in *start..*end {
                let candle = self.series_data.get_candle(i);
                if candle.close_price.is_positive() {
                    volatility_sum += (candle.high_price.value() - candle.low_price.value())
                        / candle.close_price.value();
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
            VolatilityPct::new(volatility_pct),
        );

        let mut position = 0;
        crate::trace_time!("CVA Math Loop", 8000, {
            for (start_idx, end_idx) in &self.ranges {
                for idx in *start_idx..*end_idx {
                    let candle = self.series_data.get_candle(idx);

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
                    let temporal_weight = decay_base.powf(progress);
                    self.process_candle_scores(&mut cva_core, &candle, temporal_weight);
                    position += 1;
                }
            }
        });

        cva_core
    }

    fn process_candle_scores(&self, cva_core: &mut CVACore, candle: &Candle, temporal_weight: f64) {
        let (price_min, price_max) = cva_core.price_range.min_max();
        let min_p = Price::from(price_min);
        let max_p = Price::from(price_max);
        let clamp = |price: Price| price.clamp(min_p, max_p);

        let candle_low = clamp(Price::from(candle.low_price));
        let candle_high = clamp(Price::from(candle.high_price));
        cva_core.distribute_conserved_volume(
            ScoreType::FullCandleTVW,
            candle_low,
            candle_high,
            candle.base_asset_volume.value() * temporal_weight,
        );

        let low_wick_start = clamp(Price::from(candle.low_wick_low()));
        let low_wick_end = clamp(Price::from(candle.low_wick_high()));
        cva_core.apply_rejection_impact(
            ScoreType::LowWickCount,
            low_wick_start,
            low_wick_end,
            temporal_weight,
        );

        let high_wick_start = clamp(Price::from(candle.high_wick_low()));
        let high_wick_end = clamp(Price::from(candle.high_wick_high()));
        cva_core.apply_rejection_impact(
            ScoreType::HighWickCount,
            high_wick_start,
            high_wick_end,
            temporal_weight,
        );
    }
}
