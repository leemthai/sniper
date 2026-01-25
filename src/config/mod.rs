//! Configuration module for the klines application.

// Can all be private now because we have a public re-export.
mod analysis;
mod binance;
mod debug;
mod demo;
mod persistence;
mod ticker;

// Public 
pub mod constants;

// Can't be private because we don't re-export it
pub mod plot;

// Re-export commonly used items
pub use analysis::{
    JourneySettings, 
    OptimalSearchSettings, 
    TradeProfile, 
    OptimizationGoal, 
    TimeTunerConfig, 
    StationId, 
    TunerStation,
    ZoneParams,
    SimilaritySettings,
    ZoneClassificationConfig
};
pub use binance::{BINANCE, BinanceApiConfig};
pub use debug::DEBUG_FLAGS;
pub use demo::DEMO;
pub use persistence::{PERSISTENCE, kline_cache_filename};
pub use ticker::TICKER;
