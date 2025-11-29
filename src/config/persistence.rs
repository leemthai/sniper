//! File persistence and serialization configuration

//! config/persistence.rs File persistence and serialization configuration
// /// Directory path for storing kline data
// pub const KLINE_PATH: &str = "kline_data";

// /// Base filename for kline data files (without extension)
// pub const KLINE_FILENAME_WITHOUT_EXT: &str = "kline";

// /// Current version of the kline data serialization format
// /// Bumped to 4.0 for bincode format switch
// pub const KLINE_VERSION: f64 = 4.0;

// use crate::utils::TimeUtils;

// /// Generate interval-specific cache filename
// /// Example: "kline_v4.0_1h.bin" or "kline_v4.0_15m.bin"
// pub fn kline_cache_filename(interval_ms: i64) -> String {
//     let interval_str = TimeUtils::interval_ms_to_string(interval_ms);
//     format!(
//         "{}_{}_v{}.bin",
//         KLINE_FILENAME_WITHOUT_EXT, interval_str, KLINE_VERSION
//     )
// }

// // App state persistence
// /// Path for saving/loading application UI state
// pub const APP_STATE_PATH: &str = ".states.json";

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
        filename_base: "kline",
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
