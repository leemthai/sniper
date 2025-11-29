//! Configuration module for the klines application.

pub mod analysis;
pub mod binance;

mod debug; // Can be private now because we have a public re-export. Forces files to use crate::config::DEBUG_FLAGS not crate::config::debug::DEBUG_FLAGS
pub use debug::DEBUG_FLAGS;

pub mod demo;
pub mod persistence;
pub mod plot;

// Re-export commonly used items
pub use analysis::{
    CVA_MIN_SECONDS_BETWEEN_RECALCS, CVA_PRICE_RECALC_THRESHOLD_PCT, DEFAULT_PRICE_ZONE_COUNT,
    INTERVAL_WIDTH_TO_ANALYSE_MS, JOURNEY_PRICE_UPDATE_THRESHOLD_PCT,
    JOURNEY_START_PRICE_TOLERANCE_PCT, JOURNEY_STOP_LOSS_PCT, MIN_CANDLES_FOR_ANALYSIS,
    TIME_HORIZON_DEFAULT_DAYS, TIME_HORIZON_MAX_DAYS, TIME_HORIZON_MIN_DAYS,
};
pub use binance::BINANCE;
pub use demo::{
    WASM_DEMO_CACHE_FILE, WASM_DEMO_PAIRS, WASM_DISABLE_NETWORKING, WASM_KLINE_BUNDLE_DIR,
    WASM_MAX_PAIRS,
};
pub use persistence::{
    APP_STATE_PATH, KLINE_FILENAME_WITHOUT_EXT, KLINE_PATH, KLINE_VERSION, kline_cache_filename,
};
