// Private modules (implementation details)
mod cva;
mod ledger;
mod ohlcv;
mod trade_opportunity;
mod trading_model;

// Public re-exports (crate-wide API)
pub use ohlcv::OhlcvTimeSeries;

// Internal re-exports (crate only, not public API)
pub(crate) use cva::{
    CVACore, MIN_CANDLES_FOR_ANALYSIS, PRICE_RECALC_THRESHOLD_PCT, SEGMENT_MERGE_TOLERANCE_MS,
    ScoreType,
};

pub(crate) use ledger::{OpportunityLedger, restore_engine_ledger};

pub(crate) use ohlcv::{LiveCandle, TimeSeriesSlice, find_matching_ohlcv};

pub(crate) use trade_opportunity::{
    DEFAULT_JOURNEY_SETTINGS, DEFAULT_ZONE_CONFIG, TradeDirection, TradeOpportunity, TradeVariant,
    VisualFluff,
};

pub(crate) use trading_model::{SuperZone, TradingModel};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use trade_opportunity::TradeOutcome;

#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    Pending,
    Syncing,
    Completed(usize), // number of new candles
    Failed(String),   // Error message
}

#[derive(Debug, Clone)]
pub struct ProgressEvent {
    pub index: usize,
    pub pair: String,
    pub status: SyncStatus,
}
