use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HorizonBucket {
    pub threshold_pct: f64,    // e.g. 0.05 (5%)
    pub candle_count: usize,   // How many candles found? (Resolution)
    pub duration_days: f64,    // How much time covered? (Context)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HorizonProfile {
    pub buckets: Vec<HorizonBucket>,
    pub max_candle_count: usize, // For normalization in UI
}

impl HorizonProfile {
    pub fn new() -> Self {
        Self {
            buckets: Vec::new(),
            max_candle_count: 0,
        }
    }
}