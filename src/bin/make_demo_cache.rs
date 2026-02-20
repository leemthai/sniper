// Only compile the logic for NATIVE builds.

#[cfg(not(target_arch = "wasm32"))]
use {
    anyhow::{Context, Result, anyhow},
    serde_json::Value,
    std::collections::HashMap,
    std::path::PathBuf,
    std::thread,
    std::time::{Duration, Instant},
    zone_sniper::config::{BASE_INTERVAL, DEMO, PERSISTENCE, Price, PriceLike},
    zone_sniper::data::{
        CacheFile, MarketDataStorage, PriceStreamManager, SqliteStorage, TimeSeriesCollection,
    },
    zone_sniper::domain::PairInterval,
    zone_sniper::models::OhlcvTimeSeries,
    zone_sniper::utils::interval_to_string,
};

// Limit demo data to keep WASM binary small (Github limit < 100MB)
#[cfg(not(target_arch = "wasm32"))]
const DEMO_CANDLE_LIMIT: usize = 50_000;

// --- NATIVE IMPLEMENTATION ---
#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<()> {
    // Setup Logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Configuration from demo.rs
    let demo_pairs = DEMO.resources.pairs;

    let interval_ms = BASE_INTERVAL.as_millis() as i64;
    let interval_str = interval_to_string(interval_ms);
    let db_path = "klines.sqlite";

    log::info!("🚀 Building WASM Demo Cache from local DB: {}", db_path);
    log::info!("Target Interval: {}", interval_str);
    log::info!("Selected Pairs (from demo.rs): {:?}", demo_pairs);

    let storage = SqliteStorage::new(db_path)
        .await
        .context("Failed to connect to SQLite DB. Run the Native App first to populate data!")?;

    let mut series_list = Vec::new();

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
            log::info!(
                "   ✂ Truncated to last {} candles for file size safety.",
                DEMO_CANDLE_LIMIT
            );
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

    let collection = TimeSeriesCollection {
        name: "WASM Demo Collection".to_string(),
        version: 1.0,
        series_data: series_list,
    };

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
fn fetch_current_prices_for_demo_pairs(demo_pairs: &[&str]) -> Result<HashMap<String, Price>> {
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
        let mut prices: HashMap<String, Price> = HashMap::new();

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
fn write_demo_prices_json(prices: &HashMap<String, Price>) -> Result<()> {
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
        json_map.insert(pair.to_uppercase(), Value::from(price.value()));
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
