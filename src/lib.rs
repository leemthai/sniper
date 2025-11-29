#![allow(clippy::const_is_empty)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
#![allow(clippy::type_complexity)]

// Core modules
pub mod analysis;
pub mod config;
pub mod data;
pub mod domain;
pub mod journeys;
pub mod models;
pub mod ui;
pub mod utils;

// Re-export commonly used types
pub use analysis::ZoneGenerator;
pub use data::{PriceStreamManager, TimeSeriesCollection, fetch_pair_data};
pub use domain::{Candle, PairInterval};
pub use models::{CVACore, TimeSeriesSlice, TradingModel, Zone};
pub use ui::ZoneSniperApp;
pub use utils::app_time;

// CLI argument parsing
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Use API as primary source instead of the local cache
    #[arg(long, default_value_t = false)]
    pub prefer_api: bool,
}

/// Main application entry point - creates the GUI app
/// This is the public API for the binary to call
pub fn run_app(
    cc: &eframe::CreationContext,
    timeseries_data: TimeSeriesCollection,
) -> Box<dyn eframe::App> {
    let app = ui::ZoneSniperApp::new(cc, timeseries_data);
    Box::new(app)
}
