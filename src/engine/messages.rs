use std::sync::{Arc, RwLock};
use crate::config::{OptimizationStrategy, StationId, PhPct};
use crate::data::timeseries::TimeSeriesCollection;
use crate::models::cva::CVACore;
use crate::models::horizon_profile::HorizonProfile;
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
#[derive(Debug, Clone)]
pub struct JobRequest {
    pub pair_name: String,
    pub current_price: Option<f64>,
    pub timeseries: Arc<RwLock<TimeSeriesCollection>>,
    pub existing_profile: Option<HorizonProfile>,
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