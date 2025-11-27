//! Zone Sniper - Trading zone analysis application
//!
//! Thin binary entry point - all logic lives in lib.rs

#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

// Only import wasm_bindgen when targeting WASM
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
use clap::Parser;
#[cfg(not(target_arch = "wasm32"))]
use eframe::NativeOptions;
#[cfg(not(target_arch = "wasm32"))]
use klines::{
    Cli,
    config::{APP_STATE_PATH, INTERVAL_WIDTH_TO_ANALYSE_MS, KLINE_ACCEPTABLE_AGE_SECONDS},
    data::{fetch_pair_data, timeseries::serde_version::write_timeseries_data_async},
    run_app,
};
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use tokio::runtime::Runtime;

// Only compile this function when targeting WASM
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn _keep_alive() {}

#[cfg(target_arch = "wasm32")]
fn main() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
    panic!(
        "The native binary is not available for wasm32 builds. Build the wasm target via web entrypoints instead."
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("Applicated panicked: {:?}", panic_info);
        // Add any critical cleanup here
        // Note: This runs even in release builds
    }));
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Warn)
        .init();

    let args = Cli::parse();
    #[cfg(debug_assertions)]
    println!("Parsed arguments: {:?}", args);

    // Set up a tokio Runtime object and load pair data with block on prior to starting up egui
    let rt = Runtime::new().expect("Failed to create Tokio runtime");
    let (timeseries_data, timeseries_signature) =
        rt.block_on(fetch_pair_data(KLINE_ACCEPTABLE_AGE_SECONDS, &args));

    // Write cache asynchronously (non-blocking)
    let cache_data = timeseries_data.clone();
    rt.spawn(async move {
        if let Err(e) = write_timeseries_data_async(
            timeseries_signature,
            cache_data,
            INTERVAL_WIDTH_TO_ANALYSE_MS,
        )
        .await
        {
            eprintln!("⚠️  Failed to write cache: {}", e);
        }
    });

    // Set up and run the native egui application.
    let options = NativeOptions {
        persistence_path: Some(PathBuf::from(APP_STATE_PATH)),
        ..Default::default()
    };
    eframe::run_native(
        "Zone Sniper - Scope. Lock. Snipe.",
        options,
        Box::new(move |cc| Ok(run_app(cc, timeseries_data))),
    )
}
