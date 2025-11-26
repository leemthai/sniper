use anyhow::{Context, Result};
use klines::config::{
    INTERVAL_WIDTH_TO_ANALYSE_MS, KLINE_PATH, WASM_DEMO_PAIRS, WASM_MAX_PAIRS, kline_cache_filename,
};
use klines::data::timeseries::TimeSeriesCollection;
use klines::data::timeseries::cache_file::CacheFile;
use std::collections::HashSet;
use std::path::PathBuf;

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
    Ok(())
}

fn filter_pairs(data: TimeSeriesCollection, whitelist: &HashSet<String>) -> TimeSeriesCollection {
    let mut filtered = data.clone();
    filtered
        .series_data
        .retain(|ts| whitelist.contains(&ts.pair_interval.name().to_uppercase()));
    filtered
}
