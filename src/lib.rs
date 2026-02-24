#![allow(clippy::const_is_empty)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
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

#[cfg(not(target_arch = "wasm32"))]
pub use data::{MarketDataStorage, SqliteStorage};

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(long, default_value_t = false)]
    pub prefer_api: bool,
}

use crate::app::App as AppInternal;
pub fn run_app(cc: &eframe::CreationContext<'_>, args: Cli) -> AppInternal {
    AppInternal::new(cc, args)
}
