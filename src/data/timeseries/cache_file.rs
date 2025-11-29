use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config::{PERSISTENCE, kline_cache_filename};
use crate::data::timeseries::TimeSeriesCollection;

/// Serialized cache wrapper used for both native and WASM demo builds.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CacheFile {
    pub version: f64,
    pub timestamp_ms: i64,
    pub interval_ms: i64,
    pub data: TimeSeriesCollection,
}

impl CacheFile {
    pub fn new(interval_ms: i64, data: TimeSeriesCollection, version: f64) -> Self {
        Self {
            version,
            timestamp_ms: Utc::now().timestamp_millis(),
            interval_ms,
            data,
        }
    }

    pub fn load_from_path(path: &Path) -> Result<Self> {
        let file = File::open(path).context(format!("Failed to open cache file: {:?}", path))?;
        let mut reader = BufReader::new(file);
        let cache = bincode::deserialize_from(&mut reader)
            .context(format!("Failed to deserialize cache: {:?}", path))?;
        Ok(cache)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .context(format!("Failed to create directory: {}", parent.display()))?;
        }
        let file =
            File::create(path).context(format!("Failed to create file: {}", path.display()))?;
        let mut writer = BufWriter::new(file);
        bincode::serialize_into(&mut writer, self)
            .context(format!("Failed to serialize cache to: {}", path.display()))
    }

    pub fn default_cache_path(interval_ms: i64) -> PathBuf {
        PathBuf::from(PERSISTENCE.kline.directory).join(kline_cache_filename(interval_ms))
    }
}
