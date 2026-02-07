#![allow(clippy::const_is_empty)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
#![allow(clippy::type_complexity)]

// Core modules
pub mod analysis;
pub mod config;
pub mod data;
pub mod domain;
pub mod models;
pub mod ui;
pub mod utils;
pub mod engine;
pub mod ph_audit;
mod shared;

// Re-export commonly used types
pub use data::{price_stream::PriceStreamManager, TimeSeriesCollection, fetch_pair_data};
pub use domain::{Candle, PairInterval};
pub use models::{CVACore, TimeSeriesSlice, TradingModel, Zone};
pub use ui::ZoneSniperApp;

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
// Change signature:
pub fn run_app(
    cc: &eframe::CreationContext<'_>,
    args: Cli, // Was TimeSeriesCollection
) -> ZoneSniperApp {
    ZoneSniperApp::new(cc, args)
}