use crate::config::WASM_MAX_PAIRS;
use crate::data::timeseries::{CreateTimeSeriesData, TimeSeriesCollection, cache_file::CacheFile};
use anyhow::{Context, Result};
use async_trait::async_trait;

const DEMO_CACHE_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/kline_data/demo_kline_30m_v4.bin"
));

pub struct WasmDemoData;

#[async_trait]
impl CreateTimeSeriesData for WasmDemoData {
    fn signature(&self) -> &'static str {
        "WASM Demo Cache"
    }

    async fn create_timeseries_data(&self) -> Result<TimeSeriesCollection> {
        let cache: CacheFile = bincode::deserialize(DEMO_CACHE_BYTES)
            .context("Failed to deserialize embedded demo cache")?;

        let mut data = cache.data;
        if data.series_data.len() > WASM_MAX_PAIRS {
            data.series_data.truncate(WASM_MAX_PAIRS);
        }
        Ok(data)
    }
}
