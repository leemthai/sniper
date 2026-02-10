mod cva;
pub(crate) use cva::{PRICE_RECALC_THRESHOLD_PCT, MIN_CANDLES_FOR_ANALYSIS, SEGMENT_MERGE_TOLERANCE_MS, CVACore, ScoreType};

mod ohlcv;
pub use ohlcv::{OhlcvTimeSeries};
pub(crate) use ohlcv::{find_matching_ohlcv, TimeSeriesSlice ,LiveCandle};

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
pub struct ProgressEvent{
    pub index: usize,
    pub pair: String,
    pub status: SyncStatus,
}
