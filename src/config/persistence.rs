//! File persistence and serialization configuration

/// Directory path for storing kline data
pub const KLINE_PATH: &str = "kline_data";

/// Base filename for kline data files (without extension)
pub const KLINE_FILENAME_WITHOUT_EXT: &str = "kline";

/// Current version of the kline data serialization format
/// Bumped to 4.0 for bincode format switch
pub const KLINE_VERSION: f64 = 4.0;

use crate::utils::TimeUtils;

/// Generate interval-specific cache filename
/// Example: "kline_v4.0_1h.bin" or "kline_v4.0_15m.bin"
pub fn kline_cache_filename(interval_ms: i64) -> String {
    let interval_str = TimeUtils::interval_ms_to_string(interval_ms);
    format!(
        "{}_{}_v{}.bin",
        KLINE_FILENAME_WITHOUT_EXT, interval_str, KLINE_VERSION
    )
}

// App state persistence
/// Path for saving/loading application UI state
pub const APP_STATE_PATH: &str = ".states.json";
