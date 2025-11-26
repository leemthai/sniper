// Async code to run in main before egui starts up

use crate::Cli;
use crate::config::{INTERVAL_WIDTH_TO_ANALYSE_MS, KLINE_VERSION};
use crate::data::timeseries::{
    CreateTimeSeriesData, TimeSeriesCollection,
    bnapi_version::BNAPIVersion,
    get_timeseries_data_async,
    serde_version::{SerdeVersion, check_local_data_validity},
};

// The async function to load  to run before the GUI starts at all (so can't rely on gui app state)
pub async fn fetch_pair_data(
    klines_acceptable_age_secs: i64,
    args: &Cli,
) -> (TimeSeriesCollection, &'static str) {
    #[cfg(debug_assertions)]
    println!("Fetching data asynchronously (whether from local disk or BN API)...");
    // Klines loading logic: If `check_local_data_validity` fails, then only choice is to read from API.
    // else if `check_local_data_validity` succeeds, both methods become available so we prioritize whatever the user wants (set to prioritize_local_disk_read via cli)

    let api_first = args.prefer_api;
    let providers: Vec<Box<dyn CreateTimeSeriesData>> = match (
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
            eprintln!("⚠️  Local cache validation failed: {:#}", e);
            eprintln!("   Falling back to Binance API...");
            vec![Box::new(BNAPIVersion)] // API only
        }
    };

    let (timeseries_data, timeseries_signature) = get_timeseries_data_async(&providers)
        .await
        .expect("failed to retrieve time series data so exiting main function!");

    #[cfg(debug_assertions)]
    println!(
        "Successfully retrieved time series data using: {}.",
        timeseries_signature
    );
    #[cfg(debug_assertions)]
    println!("Data fetch complete.");
    (timeseries_data, timeseries_signature)
}
