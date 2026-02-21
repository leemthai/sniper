use std::time::Duration;

/// Base interval for price updates and processing (5 minutes)
pub const BASE_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// Number of price zones for analysis
pub(crate) const ZONE_COUNT: usize = 256;
/// Time decay factor for historical data weighting
pub(crate) const TIME_DECAY_FACTOR: f64 = 1.5;
/// Steps for tuner scanning process
pub(crate) const TUNER_SCAN_STEPS: usize = 4;

mod binance;
mod debug;
mod demo;
mod persistence;
mod plot;
mod ticker;
mod tuner;
mod types;

pub use binance::{BINANCE, BinanceApiConfig};

pub(crate) use debug::DF;
pub(crate) use plot::PLOT_CONFIG;
pub(crate) use ticker::TICKER;
pub(crate) use tuner::{StationId, TUNER_CONFIG, TimeTunerConfig, TunerStation};
pub(crate) use types::{
    AroiPct, BaseVol, CandleResolution, ClosePrice, DurationMs, HighPrice, JourneySettings,
    LowPrice, MomentumPct, OpenPrice, OptimalSearchSettings, OptimizationStrategy, Pct, PhPct,
    PriceRange, Prob, QuoteVol, RoiPct, Sigma, SimilaritySettings, StopPrice, TargetPrice,
    TradeProfile, VolRatio, VolatilityPct, Weight, ZoneClassificationConfig, ZoneParams,
};

pub use demo::DEMO;
pub use persistence::{PERSISTENCE, kline_cache_filename};
pub use types::{Price, PriceLike};
