// Only compile the logic for NATIVE builds.
// For WASM, this file becomes effectively empty (just a dummy main).

#[cfg(not(target_arch = "wasm32"))]
use anyhow::{Context, Result};
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use zone_sniper::config::{ANALYSIS, DEMO, PERSISTENCE};
#[cfg(not(target_arch = "wasm32"))]
use zone_sniper::data::storage::{MarketDataStorage, SqliteStorage};
#[cfg(not(target_arch = "wasm32"))]
use zone_sniper::data::timeseries::cache_file::CacheFile;
#[cfg(not(target_arch = "wasm32"))]
use zone_sniper::data::timeseries::TimeSeriesCollection;
#[cfg(not(target_arch = "wasm32"))]
use zone_sniper::domain::pair_interval::PairInterval;
#[cfg(not(target_arch = "wasm32"))]
use zone_sniper::models::OhlcvTimeSeries;
#[cfg(not(target_arch = "wasm32"))]
use zone_sniper::utils::TimeUtils;

// Limit demo data to keep WASM binary small (Github limit < 100MB)
#[cfg(not(target_arch = "wasm32"))]
const DEMO_CANDLE_LIMIT: usize = 15_000;

// --- NATIVE IMPLEMENTATION ---
#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<()> {
    // 1. Setup Logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // 2. Configuration from demo.rs
    let demo_pairs = DEMO.resources.pairs;
    
    let interval_ms = ANALYSIS.interval_width_ms;
    let interval_str = TimeUtils::interval_to_string(interval_ms);
    let db_path = "klines.sqlite";

    log::info!("🚀 Building WASM Demo Cache from local DB: {}", db_path);
    log::info!("Target Interval: {}", interval_str);
    log::info!("Selected Pairs (from demo.rs): {:?}", demo_pairs);

    // 3. Connect to DB
    let storage = SqliteStorage::new(db_path).await
        .context("Failed to connect to SQLite DB. Run the Native App first to populate data!")?;

    let mut series_list = Vec::new();

    // 4. Extract Data
    for &pair in demo_pairs {
        log::info!("Extracting {}...", pair);
        
        let mut candles = storage.load_candles(pair, interval_str, None).await?;
        
        if candles.is_empty() {
            log::warn!("⚠ No data found for {}. Skipping.", pair);
            continue;
        }

        // TRUNCATE DATA FOR DEMO SIZE LIMITS
        if candles.len() > DEMO_CANDLE_LIMIT {
            let start = candles.len() - DEMO_CANDLE_LIMIT;
            candles = candles.drain(start..).collect();
            log::info!("   ✂ Truncated to last {} candles for file size safety.", DEMO_CANDLE_LIMIT);
        }

        let pair_interval = PairInterval {
            name: pair.to_string(),
            interval_ms,
        };

        let ts = OhlcvTimeSeries::from_candles(pair_interval, candles);
        series_list.push(ts);
    }

    if series_list.is_empty() {
        log::error!("No data extracted! Aborting.");
        return Ok(());
    }

    // 5. Build Collection
    let collection = TimeSeriesCollection {
        name: "WASM Demo Collection".to_string(),
        version: 1.0,
        series_data: series_list,
    };

    // 6. Generate Filename
    // Use the public re-export from config
    let standard_name = zone_sniper::config::kline_cache_filename(interval_ms);
    let demo_filename = format!("demo_{}", standard_name);
    
    let output_path = PathBuf::from(PERSISTENCE.kline.directory).join(&demo_filename);

    log::info!("📦 Serializing to {:?}", output_path);
    
    let cache_file = CacheFile::new(interval_ms, collection, PERSISTENCE.kline.version);
    cache_file.save_to_path(&output_path)?;

    log::info!("✅ Success!");
    log::info!("IMPORTANT: Update src/config/persistence.rs macro if the filename changed:");
    log::info!("   macro_rules! demo_cache_file {{ () => {{ \"{}\" }}; }}", demo_filename);

    Ok(())
}

// --- WASM STUB ---
// This allows the file to 'compile' (into nothing) when building for WASM
#[cfg(target_arch = "wasm32")]
fn main() {}