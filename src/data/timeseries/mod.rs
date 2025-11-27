#[cfg(not(target_arch = "wasm32"))]
pub mod bnapi_version;
pub mod cache_file;
pub mod intervals;
#[cfg(not(target_arch = "wasm32"))]
pub mod serde_version;
#[cfg(target_arch = "wasm32")]
pub mod wasm_demo;
use crate::models::OhlcvTimeSeries;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[async_trait]
pub trait CreateTimeSeriesData {
    // Either create a time-series OR return an anyhow::error
    async fn create_timeseries_data(&self) -> Result<TimeSeriesCollection>;

    /// A unique identifier for this implementation (so that afterwards we know which one we used).
    fn signature(&self) -> &'static str;
}

pub async fn get_timeseries_data_async(
    implementations: &[Box<dyn CreateTimeSeriesData>],
) -> Result<(TimeSeriesCollection, &'static str)> {
    for imp in implementations {
        match imp.create_timeseries_data().await {
            Ok(data) => {
                let signature = imp.signature();
                return Ok((data, signature));
            }
            Err(e) => {
                log::info!("Error with an async implementation: {}", e);
                // Continue to the next implementation
            }
        }
    }
    Err(anyhow!("All async implementations failed to create data"))
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct TimeSeriesCollection {
    pub name: String, // Metadata e.g. "Binance TimeSeries Collection".
    pub version: f64, // Half-hearted attempt to add versioning to Serialization (probably unncessary)
    pub series_data: Vec<OhlcvTimeSeries>,
}

impl TimeSeriesCollection {
    pub fn unique_pair_names(&self) -> Vec<String> {
        // BTreeSet maintains sorted order and ensures uniqueness
        self.series_data
            .iter()
            .map(|ts| ts.pair_interval.name().to_string())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}
