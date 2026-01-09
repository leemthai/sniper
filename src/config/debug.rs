//! Debugging feature flags.

pub struct DebugFlags {

    /// Emit UI interaction logs (e.g., pair switching, manual actions).
    pub print_ui_interactions: bool,

    /// Emit verbose logging for live price stream connections and ticks.
    pub print_price_stream_updates: bool,

    /// Emit simulation-mode state changes (enter/exit, price adjustments, etc.).
    pub print_simulation_events: bool,

    /// Activate trace_time macro
    pub enable_perf_logging: bool,

    // NEW: Limit how many pairs are loaded in Debug mode.
    pub max_pairs_load: usize,

    pub gap_report: bool,
}

pub const DEBUG_FLAGS: DebugFlags = DebugFlags {
    print_ui_interactions: false,
    print_price_stream_updates: false,
    print_simulation_events: false,

    enable_perf_logging: false, // Activates trace_time! macro in perf.rs. Turn this back on for cool scope-level timing

    // Default to a small number for quick UI testing.
    // Change this to 1000 when you want to stress-test the model i.e all pairs.
    max_pairs_load: 25, // 40, // 25, // 60,

    gap_report: false,
};
