//! Configuration module for the klines application.

// Can all be private now because we have a public re-export. Forces using file to just use crate::config, rather than crate::config::debug or crate::config::binance
mod analysis;
mod binance;
mod debug;
mod demo;
mod persistence;

// Can't be private because we don't re-export it
pub mod plot;

// Re-export commonly used items
pub use analysis::{ANALYSIS, ZoneParams, AnalysisConfig};
pub use binance::{BINANCE, BinanceApiConfig};
pub use debug::DEBUG_FLAGS;
pub use demo::DEMO;
pub use persistence::{PERSISTENCE, kline_cache_filename};
