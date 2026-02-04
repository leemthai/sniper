use std::sync::{Arc, RwLock};
use crate::config::{OptimizationStrategy, StationId, PhPct, Price};
use crate::data::timeseries::TimeSeriesCollection;
use crate::models::cva::CVACore;
use crate::models::trading_view::TradingModel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobMode {
    /// Standard operation: Calculate CVA, then run Pathfinder (Scouts/Drills)
    FullAnalysis,
    
    /// Visualization only: Calculate CVA (Zones/Volume Profile) but SKIP Pathfinder.
    /// Used when clicking an existing trade to restore the chart context.
    ContextOnly,
    
}

/// A request to calculate a model for a specific pair
/// /// Invariant:
/// - Each JobRequest is immutable
/// - Exactly one JobRequest per pair may be in-flight
#[derive(Debug, Clone)]
pub struct JobRequest {
    pub pair_name: String,
    pub current_price: Option<Price>,
    pub timeseries: Arc<RwLock<TimeSeriesCollection>>,
    pub ph_pct: PhPct,
    pub strategy: OptimizationStrategy,
    pub station_id: StationId,
    pub mode: JobMode,
    
}

/// The result returned by the worker
#[derive(Debug, Clone)]
pub struct JobResult {
    pub pair_name: String,
    pub duration_ms: u128,
    pub result: Result<Arc<TradingModel>, String>,
    pub cva: Option<Arc<CVACore>>,
    pub candle_count: usize,
}