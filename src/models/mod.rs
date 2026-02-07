// Domain models for klines analysis
// These modules contain pure business logic independent of UI/visualization

pub mod cva;
pub mod timeseries;
pub mod trading_view;
pub mod ledger;

// Re-export key types for convenience
pub use cva::CVACore;
pub use timeseries::{MostRecentIntervals, OhlcvTimeSeries, TimeSeriesSlice, find_matching_ohlcv};
pub use trading_view::{SuperZone, Zone, ZoneType};

// src/models/mod.rs

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
