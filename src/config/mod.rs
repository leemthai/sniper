//! Configuration module for the klines application.

pub mod analysis;
pub mod binance;

mod debug; // Can be private now because we have a public re-export. Forces files to use crate::config::DEBUG_FLAGS not crate::config::debug::DEBUG_FLAGS
pub use debug::DEBUG_FLAGS;

pub mod demo;
pub mod persistence;
pub mod plot;

// Re-export commonly used items
pub use analysis::ANALYSIS;
pub use binance::BINANCE;
pub use persistence::PERSISTENCE;

pub use demo::DEMO;
pub use persistence::kline_cache_filename;
