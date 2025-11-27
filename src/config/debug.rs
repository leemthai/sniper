//! Debugging feature flags.
//!
//! Toggle individual diagnostics here; keep them `false` by default so release
//! builds remain quiet even when compiled with `--features debug_assertions`.

/// Emit zone transition summaries after computing zone efficacy metrics.
pub const PRINT_ZONE_TRANSITION_SUMMARY: bool = false;

/// Emit per-candidate time-decay calibration details during CVA recomputation.
pub const PRINT_DECAY_CALIBRATION: bool = false;

/// If non-empty, emit detailed zone-scoring debug output only for this pair.
/// Example: "PAXGUSDT". Use "" to disable.
pub const PRINT_ZONE_SCORING_FOR_PAIR: &str = "";

/// Emit high-level journey scheduling and completion summaries.
pub const PRINT_JOURNEY_SUMMARY: bool = false;

/// Emit UI interaction logs (e.g., pair switching, manual actions).
pub const PRINT_UI_INTERACTIONS: bool = true;

/// Emit verbose logging for live price stream connections and ticks.
pub const PRINT_PRICE_STREAM_UPDATES: bool = false;

/// Emit bar-plot cache hit/miss diagnostics while rendering the main chart.
pub const PRINT_PLOT_CACHE_STATS: bool = false;

/// Emit detailed CVA cache hit/miss diagnostics (cache miss reasons, timings, etc.).
pub const PRINT_CVA_CACHE_EVENTS: bool = false;

/// Emit progress logs when pairs are added to the monitor and summary counts change.
pub const PRINT_MONITOR_PROGRESS: bool = false;

/// Emit simulation-mode state changes (enter/exit, price adjustments, etc.).
pub const PRINT_SIMULATION_EVENTS: bool = false;

/// Show sticky-zone dwell summary (Runs / median / p90 / max in candles) in the
/// status bar. This is intended for developers only and is further gated by
/// `cfg(debug_assertions)` in the UI layer.
pub const PRINT_STICKY_DWELL_SUMMARY: bool = false;

/// When debugging journeys, emit a detailed, step-by-step walkthrough for a
/// single historical attempt. This is intended for developers only and is
/// further gated inside the journey engine by `cfg(debug_assertions)`.
///
/// - `PRINT_JOURNEY_FOR_PAIR` must match the pair under analysis.
/// - `PRINT_TRIGGER_UPDATES` should be enabled to see the logs.
/// - `DEBUG_JOURNEY_ATTEMPT_INDEX` selects which attempt (0-based) to trace.
///
/// Set `DEBUG_JOURNEY_ATTEMPT_INDEX` to -1 to disable.
pub const DEBUG_JOURNEY_ATTEMPT_INDEX: i32 = -1;

/// Emit journey/trigger status updates (e.g., marking journeys stale, queued follow-ups).
pub const PRINT_TRIGGER_UPDATES: bool = true;

/// If non-empty, emit detailed journey analysis output only for this pair.
/// Example: "PAXGUSDT". Use "" to disable.
pub const PRINT_JOURNEY_FOR_PAIR: &str = "";

/// Emit detailed serialization/deserialization logs.
pub const PRINT_SERDE: bool = false;

/// Emit details of UI state serialization/deserialization logs.
pub const PRINT_STATE_SERDE: bool = false;

/// Emit shutdown app messages.
pub const PRINT_SHUTDOWN: bool = false;
