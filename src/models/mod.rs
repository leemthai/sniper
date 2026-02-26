mod adaptive;
mod cva;
mod ledger;
mod market_state;
mod ohlcv;
mod optimization_strategy;
mod pair_analysis;
mod range_gap_finder;
mod scenario_simulator;
mod trade_opportunity;
mod trading_model;

pub use ohlcv::OhlcvTimeSeries;

pub(crate) use {
    adaptive::AdaptiveParameters,
    cva::{
        CVACore, MIN_CANDLES_FOR_ANALYSIS, PRICE_RECALC_THRESHOLD_PCT, SEGMENT_MERGE_TOLERANCE_MS,
        ScoreType,
    },
    ledger::{OpportunityLedger, restore_engine_ledger},
    market_state::MarketState,
    ohlcv::{LiveCandle, TimeSeriesSlice, find_matching_ohlcv},
    optimization_strategy::OptimizationStrategy,
    pair_analysis::pair_analysis_pure,
    range_gap_finder::{DisplaySegment, GapReason, RangeGapFinder},
    scenario_simulator::{DEFAULT_SIMILARITY, EmpiricalOutcomeStats, ScenarioSimulator},
    trade_opportunity::{
        DEFAULT_JOURNEY_SETTINGS, DEFAULT_ZONE_CONFIG, TradeDirection, TradeOpportunity,
        TradeVariant, VisualFluff,
    },
    trading_model::{SuperZone, TradingModel},
};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use trade_opportunity::TradeOutcome;
