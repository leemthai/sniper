#[cfg(debug_assertions)]
use crate::config::debug::PRINT_SERDE;
use crate::config::{KLINE_PATH, kline_cache_filename};
use crate::utils::time_utils::how_many_seconds_ago;
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use std::path::PathBuf;

use crate::data::timeseries::{CreateTimeSeriesData, TimeSeriesCollection, cache_file::CacheFile};

pub fn check_local_data_validity(
    recency_required_secs: i64,
    version_required: f64,
    interval_ms: i64,
) -> Result<()> {
    let filename = kline_cache_filename(interval_ms);
    let full_path = PathBuf::from(KLINE_PATH).join(&filename);

    #[cfg(debug_assertions)]
    if PRINT_SERDE {
        log::info!("Checking validity of local cache at {:?}...", full_path);
        log::info!("Fetching data from local disk...");
    }
    let cache = CacheFile::load_from_path(&full_path)?;

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
    if PRINT_SERDE {
        log::info!(
            "✅ Cache valid: v{}, {}s old (limit {}s), interval {}ms",
            cache.version,
            seconds_ago,
            recency_required_secs,
            cache.interval_ms
        );
    }

    Ok(())
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
        if PRINT_SERDE {
            log::info!("Skipping cache write (data not from Binance API)");
        }
        return Ok(());
    }

    let filename = kline_cache_filename(interval_ms);
    let dir_path = PathBuf::from(KLINE_PATH);
    let full_path = dir_path.join(&filename);

    #[cfg(debug_assertions)]
    let start_time = PRINT_SERDE.then(|| {
        log::info!("Writing cache to disk: {:?}...", full_path);
        std::time::Instant::now()
    });

    let cache = CacheFile::new(
        interval_ms,
        timeseries_collection.clone(),
        crate::config::KLINE_VERSION,
    );
    cache.save_to_path(&full_path)?;

    #[cfg(debug_assertions)]
    if let Some(start) = start_time {
        let elapsed = start.elapsed();
        let file_size = std::fs::metadata(&full_path)?.len();
        log::info!(
            "✅ Cache written: {} ({:.1} MB in {:.2}s = {:.1} MB/s)",
            filename,
            file_size as f64 / 1_048_576.0,
            elapsed.as_secs_f64(),
            (file_size as f64 / 1_048_576.0) / elapsed.as_secs_f64()
        );
    }

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

        // 1. Declare the timer as an Option BEFORE the task
        // We use .then() which runs the closure only if PRINT_SERDE is true
        #[cfg(debug_assertions)]
        let start_time = PRINT_SERDE.then(|| {
            log::info!("Reading cache from: {:?}...", full_path);
            std::time::Instant::now()
        });

        // 2. Perform the task (Variable scope is unaffected here)
        // Read file content
        let cache = tokio::task::spawn_blocking(move || CacheFile::load_from_path(&full_path))
            .await
            .context("Deserialization task panicked")?
            .context("Failed to load cache file")?;

        // 3. Check if we have a start_time and log the result
        #[cfg(debug_assertions)]
        if let Some(start) = start_time {
            let elapsed = start.elapsed();
            log::info!(
                "✅ Cache loaded: {} pairs in {:.2}s",
                cache.data.series_data.len(),
                elapsed.as_secs_f64()
            );
        }

        Ok(cache.data)
    }
}
