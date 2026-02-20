#![allow(clippy::const_is_empty)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

// Core modules
pub mod analysis;
pub mod app;
pub mod config;
pub mod data;
pub mod domain;
pub mod engine;
pub mod models;
#[cfg(feature = "ph_audit")]
pub mod ph_audit;
mod shared;
pub mod ui;
pub mod utils;

// Re-export commonly used types outside of crate (for make_demo_cache.rs)
pub use crate::models::OhlcvTimeSeries;
pub use app::App;
pub use data::{TimeSeriesCollection, fetch_pair_data, price_stream::PriceStreamManager};
pub use domain::PairInterval;

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
) -> App {
    App::new(cc, args)
}
