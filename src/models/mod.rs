// Domain models for klines analysis
// These modules contain pure business logic independent of UI/visualization

// pub mod cva;
mod cva;
pub(crate) use cva::{PRICE_RECALC_THRESHOLD_PCT, MIN_CANDLES_FOR_ANALYSIS, SEGMENT_MERGE_TOLERANCE_MS, CVACore, ScoreType};

pub mod timeseries;
pub use timeseries::{MostRecentIntervals, OhlcvTimeSeries, TimeSeriesSlice, find_matching_ohlcv};

mod trading_view;
pub(crate) use trading_view::{
    DEFAULT_JOURNEY_SETTINGS,
    TradeDirection,
    TradeOutcome,
    SuperZone,
    TradeOpportunity,
    TradingModel,
    NavigationTarget,
    SortColumn,
    SortDirection,
    TradeFinderRow,
    TradeVariant,
    VisualFluff,
};

pub use trading_view::{Zone};


pub mod ledger;

#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    Pending,
    Syncing,
    Completed(usize), // usize = number of new candles
    Failed(String),   // Error message
}

#[derive(Debug, Clone)]
pub struct ProgressEvent {
    pub index: usize,
    pub pair: String,
    pub status: SyncStatus,
}
