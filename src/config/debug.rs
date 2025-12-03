//! Debugging feature flags.

pub struct DebugFlags {
    /// Emit zone transition summaries after computing zone efficacy metrics.
    pub print_zone_transition_summary: bool,

    /// Emit detailed zone-scoring debug output for all pairs.
    pub print_zone_scoring_for_all_pairs: &'static str,


    /// If non-empty, emit detailed zone-scoring debug output only for this pair.
    /// Example: "PAXGUSDT". Use "" to disable.
    pub print_zone_scoring_for_pair: &'static str,

    /// Emit high-level journey scheduling and completion summaries.
    pub print_journey_summary: bool,

    /// Emit UI interaction logs (e.g., pair switching, manual actions).
    pub print_ui_interactions: bool,

    /// Emit verbose logging for live price stream connections and ticks.
    pub print_price_stream_updates: bool,

    /// Emit bar-plot cache hit/miss diagnostics while rendering the main chart.
    pub print_plot_cache_stats: bool,

    /// Emit detailed CVA cache hit/miss diagnostics (cache miss reasons, timings, etc.).
    pub print_cva_cache_events: bool,

    /// Emit progress logs when pairs are added to the monitor and summary counts change.
    pub print_monitor_progress: bool,

    /// Emit simulation-mode state changes (enter/exit, price adjustments, etc.).
    pub print_simulation_events: bool,

    /// When debugging journeys, emit a detailed, step-by-step walkthrough for a
    /// single historical attempt. This is intended for developers only and is
    /// further gated inside the journey engine by `cfg(debug_assertions)`.
    ///
    /// - `print_journey_for_pair` must match the pair under analysis.
    /// - `print_trigger_updates` should be enabled to see the logs.
    /// - `debug_journey_attempt_index` selects which attempt (0-based) to trace.
    ///
    /// Set `debug_journey_attempt_index` to -1 to disable.
    pub debug_journey_attempt_index: i32,
    /// Emit journey/trigger status updates (e.g., marking journeys stale, queued follow-ups).
    pub print_trigger_updates: bool,
    /// If non-empty, emit detailed journey analysis output only for this pair.
    /// Example: "PAXGUSDT". Use "" to disable.
    pub print_journey_for_pair: &'static str,

    /// Emit detailed serialization/deserialization logs.
    pub print_serde: bool,

    /// Emit details of UI state serialization/deserialization logs.
    pub print_state_serde: bool,

    /// Emit shutdown app messages.
    pub print_shutdown: bool,

    /// Emit detailed journey status lines (UI flag not logging flag)
    pub display_journey_status_lines: bool,
}

pub const DEBUG_FLAGS: DebugFlags = DebugFlags {
    print_zone_transition_summary: false,
    print_zone_scoring_for_all_pairs: "",
    print_zone_scoring_for_pair: "",
    print_journey_summary: false,
    print_ui_interactions: false,
    print_price_stream_updates: false,
    print_plot_cache_stats: false,
    print_cva_cache_events: false,
    print_monitor_progress: false,
    print_simulation_events: false,

    debug_journey_attempt_index: -1, // -1 to disable, 0 to enable journey 0, 1 for 1 etc.
    print_trigger_updates: false,    // must be enabled to see journey logs
    print_journey_for_pair: "",      // pair to track journey of

    print_serde: false,
    print_state_serde: false,
    print_shutdown: false,
    display_journey_status_lines: false,
};
