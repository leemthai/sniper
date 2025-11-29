//! config/demo.rs Demo / WASM specific configuration knobs.
//!
//! These keep the browser build lightweight and deterministic by
//! constraining how much data we bundle and by ensuring we never
//! attempt network operations in that environment.

/// Static assets and paths required for the Demo
pub struct DemoResources {
    /// Directory (relative to workspace) to be bundled
    // pub bundle_dir: &'static str,
    /// Filename emitted by the demo cache helper
    // pub cache_filename: &'static str,
    /// Curated list of pairs that should appear in the demo
    pub pairs: &'static [&'static str],
}

/// The Master Demo Configuration
pub struct DemoConfig {
    /// Maximum number of pairs to load (limit)
    pub max_pairs: usize,
    /// Bundled resources
    pub resources: DemoResources,
}

pub const DEMO: DemoConfig = DemoConfig {
    max_pairs: 10,

    resources: DemoResources {
        // bundle_dir: "kline_data",
        // cache_filename: "demo_kline_30m_v4.bin",
        pairs: &["BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT", "PAXGUSDT"],
    },
};
