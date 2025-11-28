use anyhow::Result;
use std::fmt;

use crate::data::timeseries::bnapi_version::{AllValidKlines4Pair, PairInterval};
use crate::utils::{maths_utils, vec_utils};

// MAX_PCT_MISSING_KLINES_ALLOWED is a delimiter. If BN klines data has < % of missing klines than this, we simply forward-fill the missing data.
// But if the BN Klines data has > % of missing klines than this, we instead cut-off ALL missing klines, and then just deal with the `pure` data afterwards. So then guaranteed not to be filling in any values but end up much less data (but 100% pure.)
const MAX_PCT_MISSING_KLINES_ALLOWED: f64 = 10.; // 0.5; // = 0.01 = 1%

pub struct OhlcvTimeSeriesTemp {
    pub pair_interval: PairInterval,
    pub first_kline_timestamp_ms: i64, // when this timeseries starts (expressed as epoch offset)

    // Now the prices
    pub open_prices: Vec<Option<f64>>,
    pub high_prices: Vec<Option<f64>>,
    pub low_prices: Vec<Option<f64>>,
    pub close_prices: Vec<Option<f64>>,

    // Volumes
    pub base_asset_volumes: Vec<Option<f64>>, // This is `volume` from binance kline structure
    pub quote_asset_volumes: Vec<Option<f64>>, // This is `quote_asset_volume` from bn kline structure

    // Stats
    pub pct_gaps: Option<f64>,
}

impl OhlcvTimeSeriesTemp {}

#[derive(Debug)]
#[allow(dead_code)]
pub enum KlinesPreparationError {
    ExceedMaxGaps {
        gap_pct: f64,
        gap_pct_limit: f64,
        pair_interval: PairInterval,
    },
}

impl std::error::Error for KlinesPreparationError {} // Seems unnecessary but apparently useful
impl fmt::Display for KlinesPreparationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KlinesPreparationError::ExceedMaxGaps {
                gap_pct,
                gap_pct_limit,
                pair_interval,
            } => {
                write!(
                    f,
                    "{} because it has {:.2}% gaps, > accepted gap limit of {:.2}% ",
                    pair_interval, gap_pct, gap_pct_limit
                )
            }
        }
    }
}

/// Preferred Conversion Method: Implementing the `From` trait
impl TryFrom<AllValidKlines4Pair> for OhlcvTimeSeriesTemp {
    type Error = KlinesPreparationError;

