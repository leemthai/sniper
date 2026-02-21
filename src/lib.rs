#![allow(clippy::const_is_empty)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

// Make core modules accessible
mod app;
mod config;
mod data;
mod domain;
mod engine;
mod models;
#[cfg(feature = "ph_audit")]
mod ph_audit;
mod shared;
mod ui;
mod utils;

pub use {
    config::{BASE_INTERVAL, DEMO, PERSISTENCE, Price, PriceLike, kline_cache_filename},
    data::{CacheFile, PriceStreamManager, TimeSeriesCollection},
    domain::PairInterval,
    models::OhlcvTimeSeries,
    utils::TimeUtils,
};

// Gate the SQLite/Native storage specifically
#[cfg(not(target_arch = "wasm32"))]
pub use data::{MarketDataStorage, SqliteStorage};

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Use API as primary source instead of the local cache
    #[arg(long, default_value_t = false)]
    pub prefer_api: bool,
}

use crate::app::App as AppInternal;
/// Main application entry point - creates the GUI app
pub fn run_app(
    cc: &eframe::CreationContext<'_>,
    args: Cli, // Was TimeSeriesCollection
) -> AppInternal {
    AppInternal::new(cc, args)
}
