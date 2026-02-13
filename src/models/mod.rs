mod cva;
pub(crate) use cva::{
    CVACore, MIN_CANDLES_FOR_ANALYSIS, PRICE_RECALC_THRESHOLD_PCT, SEGMENT_MERGE_TOLERANCE_MS,
    ScoreType,
};

mod ohlcv;
pub use ohlcv::OhlcvTimeSeries;
pub(crate) use ohlcv::{LiveCandle, TimeSeriesSlice, find_matching_ohlcv};

mod trade_opportunity;
pub(crate) use trade_opportunity::{
    DEFAULT_JOURNEY_SETTINGS, DEFAULT_ZONE_CONFIG, TradeDirection, TradeOpportunity, TradeOutcome,
    TradeVariant, VisualFluff,
};

mod trading_model;
pub(crate) use trading_model::{SuperZone, TradingModel};

pub mod ledger;
pub(crate) use ledger::{restore_engine_ledger};

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
