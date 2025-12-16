#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

#[cfg(not(target_arch = "wasm32"))]
use clap::Parser;
#[cfg(not(target_arch = "wasm32"))]
use eframe::NativeOptions;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use zone_sniper::config::PERSISTENCE;

use zone_sniper::{
    Cli,      // re-export lib.rs
    run_app,  // The function from lib.rs
};

// --- 2. WASM SPECIFIC CODE ---
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// This keeps the WASM memory allocator from being stripped
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn _keep_alive() {}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
pub fn init_log() {
    let (global_level, my_code_level) = if cfg!(debug_assertions) {
        (log::LevelFilter::Warn, log::LevelFilter::Info)
    } else {
        (log::LevelFilter::Error, log::LevelFilter::Error)
    };

    let _ = fern::Dispatch::new()
        .level(global_level)
        .level_for(env!("CARGO_CRATE_NAME"), my_code_level)
        .chain(fern::Output::call(|record| {
            let msg = record.args().to_string();
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
    console_error_panic_hook::set_once();
    init_log();

    log::info!("🚀 Zone Sniper starting in WASM mode...");

    // 1. Get DOM elements
    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    let canvas = document
        .get_element_by_id("the_canvas_id")
        .expect("Failed to find canvas with id 'the_canvas_id'")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| "the_canvas_id was not a valid HtmlCanvasElement")?;

    // 2. Prepare Args (Default for Web)
    // We construct the Cli struct manually or use default if derived
    let args = Cli { prefer_api: false };

    // 3. Start App
    // We pass 'args' into run_app. The App itself handles loading the Demo Data
    // via its internal async loading state.
    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|cc| Ok(Box::new(run_app(cc, args)))),
        )
        .await
}

// --- 3. NATIVE SPECIFIC CODE ---

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    // A. Init Logging
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("Application panicked: {:?}", panic_info);
    }));

    let (global_level, my_code_level) = if cfg!(debug_assertions) {
        (log::LevelFilter::Warn, log::LevelFilter::Info)
    } else {
        (log::LevelFilter::Error, log::LevelFilter::Error)
    };
    
    env_logger::Builder::from_default_env()
        .filter_level(global_level)
        .filter(Some("zone_sniper"), my_code_level)
        .init();

    // B. Parse Args
    let args = Cli::parse();

    // C. Setup Options
    let options = NativeOptions {
        persistence_path: Some(PathBuf::from(PERSISTENCE.app.state_path)),
        viewport: eframe::egui::ViewportBuilder::default()
            .with_maximized(true)
            .with_title("Zone Sniper - Scope. Lock. Snipe."),
        ..Default::default()
    };

    // D. Run
    // Note: We no longer load data here. We pass 'args' to the App,
    // and the App spawns its own loading thread.
    eframe::run_native(
        "Zone Sniper",
        options,
        Box::new(move |cc| Ok(Box::new(run_app(cc, args)))),
    )
}