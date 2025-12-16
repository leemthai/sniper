//! File persistence and serialization configuration
use crate::utils::TimeUtils;

/// Configuration for Kline Data Persistence
pub struct KlinePersistenceConfig {
    /// Directory path for storing kline data
    pub directory: &'static str,
    /// Base filename for kline data files (without extension)
    pub filename_base: &'static str,
    /// Current version of the kline data serialization format
    pub version: f64,
}

/// Configuration for Application State Persistence
pub struct AppPersistenceConfig {
    /// Path for saving/loading application UI state
    pub state_path: &'static str,
}

/// The Master Persistence Configuration
pub struct PersistenceConfig {
    pub kline: KlinePersistenceConfig,
    pub app: AppPersistenceConfig,
}

pub const PERSISTENCE: PersistenceConfig = PersistenceConfig {
    kline: KlinePersistenceConfig {
        directory: "kline_data",
        filename_base: "kd",
        version: 4.0,
    },
    app: AppPersistenceConfig {
        state_path: ".states.json",
    },
};

/// Generate interval-specific cache filename
/// Example: "kline_v4.0_1h.bin"
pub fn kline_cache_filename(interval_ms: i64) -> String {
    // Note: Assuming you renamed this to 'interval_to_string' in TimeUtils earlier.
    // If not, stick to 'interval_ms_to_string'.
    let interval_str = TimeUtils::interval_to_string(interval_ms);

    format!(
        "{}_{}_v{}.bin",
        PERSISTENCE.kline.filename_base, interval_str, PERSISTENCE.kline.version
    )
}

// --- MACROS FOR COMPILE-TIME INCLUDES ---
// These allow include_bytes! to read "variables" by expanding them as literals.

#[macro_export]
macro_rules! kline_data_dir {
    () => { "kline_data" };
}

#[macro_export]
macro_rules! demo_prices_file {
    () => { "demo_prices.json" };
}

#[macro_export]
macro_rules! demo_cache_file {
    // You must update this string manually if you change the interval constant
    () => { "demo_kd_5m_v4.bin" };
}