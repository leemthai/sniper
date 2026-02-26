use std::time::Duration;

pub(crate) const BINANCE_QUOTE_ASSETS: &[&str] = &[
    "USDT", "USDC", "FDUSD", "BTC", "ETH", "BNB", "EUR", "TRY", "JPY", "BRL", "USD", "USD1", "COP",
    "BRL", "ARS", "MXN",
];
/// Base interval for price updates and processing (5 minutes)
pub const BASE_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// Number of price zones for analysis
pub(crate) const ZONE_COUNT: usize = 256;
/// Time decay factor for historical data weighting
pub(crate) const TIME_DECAY_FACTOR: f64 = 1.5;
/// Steps for tuner scanning process
pub(crate) const TUNER_SCAN_STEPS: usize = 4;

mod debug;
mod demo;
mod persistence;
mod plot;
mod ticker;
mod tuner;
mod types;

#[cfg(not(target_arch = "wasm32"))]
mod binance;

pub(crate) use {
    debug::LOG_PERFORMANCE,
    plot::PLOT_CONFIG,
    ticker::TICKER,
    tuner::{StationId, TUNER_CONFIG, TimeTunerConfig, TunerStation},
    types::{
        AroiPct, BaseVol, CandleResolution, ClosePrice, DurationMs, HighPrice, JourneySettings,
        LowPrice, MomentumPct, OpenPrice, OptimalSearchSettings, Pct, PhPct, PriceRange, Prob,
        QuoteVol, RoiPct, Sigma, SimilaritySettings, StopPrice, TargetPrice, TradeProfile,
        VolRatio, VolatilityPct, Weight, ZoneClassificationConfig, ZoneParams,
    },
};

#[cfg(debug_assertions)]
pub(crate) use debug::DF;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use binance::{BINANCE, BINANCE_MAX_PAIRS, BINANCE_PAIRS_FILENAME, BinanceApiConfig};

pub use {
    demo::DEMO,
    persistence::{PERSISTENCE, kline_cache_filename},
    types::{Price, PriceLike},
};
