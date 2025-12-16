use anyhow::{Context, Result};
use crate::config::DEMO;
use crate::data::timeseries::cache_file::CacheFile;
use crate::data::timeseries::TimeSeriesCollection;

// Embed the demo data binary
const DEMO_CACHE_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/",
    crate::kline_data_dir!(),
    "/",
    crate::demo_cache_file!()
));

pub struct WasmDemoData;

impl WasmDemoData {
    pub fn load() -> Result<TimeSeriesCollection> {
        #[cfg(debug_assertions)]
        log::info!("Loading embedded WASM demo cache...");

        let cache: CacheFile = bincode::deserialize(DEMO_CACHE_BYTES)
            .context("Failed to deserialize embedded demo cache")?;

        let mut data = cache.data;

        // Enforce the WASM pair limit
        if data.series_data.len() > DEMO.max_pairs {
            data.series_data.truncate(DEMO.max_pairs);
        }

        Ok(data)
    }
}