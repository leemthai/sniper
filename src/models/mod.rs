// Domain models for klines analysis
// These modules contain pure business logic independent of UI/visualization

pub mod timeseries;
pub mod cva;
pub mod trading_view;
pub mod pair_context;

// Re-export key types for convenience
pub use timeseries::{TimeSeriesSlice, OhlcvTimeSeries, MostRecentIntervals, find_matching_ohlcv};
pub use cva::CVACore;
pub use trading_view::{TradingModel, Zone, SuperZone, ZoneType};
pub use pair_context::{PairContext, TradingSignal};
