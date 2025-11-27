//! Klines - Trading zone analysis library
//!
//! Organized into clear modules:
//! - `models`: Core domain models (CVA, timeseries, trading view)
//! - `analysis`: Zone analysis and scoring algorithms
//! - `ui`: GUI components and visualization
//! - `data`: Data loading, caching, streaming
//! - `domain`: Small domain types (candle, pair, duration)
//! - `config`: Application configuration
//! - `utils`: Utility functions

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

// Re-export commonly used types for convenience
pub use analysis::ZoneGenerator;
pub use data::{PriceStreamManager, TimeSeriesCollection, fetch_pair_data};
pub use domain::{Candle, PairInterval};
pub use models::{CVACore, TimeSeriesSlice, TradingModel, Zone};
pub use ui::ZoneSniperApp;
pub use utils::app_time;

// Re-export constants (matching main.rs)
pub use utils::time_utils::{MS_IN_15_MIN, MS_IN_H};

// Klines saving and loading
pub const KLINE_PATH: &str = "kline_data";
pub const KLINE_FILENAME_WITHOUT_EXT: &str = "kline";
pub const KLINE_VERSION: f64 = 3.0;

pub const MAX_PAIRS: usize = 20;

// Re-export config constants for tests and benchmarks
pub use config::{DEFAULT_PRICE_ZONE_COUNT, INTERVAL_WIDTH_TO_ANALYSE_MS};

pub const KLINE_ACCEPTABLE_AGE_SECONDS: i64 = 60 * 60 * 24;

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
