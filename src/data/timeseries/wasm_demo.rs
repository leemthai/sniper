use anyhow::{Context, Result};
use async_trait::async_trait;

use crate::config::WASM_MAX_PAIRS;
use crate::data::timeseries::{CreateTimeSeriesData, TimeSeriesCollection, cache_file::CacheFile};

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

        // Log based on cache.data *before* we move it out
        #[cfg(debug_assertions)]
        {
            let series_count = cache.data.series_data.len();
            log::info!(
                "WASM demo cache deserialized: interval_ms={} version={} series_count={}",
                cache.interval_ms,
                cache.version,
                series_count
            );
            let names: Vec<String> = cache
                .data
                .series_data
                .iter()
                .map(|ts| ts.pair_interval.name().to_string())
                .collect();
            log::info!("WASM demo cache pairs: {:?}", names);
        }

        // Now move the data out
        let mut data = cache.data;
        if data.series_data.len() > WASM_MAX_PAIRS {
            #[cfg(debug_assertions)]
            let original_len = data.series_data.len();
            data.series_data.truncate(WASM_MAX_PAIRS);
            #[cfg(debug_assertions)]
            log::info!(
                "WASM demo build limited to {} pairs (from {}).",
                WASM_MAX_PAIRS,
                original_len
            );
        }
        Ok(data)
    }
}
