
use std::time::Duration;

// Truly global constants
/// Base interval for price updates and processing (5 minutes)
pub const BASE_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// Number of price zones for analysis
pub(crate) const ZONE_COUNT: usize = 256;
/// Time decay factor for historical data weighting
pub(crate) const TIME_DECAY_FACTOR: f64 = 1.5;
/// Steps for tuner scanning process
pub(crate) const TUNER_SCAN_STEPS: usize = 4;

mod binance;
pub use binance::{BINANCE, BinanceApiConfig};

mod debug;
pub(crate) use debug::DF;

mod demo;
pub use demo::DEMO;

mod persistence;
pub use persistence::{PERSISTENCE, kline_cache_filename};

mod ticker;
pub(crate) use ticker::TICKER;

pub mod plot;


// Private module with crate-wide re-exports
mod tuner;
pub(crate) use tuner::{StationId, TunerStation, TUNER_CONFIG, TimeTunerConfig};

// Private module
mod types;
// Re-export to everyone
pub use types::{
    Price,
    PriceLike,
};
// Crate-wide re-export
pub(crate) use types::{
    JourneySettings, 
    OptimalSearchSettings, 
    TradeProfile, 
    OptimizationStrategy, 
    ZoneParams,
    SimilaritySettings,
    ZoneClassificationConfig,
    Pct,
    PhPct,
    VolatilityPct,
    MomentumPct,
    RoiPct,
    AroiPct,
    Prob,
    VolRatio,
    Sigma,
    Weight,
    DurationMs,
    BaseVol,
    QuoteVol,
    OpenPrice,
    HighPrice,
    LowPrice,
    ClosePrice,
    TargetPrice,
    StopPrice,
    PriceRange,
    CandleResolution,
};


