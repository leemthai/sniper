pub const LOG_PERFORMANCE: bool = false;

#[cfg(debug_assertions)]
pub struct DebugVars {
    pub log_candle_update: bool,
    pub log_engine_core: bool,
    pub log_ledger: bool,
    pub log_pairs: bool,
    pub log_pathfinder: bool,
    pub log_ph_overrides: bool,
    pub log_price_stream_updates: bool,
    pub log_selection: bool,
    pub log_simd: bool,
    pub log_station_overrides: bool,
    pub log_strategy_selection: bool,
    pub log_tuner: bool,
    pub log_zones: bool,
    pub wipe_ledger_on_startup: bool,
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    pub log_wasm_demo: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pub log_startup_prices: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pub log_results_repo: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pub max_pairs_load: usize,
}

#[cfg(debug_assertions)]
pub const DF: DebugVars = DebugVars {
    log_candle_update: false,
    log_engine_core: false,
    log_ledger: false,
    log_pairs: false,
    log_pathfinder: false,
    log_ph_overrides: false,
    log_price_stream_updates: false,
    log_selection: false,
    log_simd: false,
    log_station_overrides: false,
    log_strategy_selection: false,
    log_tuner: false,
    log_zones: false,
    wipe_ledger_on_startup: false,
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    log_wasm_demo: false,
    #[cfg(not(target_arch = "wasm32"))]
    log_results_repo: true,
    #[cfg(not(target_arch = "wasm32"))]
    log_startup_prices: true,
    #[cfg(not(target_arch = "wasm32"))]
    max_pairs_load: 20,
};
