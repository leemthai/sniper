use std::sync::Arc;
use crate::config::AnalysisConfig;
use crate::data::timeseries::TimeSeriesCollection;
use crate::models::cva::CVACore;
use crate::models::horizon_profile::HorizonProfile;
use crate::models::trading_view::TradingModel;

/// A request to calculate a model for a specific pair
#[derive(Debug, Clone)]
pub struct JobRequest {
    pub pair_name: String,
    
    // CHANGED: f64 -> Option<f64> 
    // This allows the worker to handle "No Live Price" scenarios gracefully.
    pub current_price: Option<f64>,
    
    pub config: AnalysisConfig,
    pub timeseries: Arc<TimeSeriesCollection>,

    // NEW: The Input Cache
    // We send the profile we currently have. The worker checks if it's still valid.
    pub existing_profile: Option<HorizonProfile>, 
}

/// The result returned by the worker
#[derive(Debug, Clone)]
pub struct JobResult {
    pub pair_name: String,
    pub duration_ms: u128,
    
    pub result: Result<Arc<TradingModel>, String>,
    
    pub cva: Option<Arc<CVACore>>,
    
    // This is the Output (New or Reused profile)
    pub profile: Option<HorizonProfile>,
    
    pub candle_count: usize,
}