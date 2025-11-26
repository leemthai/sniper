// Domain models for klines analysis
// These modules contain pure business logic independent of UI/visualization

pub mod cva;
pub mod pair_context;
pub mod timeseries;
pub mod trading_view;

// Re-export key types for convenience
pub use cva::CVACore;
pub use pair_context::{PairContext, TradingSignal};
pub use timeseries::{MostRecentIntervals, OhlcvTimeSeries, TimeSeriesSlice, find_matching_ohlcv};
pub use trading_view::{SuperZone, TradingModel, Zone, ZoneType};
