use anyhow::{Context, Result, anyhow};
use klines::config::{
    INTERVAL_WIDTH_TO_ANALYSE_MS, KLINE_PATH, WASM_DEMO_PAIRS, WASM_MAX_PAIRS, kline_cache_filename,
};
use klines::data::price_stream::PriceStreamManager;
use klines::data::timeseries::TimeSeriesCollection;
use klines::data::timeseries::cache_file::CacheFile;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    build_demo_cache()
}

fn build_demo_cache() -> Result<()> {
    let source_filename = kline_cache_filename(INTERVAL_WIDTH_TO_ANALYSE_MS);
    let source_path = PathBuf::from(KLINE_PATH).join(&source_filename);
    let cache = CacheFile::load_from_path(&source_path)
        .with_context(|| format!("Failed to load source cache {:?}", source_path))?;

    println!(
        "Loaded {} pairs from {:?}",
        cache.data.series_data.len(),
        source_path
    );

    let demo_pairs: HashSet<String> = WASM_DEMO_PAIRS.iter().map(|p| p.to_uppercase()).collect();

    let filtered = filter_pairs(cache.data.clone(), &demo_pairs);
    let mut filtered_collection = filtered;

    if filtered_collection.series_data.len() > WASM_MAX_PAIRS {
        filtered_collection.series_data.truncate(WASM_MAX_PAIRS);
    }

    let output_cache = CacheFile::new(
        INTERVAL_WIDTH_TO_ANALYSE_MS,
        filtered_collection,
        cache.version,
    );

    let demo_filename = format!("demo_{}", source_filename);
    let output_path = PathBuf::from(KLINE_PATH).join(&demo_filename);
    output_cache.save_to_path(&output_path)?;

    println!(
        "✅ Demo cache written to {:?} with {} pairs.",
        output_path,
        output_cache.data.series_data.len()
    );

    // After building the demo cache, also snapshot a single current price per
    // WASM demo pair so the WASM build can run without live networking.
    let prices = fetch_current_prices_for_demo_pairs(&demo_pairs)?;
    write_demo_prices_json(&prices)?;

    Ok(())
}

fn filter_pairs(data: TimeSeriesCollection, whitelist: &HashSet<String>) -> TimeSeriesCollection {
    let mut filtered = data.clone();
    filtered
        .series_data
        .retain(|ts| whitelist.contains(&ts.pair_interval.name().to_uppercase()));
    filtered
}

fn fetch_current_prices_for_demo_pairs(demo_pairs: &HashSet<String>) -> Result<HashMap<String, f64>> {
    let stream = PriceStreamManager::new();

    let symbols: Vec<String> = demo_pairs.iter().cloned().collect();
    if symbols.is_empty() {
        return Err(anyhow!("No WASM demo pairs configured"));
    }

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

fn write_demo_prices_json(prices: &HashMap<String, f64>) -> Result<()> {
    let output_path = PathBuf::from(KLINE_PATH).join("demo_prices.json");

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!("Failed to create directory for demo prices: {}", parent.display())
        })?;
    }

    let mut json_map: HashMap<String, Value> = HashMap::new();
    for (pair, price) in prices {
        json_map.insert(pair.to_uppercase(), Value::from(*price));
    }

    let json = serde_json::to_string_pretty(&json_map)
        .context("Failed to serialize demo prices to JSON")?;

    std::fs::write(&output_path, json).with_context(|| {
        format!("Failed to write demo prices JSON to {}", output_path.display())
    })?;

    println!("✅ Demo prices written to {:?} ({} pairs).", output_path, prices.len());

    Ok(())
}
