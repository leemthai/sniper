#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

// Imports come from zone_sniper not crate, because main generates a  `bin`, so sees library as external dependency, just like serde or tokio.
#[cfg(not(target_arch = "wasm32"))]
use clap::Parser;
#[cfg(not(target_arch = "wasm32"))]
use eframe::NativeOptions;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use tokio::runtime::Runtime;
#[cfg(not(target_arch = "wasm32"))]
use zone_sniper::config::ANALYSIS;
use zone_sniper::config::BINANCE;
#[cfg(not(target_arch = "wasm32"))]
use zone_sniper::{config::PERSISTENCE, data::write_timeseries_data_async};

#[allow(unused_imports)]
use zone_sniper::{
    Cli,                  // re-export lib.rs
    TimeSeriesCollection, // re-export from lib.rs
    fetch_pair_data,      // The re-export from lib.rs
    run_app,              // The function from lib.rs
};

// --- 2. WASM SPECIFIC CODE ---
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*; // <--- REQUIRED for .dyn_into()

// This keeps the WASM memory allocator from being stripped
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn _keep_alive() {}

// --- Fix B: Added a dummy main() for WASM ---
// Even though we use 'start', the compiler still wants a main() function
// because this file is compiled as a binary.
#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
pub fn init_log() {
    // 1. Determine levels:
    //    Debug build: I want Info, 3rd party needs to pipe down (Warn).
    //    Release build: Only Errors.
    let (global_level, my_code_level) = if cfg!(debug_assertions) {
        (log::LevelFilter::Warn, log::LevelFilter::Info)
    } else {
        (log::LevelFilter::Error, log::LevelFilter::Error)
    };

    // 2. Configure Fern
    let _ = fern::Dispatch::new()
        // a. Set the "Global" noise floor (e.g., silence binance-sdk INFO logs)
        .level(global_level)
        // b. Override for YOUR specific crate (replace 'my_crate_name')
        .level_for(env!("CARGO_CRATE_NAME"), my_code_level)
        // c. Output to Browser Console
        .chain(fern::Output::call(|record| {
            let msg = record.args().to_string();
            // Map Rust log levels to browser console methods
            match record.level() {
                log::Level::Error => web_sys::console::error_1(&msg.into()),
                log::Level::Warn => web_sys::console::warn_1(&msg.into()),
                log::Level::Info => web_sys::console::info_1(&msg.into()),
                log::Level::Debug | log::Level::Trace => web_sys::console::log_1(&msg.into()),
            }
        }))
        .apply();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub async fn start() -> Result<(), wasm_bindgen::JsValue> {
    // A. Init Logging
    console_error_panic_hook::set_once();
    init_log();

    log::info!("🚀 Zone Sniper starting in WASM mode...");

    // B. Setup for Web
    let web_options = eframe::WebOptions::default();

    // C. Load demo timeseries data for WASM using the bundled cache
    //    This calls into fetch_pair_data(), which under wasm uses WasmDemoData.
    let args = Cli { prefer_api: false };
    let (timeseries_data, timeseries_signature) =
        fetch_pair_data(BINANCE.limits.kline_acceptable_age_sec, &args).await;

    log::info!(
        "WASM startup loaded timeseries via provider: {} (series_count={})",
        timeseries_signature,
        timeseries_data.series_data.len()
    );

    // 1. Get the browser window and document
    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");

    // 2. Find the canvas element by ID
    let canvas = document
        .get_element_by_id("the_canvas_id")
        .expect("Failed to find canvas with id 'the_canvas_id'")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| "the_canvas_id was not a valid HtmlCanvasElement")?;

    // Start the App
    // 3. Pass the canvas OBJECT to start()
    eframe::WebRunner::new()
        .start(
            canvas,
            web_options,
            Box::new(|cc| Ok(run_app(cc, timeseries_data))),
        )
        .await
}

// --- 3. NATIVE SPECIFIC CODE ---
// Emit CLI messages.
pub const PRINT_CLI: bool = false;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    // A. Init Logging
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("Application panicked: {:?}", panic_info);
    }));
    // Determine base levels based on Debug vs Release
    let (global_level, my_code_level) = if cfg!(debug_assertions) {
        // Debug: Others = Warn, Me = Info
        (log::LevelFilter::Warn, log::LevelFilter::Info)
    } else {
        // Release: Everyone = Error
        (log::LevelFilter::Error, log::LevelFilter::Error)
    };
    env_logger::Builder::from_default_env()
        // Set the baseline for "all other crates" (binance, tokio, etc.)
        .filter_level(global_level)
        // Override the setting specifically for YOUR crate
        .filter(Some("zone_sniper"), my_code_level)
        .init();

    // B. Parse Args
    let args = Cli::parse();

    #[cfg(debug_assertions)]
    if PRINT_CLI {
        log::info!("Parsed arguments: {:?}", args);
    }
    // C. Data Loading (Blocking)
    let rt = Runtime::new().expect("Failed to create Tokio runtime");
    let (timeseries_data, timeseries_signature) = rt.block_on(fetch_pair_data(
        BINANCE.limits.kline_acceptable_age_sec,
        &args,
    ));

    // D. Background Cache Write
    let cache_data = timeseries_data.clone();
    rt.spawn(async move {
        if let Err(e) = write_timeseries_data_async(
            timeseries_signature,
            cache_data,
            ANALYSIS.interval_width_ms,
        )
        .await
        {
            log::error!("⚠️  Failed to write cache: {}", e);
        }
    });

    // E. Run Native App
    let options = NativeOptions {
        persistence_path: Some(PathBuf::from(PERSISTENCE.app.state_path)),
        ..Default::default()
    };

    eframe::run_native(
        "Zone Sniper - Scope. Lock. Snipe.",
        options,
        Box::new(move |cc| Ok(run_app(cc, timeseries_data))),
    )
}
