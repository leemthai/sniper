//! Debugging feature flags.

pub struct DebugFlags {

    /// Emit UI interaction logs (e.g., pair switching, manual actions).
    pub print_ui_interactions: bool,

    /// Emit verbose logging for live price stream connections and ticks.
    pub print_price_stream_updates: bool,

    /// Emit simulation-mode state changes (enter/exit, price adjustments, etc.).
    pub print_simulation_events: bool,

    pub debug_journey_attempt_index: i32,

    pub print_trigger_updates: bool,
    /// If non-empty, emit detailed journey analysis output only for this pair.
    /// Example: "PAXGUSDT". Use "" to disable.
    pub print_journey_for_pair: &'static str,

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

    debug_journey_attempt_index: -1, // -1 to disable, 0 to enable journey 0, 1 for 1 etc.
    print_trigger_updates: false,    // must be enabled to see journey logs
    print_journey_for_pair: "",      // pair to track journey of

    enable_perf_logging: false, // Turn this back on for cool scope-level timings..... set via trace_time! macro.

    // Default to a small number for quick UI testing.
    // Change this to 1000 when you want to stress-test the model i.e all pairs.
    max_pairs_load: 5, // 25, // 60,

    gap_report: false,
};
