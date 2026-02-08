//! Debugging feature flags.

#[allow(dead_code)]
pub struct LogFlags {
    /// Emit verbose logging for live price stream connections and ticks.
    pub log_price_stream_updates: bool,

    /// Emit simulation-mode state changes (enter/exit, price adjustments, etc.).
    pub log_simulation_events: bool,

    /// Activate trace_time macro (for cool scope-level timing)
    pub log_performance: bool,
    /// Log ledger activity
    pub log_ledger: bool,

    pub log_results_repo: bool,

    pub log_engine_core: bool,

    pub log_tuner: bool,
    pub log_station_overrides: bool,
    pub log_ph_overrides: bool,

    pub log_pathfinder: bool,
    pub log_zones: bool,

    /// Anything about handling self.selected_pair
    pub log_selected_pair: bool,

    pub log_candle_update: bool,

    /// Anything about self.selected_opportunity
    pub log_selected_opportunity: bool,

    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    pub log_wasm_demo: bool,

    /// Verify SIMD produces near-same results as scalar version
    pub log_simd: bool,

    pub log_strategy_selection: bool,

    pub log_startup_prices: bool,

    // These two need moving out to somehwere else!!!!!!!!!
    // Limit how many pairs are loaded in Debug mode.
    pub max_pairs_load: usize,

    // Nuke ledger automatically on start-up
    pub wipe_ledger_on_startup: bool,

    pub log_pairs: bool,
}

pub const DF: LogFlags = LogFlags {

    log_selected_opportunity: true,
    log_selected_pair: true,
    log_pairs: false,

    log_ledger: false,
    log_startup_prices: false,

    log_station_overrides: false,
    log_ph_overrides: false,
    log_tuner: false,

    log_engine_core: false, // exclusively core.rs stuff
    log_pathfinder: false,

    log_strategy_selection: false,
    log_candle_update: false,
    log_performance: false,
    log_price_stream_updates: false,
    log_simulation_events: false,
    log_results_repo: false,
    log_zones: false,
    log_simd: false,

    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    log_wasm_demo: false,

    // These two need moving out to somehwere else!!!!!!!!!
    // Default to a small number for quick UI testing. Change this to 1000 when you want to stress-test the model i.e all pairs.
    max_pairs_load: 20, // 40, // 25, // 60,
    wipe_ledger_on_startup: false,
};
