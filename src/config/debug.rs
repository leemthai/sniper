//! Debugging feature flags.

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
    pub log_engine: bool,

    /// Includes PH logging e.g active_ph_pct
    pub log_tuner: bool, 
    
    pub log_pathfinder: bool,
    pub log_zones: bool,

    /// Anything about handling self.selected_pair
    pub log_selected_pair: bool,

    /// Anything about self.selected_opportunity
    pub log_selected_opportunity: bool,

    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    pub log_wasm_demo: bool,

    // These two need moving out to somehwere else!!!!!!!!!
    // Limit how many pairs are loaded in Debug mode.
    pub max_pairs_load: usize,
    // Nuke button for the Ledger
    pub wipe_ledger_on_startup: bool, 

}

pub const DF: LogFlags = LogFlags {
    log_performance: false,
    log_price_stream_updates: false,
    log_simulation_events: false,
    log_ledger: true,
    log_results_repo: false,
    log_engine: false,
    log_tuner: false,
    log_pathfinder: false,
    log_zones: false,
    log_selected_pair: false,
    log_selected_opportunity: true,

    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    log_wasm_demo: false,

    // These two need moving out to somehwere else!!!!!!!!!
    // Default to a small number for quick UI testing. Change this to 1000 when you want to stress-test the model i.e all pairs.
    max_pairs_load: 12, // 40, // 25, // 60,
    wipe_ledger_on_startup: false,
};

// use crate::config::DF
// if df.log_ledger {}