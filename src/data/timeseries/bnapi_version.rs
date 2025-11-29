pub mod bn_kline;
pub mod raw_ohlcv;

use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;
use bn_kline::AllValidKlines4Pair;
use futures::future::join_all;
use itertools::iproduct;
use rayon::prelude::*;
use tokio::{fs, task::JoinError, task::JoinHandle, time::Instant};

use crate::config::ANALYSIS;
use crate::config::BINANCE;
use crate::config::PERSISTENCE;
use crate::data::timeseries::{CreateTimeSeriesData, TimeSeriesCollection};
use crate::domain::pair_interval::PairInterval;
use crate::models::OhlcvTimeSeries;
pub use raw_ohlcv::OhlcvTimeSeriesTemp;

#[cfg(debug_assertions)]
use crate::utils::time_utils;

pub struct BNAPIVersion;
#[async_trait]
impl CreateTimeSeriesData for BNAPIVersion {
    fn signature(&self) -> &'static str {
        "Binance API"
    }

    async fn create_timeseries_data(&self) -> Result<TimeSeriesCollection> {
        // Load timeseries (klines) data from a pair list stored in text file
        // Interval is configured via INTERVAL_WIDTH_TO_ANALYSE_MS constant
        let supply_interval_asset = vec![ANALYSIS.interval_width_ms];
        let start_time = Instant::now();

        let series_data = timeseries_data_load(&supply_interval_asset).await?;
        #[cfg(debug_assertions)]
        log::info!(
            "\n...After loading all we have complete timeseries data for {} valid BN pairs. ",
            series_data.len(),
        );
        #[cfg(debug_assertions)]
        {
            for ts in &series_data {
                log::info!(
                    "{} (started on {}, ended on {}) with {} klines and {:.2}% gaps",
                    ts.pair_interval,
                    time_utils::epoch_ms_to_utc(ts.first_kline_timestamp_ms),
                    time_utils::epoch_ms_to_utc(ts.last_kline_timestamp_ms()),
                    ts.klines(),
                    ts.pct_gaps,
                );
            }
        }

        let elapsed_time = start_time.elapsed(); // Calculate the elapsed time
        log::info!("Main function executed in: {:?}", elapsed_time);

        Ok(TimeSeriesCollection {
            name: "Binance TimeSeries Collection".to_string(),
            version: PERSISTENCE.kline.version,
            series_data,
        })
    }
}

pub async fn timeseries_data_load(
    // supply_base_asset: &[&str],
    // supply_quote_asset: &[&str],
    // supply_name: &[&str],
    supply_interval_asset: &[i64],
) -> Result<Vec<OhlcvTimeSeries>> {
    let mut all_valid_klines_4_pairs: Vec<AllValidKlines4Pair> = Vec::new();

    let pairs_file_content = fs::read_to_string("pairs.txt").await?; // On fail, return Err from this func.
    // Create the Vec<String> from the file content - TEMP  - what if this fails?
    let supply_pairs: Vec<String> = pairs_file_content
        .lines()
        .map(|s| s.trim().to_uppercase()) // Trim whitespace and make uppercase
        .filter(|s| !s.is_empty()) // Filter out empty lines
        .take(BINANCE.max_pairs)
        .collect();

    let all_permutations = iproduct!(supply_pairs, supply_interval_asset)
        .take(BINANCE.limits.max_lookups_total)
        .map(|(pair_name, interval_ms)| PairInterval {
            name: pair_name,
            interval_ms: *interval_ms,
        });

    // Collect all_permutations into an owned collection so data can be safely sent to other threads
    let all_permutations_vec: Vec<_> = all_permutations.collect();
    for batch in all_permutations_vec.chunks(BINANCE.limits.simultaneous_calls_ceiling) {
        // `batch` is a new iterator for each chunk.
        // `batch.iter().collect()` turns it into a vector.
        let batch_vec: Vec<_> = batch.iter().collect();
        let batch_size: u32 = batch_vec.len() as u32;

        // Process the current batch (i.e., make API calls)
        log::info!("--- Processing batch of size {} ---", batch_vec.len());
        let start_tasks_time = Instant::now(); // Record the start time
        let mut handles: Vec<JoinHandle<Result<AllValidKlines4Pair>>> = Vec::new();
        for pair_interval in batch_vec {
            log::info!(
                "Processing: ({}, {},)",
                pair_interval.name(),
                // pair_interval.quote_asset,
                pair_interval.interval_ms
            );
            // Here you can make your API call for each item in the batch
            let handle = tokio::spawn(bn_kline::load_klines(pair_interval.clone(), batch_size));
            handles.push(handle);
        }
        let results: Vec<Result<Result<AllValidKlines4Pair>, JoinError>> = join_all(handles).await;
        let duration = start_tasks_time.elapsed(); // Calculate the elapsed time
        log::info!("\n...Time to complete all async tasks: {:?}", duration);
        log::info!(
            "Number of results (successful + failed) returned is {}",
            results.len(),
        );

        let mut errors = Vec::new();

        for result in results {
            let pair_kline = match result {
                Ok(inner_result) => inner_result,
                Err(e) => {
                    errors.push(format!("Request failed: {:?}", e));
                    continue;
                }
            };

            let pair_kline = match pair_kline {
                Ok(data) => data,
                Err(e) => {
                    log::info!("Binance API error for pair: {:?}", e);
                    continue;
                }
            };

            log::info!(
                "{} Number of klines in Binance data is: {}",
                pair_kline.pair_interval,
                pair_kline.klines.len()
            );
            all_valid_klines_4_pairs.push(pair_kline);
        }

        // Return error if any critical failures occurred
        if !errors.is_empty() {
            return Err(anyhow!("Failed to fetch data: {}", errors.join(", ")));
        }
    }

    if all_valid_klines_4_pairs.is_empty() {
        bail!("Gotta bail because all_valid_klines_4_pairs is empty");
    }

    // Convert Vec<AllValidKlines4Pair> to Vec<OhlcvTimeSeriesTemp>
    let ohlcv_time_series: Vec<OhlcvTimeSeriesTemp> = all_valid_klines_4_pairs
        .into_par_iter()
        // Map each item to a Result<OhlcvTimeSeriesTemp, E>
        .map(OhlcvTimeSeriesTemp::try_from)
        .filter_map(|result| match result {
            Ok(ohlcv) => Some(ohlcv),
            // For Err results, log the error and then return None to filter it out
            Err(e) => {
                log::error!("Error converting item: {}", e);
                None
            }
        })
        .collect();

    if ohlcv_time_series.is_empty() {
        bail!("Gotta get out of app because ohlcv_time_series is empty.")
    }
    let ohlcv_time_series: Vec<OhlcvTimeSeries> = ohlcv_time_series
        .into_par_iter()
        .map(|s| s.into())
        .collect();

    Ok(ohlcv_time_series)
}
