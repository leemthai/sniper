// Only compile the logic for NATIVE builds.
// For WASM, this file becomes effectively empty (just a dummy main).

#[cfg(not(target_arch = "wasm32"))]
use {
    anyhow::{anyhow, Context, Result},
    serde_json::Value,
    std::collections::HashMap,
    std::path::PathBuf,
    std::thread,
    std::time::{Duration, Instant},
    zone_sniper::config::{ANALYSIS, DEMO, PERSISTENCE},
    zone_sniper::data::price_stream::PriceStreamManager,
    zone_sniper::data::storage::{MarketDataStorage, SqliteStorage},
    zone_sniper::data::timeseries::cache_file::CacheFile,
    zone_sniper::data::timeseries::TimeSeriesCollection,
    zone_sniper::domain::pair_interval::PairInterval,
    zone_sniper::models::OhlcvTimeSeries,
    zone_sniper::utils::TimeUtils,
};

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
    let standard_name = zone_sniper::config::kline_cache_filename(interval_ms);
    let demo_filename = format!("demo_{}", standard_name);
    let output_path = PathBuf::from(PERSISTENCE.kline.directory).join(&demo_filename);

    log::info!("📦 Serializing to {:?}", output_path);
    
    let cache_file = CacheFile::new(interval_ms, collection, PERSISTENCE.kline.version);
    cache_file.save_to_path(&output_path)?;

    // 7. Snapshot Prices (RESTORED)
    log::info!("📸 Snapshotting Live Prices for WASM...");
    let prices = fetch_current_prices_for_demo_pairs(demo_pairs)?;
    write_demo_prices_json(&prices)?;

    log::info!("✅ Success!");
    
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_current_prices_for_demo_pairs(
    demo_pairs: &[&str],
) -> Result<HashMap<String, f64>> {
    let stream = PriceStreamManager::new();

    let symbols: Vec<String> = demo_pairs.iter().map(|s| s.to_string()).collect();
    if symbols.is_empty() {
        return Err(anyhow!("No WASM demo pairs configured"));
    }

    // This spawns a background thread/runtime to fetch prices
    stream.subscribe_all(symbols.clone());

    let timeout = Duration::from_secs(15);
    let poll_interval = Duration::from_millis(200);
    let start = Instant::now();

    loop {
        let mut prices: HashMap<String, f64> = HashMap::new();

        for symbol in &symbols {
            if let Some(price) = stream.get_price(symbol) {
                prices.insert(symbol.clone(), price);
            }
        }

        if prices.len() == symbols.len() {
            println!("✅ Collected live prices for {} demo pairs.", prices.len());
            return Ok(prices);
        }

        if start.elapsed() >= timeout {
            return Err(anyhow!(
                "Timed out after {:?} waiting for live prices (got {}/{}).",
                timeout,
                prices.len(),
                symbols.len()
            ));
        }

        thread::sleep(poll_interval);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn write_demo_prices_json(prices: &HashMap<String, f64>) -> Result<()> {
    let output_path = PathBuf::from(PERSISTENCE.kline.directory).join("demo_prices.json");

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create directory for demo prices: {}",
                parent.display()
            )
        })?;
    }

    let mut json_map: HashMap<String, Value> = HashMap::new();
    for (pair, price) in prices {
        json_map.insert(pair.to_uppercase(), Value::from(*price));
    }

    let json = serde_json::to_string_pretty(&json_map)
        .context("Failed to serialize demo prices to JSON")?;

    std::fs::write(&output_path, json).with_context(|| {
        format!(
            "Failed to write demo prices JSON to {}",
            output_path.display()
        )
    })?;

    println!(
        "✅ Demo prices written to {:?} ({} pairs).",
        output_path,
        prices.len()
    );

    Ok(())
}

// --- WASM STUB ---
#[cfg(target_arch = "wasm32")]
fn main() {}