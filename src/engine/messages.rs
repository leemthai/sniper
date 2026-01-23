use std::sync::{Arc, RwLock};
use crate::config::{AppConstants, OptimizationGoal, StationId};
use crate::data::timeseries::TimeSeriesCollection;
use crate::models::cva::CVACore;
use crate::models::horizon_profile::HorizonProfile;
use crate::models::trading_view::TradingModel;
// use serde::{Deserialize, Serialize};

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
    pub config: AppConstants,
    pub timeseries: Arc<RwLock<TimeSeriesCollection>>,
    pub existing_profile: Option<HorizonProfile>,
    pub ph_pct: f64,
    pub strategy: OptimizationGoal,
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
    
    // This is the Output (New or Reused profile)
    // pub profile: Option<HorizonProfile>,
    
    pub candle_count: usize,
}