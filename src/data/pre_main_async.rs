// Async code to run in main before egui starts up

use crate::Cli;
#[cfg(target_arch = "wasm32")]
use crate::config::WASM_MAX_PAIRS;
#[cfg(not(target_arch = "wasm32"))]
use crate::config::{INTERVAL_WIDTH_TO_ANALYSE_MS, KLINE_VERSION};
#[cfg(not(target_arch = "wasm32"))]
use crate::data::timeseries::bnapi_version::BNAPIVersion;
#[cfg(not(target_arch = "wasm32"))]
use crate::data::timeseries::serde_version::{SerdeVersion, check_local_data_validity};
#[cfg(target_arch = "wasm32")]
use crate::data::timeseries::wasm_demo::WasmDemoData;
use crate::data::timeseries::{
    CreateTimeSeriesData, TimeSeriesCollection, get_timeseries_data_async,
};

// The async function to load  to run before the GUI starts at all (so can't rely on gui app state)
pub async fn fetch_pair_data(
    klines_acceptable_age_secs: i64,
    args: &Cli,
) -> (TimeSeriesCollection, &'static str) {
    #[cfg(debug_assertions)]
    log::info!("Fetching data asynchronously (whether from local disk or BN API)...");
    // Klines loading logic: If `check_local_data_validity` fails, then only choice is to read from API.
    // else if `check_local_data_validity` succeeds, both methods become available so we prioritize whatever the user wants (set to prioritize_local_disk_read via cli)

    #[cfg(target_arch = "wasm32")]
    {
        let _ = args;
        let _ = klines_acceptable_age_secs;
    }

    #[cfg(not(target_arch = "wasm32"))]
    let providers: Vec<Box<dyn CreateTimeSeriesData>> = {
        let api_first = args.prefer_api;
        match (
            api_first,
            check_local_data_validity(
                klines_acceptable_age_secs,
                KLINE_VERSION,
                INTERVAL_WIDTH_TO_ANALYSE_MS,
            ),
        ) {
            (false, Ok(_)) => vec![
                Box::new(SerdeVersion {
                    interval_ms: INTERVAL_WIDTH_TO_ANALYSE_MS,
                }),
                Box::new(BNAPIVersion),
            ], // local first
            (true, Ok(_)) => vec![
                Box::new(BNAPIVersion),
                Box::new(SerdeVersion {
                    interval_ms: INTERVAL_WIDTH_TO_ANALYSE_MS,
                }),
            ], // API first
            (_, Err(e)) => {
                log::error!("⚠️  Local cache validation failed: {:#}", e);
                log::error!("   Falling back to Binance API...");
                vec![Box::new(BNAPIVersion)] // API only
            }
        }
    };

    #[cfg(target_arch = "wasm32")]
    let providers: Vec<Box<dyn CreateTimeSeriesData>> = vec![Box::new(WasmDemoData)];

    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_mut))]
    let (mut timeseries_data, timeseries_signature) = get_timeseries_data_async(&providers)
        .await
        .expect("failed to retrieve time series data so exiting main function!");

    #[cfg(target_arch = "wasm32")]
    {
        let original_len = timeseries_data.series_data.len();
        if original_len > WASM_MAX_PAIRS {
            timeseries_data.series_data.truncate(WASM_MAX_PAIRS);
            #[cfg(debug_assertions)]
            log::info!(
                "WASM demo build limited to {} pairs (from {}).",
                WASM_MAX_PAIRS,
                original_len
            );
        }
    }

    #[cfg(debug_assertions)]
    log::info!(
        "Successfully retrieved time series data using: {}.",
        timeseries_signature
    );
    #[cfg(debug_assertions)]
    log::info!("Data fetch complete.");
    (timeseries_data, timeseries_signature)
}
