// 1. Native Only: The Binance Kline Logic
#[cfg(not(target_arch = "wasm32"))]
pub mod bn_kline; 

// 2. Shared: The binary cache format (used by make_demo_cache AND wasm_demo)
pub mod cache_file;

// 3. WASM Only: The static loader
#[cfg(target_arch = "wasm32")]
pub mod wasm_demo;

// --- SHARED DATA STRUCTURES ---
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::models::OhlcvTimeSeries;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct TimeSeriesCollection {
    pub name: String,
    pub version: f64,
    pub series_data: Vec<OhlcvTimeSeries>,
}

impl TimeSeriesCollection {
    pub fn unique_pair_names(&self) -> Vec<String> {
        self.series_data
            .iter()
            .map(|ts| ts.pair_interval.name().to_string())
            .collect::<BTreeSet<_>>() // Sorts and deduplicates
            .into_iter()
            .collect()
    }
}