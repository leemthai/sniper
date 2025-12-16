//! config/demo.rs Demo / WASM specific configuration knobs.
//!
//! These keep the browser build lightweight and deterministic by
//! constraining how much data we bundle and by ensuring we never
//! attempt network operations in that environment.

/// Static assets and paths required for the Demo
pub struct DemoResources {
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
        pairs: &["BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT", "PAXGUSDT"],
    },
};
