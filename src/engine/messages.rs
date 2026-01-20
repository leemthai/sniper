use std::sync::{Arc, RwLock};
use crate::config::AnalysisConfig;
use crate::data::timeseries::TimeSeriesCollection;
use crate::models::cva::CVACore;
use crate::models::horizon_profile::HorizonProfile;
use crate::models::trading_view::TradingModel;
// use serde::{Deserialize, Serialize};

// // --- NEW ENUM ---
// #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
// pub enum JobMode {
//     Standard, // Normal recalc using provided config
//     AutoTune, // Ignore provided PH, scan spectrum, return BEST PH
// }

/// A request to calculate a model for a specific pair
#[derive(Debug, Clone)]
pub struct JobRequest {
    pub pair_name: String,
    pub current_price: Option<f64>,
    pub config: AnalysisConfig,
    pub timeseries: Arc<RwLock<TimeSeriesCollection>>,
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
    // pub profile: Option<HorizonProfile>,
    
    pub candle_count: usize,
}