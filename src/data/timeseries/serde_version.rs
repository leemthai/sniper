use crate::config::{KLINE_PATH, kline_cache_filename};
use crate::utils::time_utils::how_many_seconds_ago;
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use crate::data::timeseries::{CreateTimeSeriesData, TimeSeriesCollection};

/// Binary cache file wrapper with metadata
#[derive(Serialize, Deserialize, Debug)]
struct CacheFile {
    pub version: f64,
    pub timestamp_ms: i64,
    pub interval_ms: i64,
    pub data: TimeSeriesCollection,
}

pub fn check_local_data_validity(
    recency_required_secs: i64,
    version_required: f64,
    interval_ms: i64,
) -> Result<()> {
    let filename = kline_cache_filename(interval_ms);
    let full_path = PathBuf::from(KLINE_PATH).join(&filename);

    #[cfg(debug_assertions)]
    {
        println!("Checking validity of local cache at {:?}...", full_path);
        println!("Fetching data from local disk...");
    }
    // Open and read only the metadata (bincode reads sequentially)
    let file = File::open(&full_path).context(format!("Failed to open file: {:?}", full_path))?;
    let mut reader = BufReader::new(file);

    // Deserialize full cache file (bincode is fast even for large files)
    let cache: CacheFile = bincode::deserialize_from(&mut reader)
        .context(format!("Failed to deserialize cache from: {:?}", full_path))?;

    // Check version
    if cache.version != version_required {
        bail!(
            "Cache version mismatch: file v{} vs required v{}",
            cache.version,
            version_required
        );
    }

    // Check interval matches
    if cache.interval_ms != interval_ms {
        bail!(
            "Cache interval mismatch: file has {}ms intervals, expected {}ms",
            cache.interval_ms,
            interval_ms
        );
    }

    // Check recency
    let seconds_ago = how_many_seconds_ago(cache.timestamp_ms);
    if seconds_ago > recency_required_secs {
        bail!(
            "Cache too old: created {} seconds ago (limit: {} seconds)",
            seconds_ago,
            recency_required_secs
        );
    }

    #[cfg(debug_assertions)]
    println!(
        "✅ Cache valid: v{}, {}s old (limit {}s), interval {}ms",
        cache.version, seconds_ago, recency_required_secs, cache.interval_ms
    );

    Ok(())
}

// Helper function to create a new file and any missing parent directories.
fn create_file_with_parents(path: &Path) -> Result<fs::File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }
    fs::File::create(path).with_context(|| format!("Failed to create file: {}", path.display()))
}

/// Write timeseries data to binary cache file
/// Uses bincode for ~10-20x faster serialization vs JSON
pub fn write_timeseries_data_locally(
    timeseries_signature: &'static str,
    timeseries_collection: &TimeSeriesCollection,
    interval_ms: i64,
) -> Result<()> {
    if timeseries_signature != "Binance API" {
        #[cfg(debug_assertions)]
        println!("Skipping cache write (data not from Binance API)");
        return Ok(());
    }

    let filename = kline_cache_filename(interval_ms);
    let dir_path = PathBuf::from(KLINE_PATH);
    let full_path = dir_path.join(&filename);

    println!("Writing cache to disk: {:?}...", full_path);
    let start_time = std::time::Instant::now();

    let file = create_file_with_parents(&full_path)?;
    let writer = BufWriter::new(file);

    let cache = CacheFile {
        version: crate::config::KLINE_VERSION,
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
        interval_ms,
        data: timeseries_collection.clone(),
    };

    bincode::serialize_into(writer, &cache)
        .with_context(|| format!("Failed to serialize cache to: {}", full_path.display()))?;

    let elapsed = start_time.elapsed();
    let file_size = std::fs::metadata(&full_path)?.len();
    println!(
        "✅ Cache written: {} ({:.1} MB in {:.2}s = {:.1} MB/s)",
        filename,
        file_size as f64 / 1_048_576.0,
        elapsed.as_secs_f64(),
        (file_size as f64 / 1_048_576.0) / elapsed.as_secs_f64()
    );

    Ok(())
}

/// Async wrapper for write_timeseries_data_locally
/// Spawns blocking task to avoid freezing UI
pub async fn write_timeseries_data_async(
    timeseries_signature: &'static str,
    timeseries_collection: TimeSeriesCollection,
    interval_ms: i64,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        write_timeseries_data_locally(timeseries_signature, &timeseries_collection, interval_ms)
    })
    .await
    .context("Cache write task panicked")?
}

pub struct SerdeVersion {
    pub interval_ms: i64,
}

#[async_trait]
impl CreateTimeSeriesData for SerdeVersion {
    fn signature(&self) -> &'static str {
        "Local Cache"
    }

    async fn create_timeseries_data(&self) -> Result<TimeSeriesCollection> {
        let filename = kline_cache_filename(self.interval_ms);
        let full_path = PathBuf::from(KLINE_PATH).join(&filename);

        #[cfg(debug_assertions)]
        let start_time = std::time::Instant::now();

        // Read file content
        let bytes = tokio::fs::read(&full_path)
            .await
            .with_context(|| format!("Failed to read cache: {:?}", full_path))?;

        #[cfg(debug_assertions)]
        println!("Reading cache from: {:?}...", full_path);

        // Deserialize in blocking task (bincode is CPU-bound)
        let cache: CacheFile = tokio::task::spawn_blocking(move || bincode::deserialize(&bytes))
            .await
            .context("Deserialization task panicked")?
            .context(format!("Failed to deserialize cache from: {:?}", full_path))?;

        #[cfg(debug_assertions)]
        {
            let elapsed = start_time.elapsed();
            println!(
                "✅ Cache loaded: {} pairs in {:.2}s",
                cache.data.series_data.len(),
                elapsed.as_secs_f64()
            );
        }

        Ok(cache.data)
    }
}
