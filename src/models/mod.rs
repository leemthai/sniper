mod adaptive;
mod cva;
mod ledger;
mod market_state;
mod ohlcv;
mod pair_analysis;
mod range_gap_finder;
mod scenario_simulator;
mod trade_opportunity;
mod trading_model;

pub use ohlcv::OhlcvTimeSeries;

pub(crate) use adaptive::AdaptiveParameters;
pub(crate) use cva::{
    CVACore, MIN_CANDLES_FOR_ANALYSIS, PRICE_RECALC_THRESHOLD_PCT, SEGMENT_MERGE_TOLERANCE_MS,
    ScoreType,
};
pub(crate) use ledger::{OpportunityLedger, restore_engine_ledger};
pub(crate) use market_state::MarketState;
pub(crate) use ohlcv::{LiveCandle, TimeSeriesSlice, find_matching_ohlcv};
pub(crate) use pair_analysis::pair_analysis_pure;
pub(crate) use range_gap_finder::{DisplaySegment, GapReason, RangeGapFinder};
pub(crate) use scenario_simulator::{DEFAULT_SIMILARITY, EmpiricalOutcomeStats, ScenarioSimulator};
pub(crate) use trade_opportunity::{
    DEFAULT_JOURNEY_SETTINGS, DEFAULT_ZONE_CONFIG, TradeDirection, TradeOpportunity, TradeVariant,
    VisualFluff,
};
pub(crate) use trading_model::{SuperZone, TradingModel};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use trade_opportunity::TradeOutcome;
