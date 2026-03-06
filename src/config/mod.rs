use std::time::Duration;

/// Base interval for price updates and processing (5 minutes)
pub const BASE_INTERVAL: Duration = Duration::from_secs(5 * 60);

mod debug;
mod demo;
mod persistence;
mod tuner;
mod types;

pub(crate) use {
    debug::LOG_PERFORMANCE,
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

pub use {
    demo::DEMO,
    persistence::{PERSISTENCE, kline_cache_filename},
    types::{Price, PriceLike},
};
