use std::sync::Arc;
use crate::config::AnalysisConfig;
use crate::data::timeseries::TimeSeriesCollection;
use crate::models::cva::CVACore;
use crate::models::trading_view::TradingModel;

/// A request to calculate a model for a specific pair
#[derive(Debug, Clone)]
pub struct JobRequest {
    pub pair_name: String,
    pub current_price: f64,
    pub config: AnalysisConfig,
    // We pass a reference to the immutable timeseries data
    pub timeseries: Arc<TimeSeriesCollection>,
}

/// The result returned by the worker
#[derive(Debug, Clone)]
pub struct JobResult {
    pub pair_name: String,
    pub duration_ms: u128,
    
    // Success: The new Front Buffer
    // Failure: The error string
    pub result: Result<Arc<TradingModel>, String>,
    
    // We pass back the CVACore too if needed for debugging/plots, 
    // though TradingModel usually wraps it.
    pub cva: Option<Arc<CVACore>>,
}