    fn try_from(klines: AllValidKlines4Pair) -> Result<Self, Self::Error> {
        let number_klines_needed: usize = maths_utils::intervals(
            klines.first_timestamp_ms(),
            klines.last_timestamp_ms(),
            klines.pair_interval.interval_ms,
        )
        .try_into()
        .unwrap();
        let mut time_series = OhlcvTimeSeriesTemp {
            pair_interval: klines.pair_interval.clone(),
            first_kline_timestamp_ms: klines.klines[0].open_timestamp_ms,
            // Initialize all our vectors to None
            open_prices: vec![None; number_klines_needed],
            high_prices: vec![None; number_klines_needed],
            low_prices: vec![None; number_klines_needed],
            close_prices: vec![None; number_klines_needed],
            base_asset_volumes: vec![None; number_klines_needed],
            quote_asset_volumes: vec![None; number_klines_needed],
            pct_gaps: None,
        };

        // Loop through original klines data and map source data to destination data
        for source_kline in klines.klines {
            let kline_index = maths_utils::index_into_range(
                time_series.first_kline_timestamp_ms,
                source_kline.open_timestamp_ms,
                time_series.pair_interval.interval_ms,
            ) as usize;

            #[cfg(debug_assertions)]
            {
                assert!(
                    kline_index < number_klines_needed,
                    "Calcluated index {} was not less than {}",
                    kline_index,
                    number_klines_needed,
                );
            }

            time_series.open_prices[kline_index] = source_kline.open_price;
            time_series.high_prices[kline_index] = source_kline.high_price;
            time_series.close_prices[kline_index] = source_kline.close_price;
            time_series.low_prices[kline_index] = source_kline.low_price;
            time_series.base_asset_volumes[kline_index] = source_kline.base_asset_volume;
            time_series.quote_asset_volumes[kline_index] = source_kline.quote_asset_volume;
        }

        // Detect whether the gap total in price data is "too big"
        // If it is, run code that cuts off *all* gaps up until the last gap
        let open_price_none_pct = vec_utils::count_pct_none_elements(&time_series.open_prices);
        // log::info!("% of open price is {}", open_price_none_pct);
        time_series.pct_gaps = Some(open_price_none_pct);
        if open_price_none_pct > MAX_PCT_MISSING_KLINES_ALLOWED {
            #[cfg(debug_assertions)]
            log::info!(
                "{} has {:.2}% gaps, which is above our limit ({:.2}%), so serious surgery required. ",
                time_series.pair_interval,
                vec_utils::count_pct_none_elements(&time_series.open_prices),
                MAX_PCT_MISSING_KLINES_ALLOWED,
            );
            // Find last None index, we will cut from this + 1
            let last_none_index = vec_utils::find_last_none_index(&time_series.open_prices);
            log::info!("We have found last none index at {}", last_none_index);
            log::info!(
                "Before draining, open_prices was of size: {}",
                time_series.open_prices.len()
            );
            // Drain all vectors so they cut off up to this index
            let removed_count = time_series.open_prices.drain(..last_none_index).count();
            #[cfg(debug_assertions)]
            log::info!(
                "After draining, open_prices is of size: {} and we removed {} items",
                time_series.open_prices.len(),
                removed_count
            );
            time_series.high_prices.drain(..last_none_index);
            time_series.low_prices.drain(..last_none_index);
            time_series.close_prices.drain(..last_none_index);
            time_series.base_asset_volumes.drain(..last_none_index);
            time_series.quote_asset_volumes.drain(..last_none_index);
            // Finally, adjust value of time_series.first_kline_timestamp_ms...
            // How to do that? Just add on removed_count * interval_ms to old
            time_series.first_kline_timestamp_ms +=
                removed_count as i64 * time_series.pair_interval.interval_ms;
            time_series.pct_gaps = None;
        }
        // Now go through each vector and forward fill (with default as 0?) any None values
        let default_price = 9999.99999; // Should never get filled in...
        let default_volume = 0.0; // Might happen occassionally

        let mut kline_gaps: Vec<u32> = Vec::new();

        if vec_utils::has_any_none_elements(&time_series.open_prices) {
            kline_gaps.push(vec_utils::fill_forward_mut(
                &mut time_series.open_prices,
                default_price,
            ));
        }
        if vec_utils::has_any_none_elements(&time_series.high_prices) {
            kline_gaps.push(vec_utils::fill_forward_mut(
                &mut time_series.high_prices,
                default_price,
            ));
        }
        if vec_utils::has_any_none_elements(&time_series.low_prices) {
            kline_gaps.push(vec_utils::fill_forward_mut(
                &mut time_series.low_prices,
                default_price,
            ));
        }
        if vec_utils::has_any_none_elements(&time_series.close_prices) {
            kline_gaps.push(vec_utils::fill_forward_mut(
                &mut time_series.close_prices,
                default_price,
            ));
        }
        if vec_utils::has_any_none_elements(&time_series.base_asset_volumes) {
            kline_gaps.push(vec_utils::fill_forward_mut(
                &mut time_series.base_asset_volumes,
                default_volume,
            ));
        }
        if vec_utils::has_any_none_elements(&time_series.quote_asset_volumes) {
            kline_gaps.push(vec_utils::fill_forward_mut(
                &mut time_series.quote_asset_volumes,
                default_volume,
            ));
        }

        // Warn if kline gaps are uneven between different kline members
        if !vec_utils::are_all_elements_same(&kline_gaps) {
            #[cfg(debug_assertions)]
            log::error!(
                "For some reason the kline gaps are not all equal. This suggests not just gaps in data, but the cases where BN has data for one or more elements of the kline but not others e.g. has high_price but not low_price, for some kline(s). Here is actual data: {:?}",
                kline_gaps
            );
        }
        // Debug check whether any None values left .....
        #[cfg(debug_assertions)]
        {
            if vec_utils::has_any_none_elements(&time_series.open_prices)
                || vec_utils::has_any_none_elements(&time_series.high_prices)
                || vec_utils::has_any_none_elements(&time_series.low_prices)
                || vec_utils::has_any_none_elements(&time_series.close_prices)
                || vec_utils::has_any_none_elements(&time_series.base_asset_volumes)
                || vec_utils::has_any_none_elements(&time_series.quote_asset_volumes)
            {
                panic!("We shouldn not have any None values left but it seems we have....");
            }
        }
        Ok(time_series) // This is interrim time_series structure.....
    }
}
