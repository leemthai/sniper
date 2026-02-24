#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // Windows release: hide console window 
use zone_sniper::{Cli, run_app};

#[cfg(not(target_arch = "wasm32"))]
use {
    clap::Parser,
    eframe::NativeOptions,
    std::{panic, path::PathBuf},
    zone_sniper::PERSISTENCE,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, prelude::*};

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

    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    let canvas = document
        .get_element_by_id("the_canvas_id")
        .expect("Failed to find canvas with id 'the_canvas_id'")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| "the_canvas_id was not a valid HtmlCanvasElement")?;

    let args = Cli { prefer_api: false };

    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|cc| Ok(Box::new(run_app(cc, args)))),
        )
        .await
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    panic::set_hook(Box::new(|info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        log::error!("CRITICAL PANIC:\n{}\nStack Trace:\n{}", info, backtrace);
    }));

    let (global_level, my_code_level) = if cfg!(debug_assertions) {
        (log::LevelFilter::Warn, log::LevelFilter::Info)
    } else {
        (log::LevelFilter::Error, log::LevelFilter::Error)
    };

    let mut builder = env_logger::Builder::new();

    builder
        .filter(None, global_level)
        .filter(Some("zone_sniper"), my_code_level)
        .init();

    let args = Cli::parse();
    let options = NativeOptions {
        persistence_path: Some(PathBuf::from(PERSISTENCE.app.state_path)),
        viewport: eframe::egui::ViewportBuilder::default()
            .with_maximized(true)
            .with_title("Zone Sniper - Scope. Lock. Snipe."),
        ..Default::default()
    };

    eframe::run_native(
        "Zone Sniper",
        options,
        Box::new(move |cc| Ok(Box::new(run_app(cc, args)))),
    )
}
