//! Demo / WASM specific configuration knobs.
//!
//! These keep the browser build lightweight and deterministic by
//! constraining how much data we bundle and by ensuring we never
//! attempt network operations in that environment.

/// Maximum number of pairs that the WASM demo should load, even if
/// more symbols are available in `pairs.txt` or the bundled cache.
pub const WASM_MAX_PAIRS: usize = 10;

/// When `true` the WASM demo must rely exclusively on embedded /
/// cached data sources and skip any network requests.
/// And this must always be `true` in the WASM demo.
pub const WASM_DISABLE_NETWORKING: bool = true;

/// Directory (relative to the workspace root) that should be packaged
/// with the demo so it can deserialize cached klines on startup.
pub const WASM_KLINE_BUNDLE_DIR: &str = "kline_data";

/// Filename emitted by the demo cache helper (bundled with the wasm build).
pub const WASM_DEMO_CACHE_FILE: &str = "demo_kline_30m_v4.bin";

/// Curated list of pairs that should appear in the WASM demo cache.
/// Update this when regenerating `demo_pairs.bin`.
pub const WASM_DEMO_PAIRS: &[&str] = &["BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT", "PAXGUSDT"];
