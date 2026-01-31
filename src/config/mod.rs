//! Configuration module for the klines application.

// Can all be private now because we have a public re-export. Forces using file to just use crate::config, rather than crate::config::debug or crate::config::binance
mod types; // Renamed from analysis.rs
mod binance;
pub mod constants;
mod debug;
mod demo;
mod persistence;
mod ticker;
pub mod tuner;

// Can't be private because we don't re-export it
pub mod plot;

// Re-export commonly used items
pub use types::{
    JourneySettings, 
    OptimalSearchSettings, 
    TradeProfile, 
    OptimizationStrategy, 
    ZoneParams,
    SimilaritySettings,
    ZoneClassificationConfig,
    PhPct,
    VolatilityPct,
    MomentumPct,
    RoiPct,
    AroiPct,
    Prob,
    VolRatio,
    Sigma,
    Weight,
    BaseVol,
    QuoteVol
};
pub use tuner::{StationId, TunerStation, TimeTunerConfig};
pub use binance::{BINANCE, BinanceApiConfig};
pub use debug::DF;
pub use demo::DEMO;
pub use persistence::{PERSISTENCE, kline_cache_filename};
pub use ticker::TICKER;
