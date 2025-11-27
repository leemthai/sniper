#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

// Adjust "zone_sniper" if your package name in Cargo.toml is different
#[allow(unused_imports)]
use zone_sniper::{
    Cli, // The struct from lib.rs
    INTERVAL_WIDTH_TO_ANALYSE_MS,
    KLINE_ACCEPTABLE_AGE_SECONDS,
    TimeSeriesCollection,
    fetch_pair_data, // The re-export from lib.rs
    run_app,         // The function from lib.rs
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
#[wasm_bindgen(start)]
pub async fn start() -> Result<(), wasm_bindgen::JsValue> {
    // A. Init Logging
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Debug);

    log::info!("🚀 Zone Sniper starting in WASM mode...");

    // B. Setup for Web
    let web_options = eframe::WebOptions::default();

    // C. Create Empty Data (WASM can't block to fetch it yet)
    // You will need to trigger a fetch inside your app after it loads
    let timeseries_data = TimeSeriesCollection::default();

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
#[cfg(not(target_arch = "wasm32"))]
// Define this locally if not in lib.rs, or import it if it is in config
const APP_STATE_PATH: &str = "app_state.json";
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    use clap::Parser;
    use eframe::NativeOptions;
    use std::path::PathBuf;
    use tokio::runtime::Runtime;
    use zone_sniper::data::write_timeseries_data_async;

    // A. Init Logging
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("Application panicked: {:?}", panic_info);
    }));
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    // B. Parse Args
    let args = Cli::parse();
    #[cfg(debug_assertions)]
    log::info!("Parsed arguments: {:?}", args);

    // C. Data Loading (Blocking)
    let rt = Runtime::new().expect("Failed to create Tokio runtime");
    let (timeseries_data, timeseries_signature) =
        rt.block_on(fetch_pair_data(KLINE_ACCEPTABLE_AGE_SECONDS, &args));

    // D. Background Cache Write
    let cache_data = timeseries_data.clone();
    rt.spawn(async move {
        if let Err(e) = write_timeseries_data_async(
            timeseries_signature,
            cache_data,
            INTERVAL_WIDTH_TO_ANALYSE_MS,
        )
        .await
        {
            log::error!("⚠️  Failed to write cache: {}", e);
        }
    });

    // E. Run Native App
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
