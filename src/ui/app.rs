use eframe::{Frame, egui};
use poll_promise::Promise;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::analysis::MultiPairMonitor;
use crate::analysis::pair_analysis::ZoneGenerator;
use crate::config::{DEFAULT_PRICE_ZONE_COUNT, TIME_HORIZON_DEFAULT_DAYS};
use crate::data::price_stream::PriceStreamManager;
use crate::data::timeseries::TimeSeriesCollection;
use crate::journeys::{JourneyExecution, Outcome, ZoneEfficacyStats};
use crate::models::{CVACore, PairContext, TradingModel};
use crate::ui::app_async::AsyncCalcResult;
use crate::ui::app_simulation::{SimDirection, SimStepSize};
use crate::ui::app_triggers::PairTriggerState;
use crate::ui::config::UI_TEXT;
use crate::ui::ui_plot_view::PlotView;
use crate::ui::utils::setup_custom_visuals;
use crate::utils::app_time::{AppInstant, now};

#[cfg(debug_assertions)]
use crate::config::debug::DEBUG_FLAGS;
#[cfg(debug_assertions)]
use crate::ui::config::UI_CONFIG;

/// Error types for application operations
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AppError {
    /// No data is available for the operation
    DataNotAvailable,
    /// The selected pair is invalid or not found
    InvalidPair(String),
    /// CVA calculation failed
    CalculationFailed(String),
    /// General error with a message
    General(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::DataNotAvailable => write!(f, "No data available"),
            AppError::InvalidPair(pair) => write!(f, "Invalid or missing pair: {}", pair),
            AppError::CalculationFailed(msg) => write!(f, "Calculation failed: {}", msg),
            AppError::General(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

#[derive(Default)]
pub struct DataState {
    pub timeseries_collection: TimeSeriesCollection,
    pub cva_results: Option<Arc<CVACore>>,
    pub generator: ZoneGenerator,
    pub last_error: Option<AppError>,
    pub zone_efficacy: Option<(String, ZoneEfficacyStats)>,
}

#[derive(Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(super) enum JourneySummaryStatus {
    Completed,
    NoData,
    Failed,
}

#[derive(Clone)]
#[allow(dead_code)]
pub(super) struct JourneySummary {
    pub pair: String,
    pub zones_analyzed: usize,
    pub total_attempts: usize,
    pub successes: usize,
    pub timeouts: usize,
    pub stopped_out: usize,
    pub elapsed: Duration,
    pub completed_at: AppInstant,
    pub status: JourneySummaryStatus,
    pub note: Option<String>,
    pub median_ttt_days: Option<f64>,
    pub p90_ttt_days: Option<f64>,
}

#[derive(Clone)]
pub(super) enum JourneySummaryUpdate {
    NoData(String),
    Failure(String),
    Success {
        executions: Vec<JourneyExecution>,
        elapsed: Duration,
    },
}

#[allow(clippy::too_many_arguments)]
impl JourneySummary {
    pub fn completed(
        pair: String,
        zones_analyzed: usize,
        total_attempts: usize,
        successes: usize,
        timeouts: usize,
        stopped_out: usize,
        elapsed: Duration,
        median_ttt_days: Option<f64>,
        p90_ttt_days: Option<f64>,
    ) -> Self {
        Self {
            pair,
            zones_analyzed,
            total_attempts,
            successes,
            timeouts,
            stopped_out,
            elapsed,
            completed_at: now(),
            status: JourneySummaryStatus::Completed,
            note: None,
            median_ttt_days,
            p90_ttt_days,
        }
    }

    pub fn no_data(pair: String, note: impl Into<String>) -> Self {
        Self {
            pair,
            zones_analyzed: 0,
            total_attempts: 0,
            successes: 0,
            timeouts: 0,
            stopped_out: 0,
            elapsed: Duration::default(),
            completed_at: now(),
            status: JourneySummaryStatus::NoData,
            note: Some(note.into()),
            median_ttt_days: None,
            p90_ttt_days: None,
        }
    }

    pub fn failed(pair: String, note: impl Into<String>) -> Self {
        Self {
            pair,
            zones_analyzed: 0,
            total_attempts: 0,
            successes: 0,
            timeouts: 0,
            stopped_out: 0,
            elapsed: Duration::default(),
            completed_at: now(),
            status: JourneySummaryStatus::Failed,
            note: Some(note.into()),
            median_ttt_days: None,
            p90_ttt_days: None,
        }
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub(super) struct JourneyAggregateSummary {
    pub pair_count: usize,
    pub completed_pairs: usize,
    pub zones_analyzed: usize,
    pub total_attempts: usize,
    pub successes: usize,
    pub timeouts: usize,
    pub stopped_out: usize,
    pub elapsed: Duration,
    pub last_run_ago: Option<Duration>,
    pub median_ttt_days: Option<f64>,
    pub p90_ttt_days: Option<f64>,
}

/// Parameters for CVA calculation
///
/// This struct represents all the parameters needed to generate CVA results.
/// It implements PartialEq to enable efficient change detection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataParams {
    pub selected_pair: Option<String>,
    pub zone_count: usize,
    pub time_decay_factor: f64,
    /// Vector of discontinuous slice ranges [(start_idx, end_idx), ...] - computed by auto duration
    pub slice_ranges: Vec<(usize, usize)>,
    pub price_range: (f64, f64),
}

// Manual PartialEq implementation to handle f64 comparison
impl PartialEq for DataParams {
    fn eq(&self, other: &Self) -> bool {
        self.selected_pair == other.selected_pair
            && self.zone_count == other.zone_count
            && self.time_decay_factor.to_bits() == other.time_decay_factor.to_bits()
            && self.slice_ranges == other.slice_ranges
    }
}

impl Eq for DataParams {}

// Manual Hash implementation to handle f64 hashing
impl std::hash::Hash for DataParams {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.selected_pair.hash(state);
        self.zone_count.hash(state);
        self.time_decay_factor.to_bits().hash(state);
        for range in &self.slice_ranges {
            range.hash(state);
        }
    }
}

impl DataParams {
    /// Creates DataParams from the current app state
    pub fn from_app(
        selected_pair: &Option<String>,
        zone_count: usize,
        time_decay_factor: f64,
        slice_ranges: Vec<(usize, usize)>,
        price_range: (f64, f64),
    ) -> Self {
        Self {
            selected_pair: selected_pair.clone(),
            zone_count,
            time_decay_factor,
            slice_ranges,
            price_range,
        }
    }

    /// Validates that the parameters are valid for calculation
    pub fn is_valid(&self) -> Result<(), AppError> {
        if self.selected_pair.is_none() {
            return Err(AppError::DataNotAvailable);
        }
        if self.zone_count == 0 {
            return Err(AppError::CalculationFailed(
                "Zone count must be greater than 0".to_string(),
            ));
        }
        Ok(())
    }

    /// Returns the selected pair name, or an error if none selected
    pub fn pair(&self) -> Result<&str, AppError> {
        self.selected_pair
            .as_deref()
            .ok_or(AppError::DataNotAvailable)
    }
}

impl DataState {
    pub fn new(timeseries_collection: TimeSeriesCollection, generator: ZoneGenerator) -> Self {
        Self {
            timeseries_collection,
            cva_results: None,
            generator,
            last_error: None,
            zone_efficacy: None,
        }
    }

    pub fn clear_zone_efficacy(&mut self) {
        self.zone_efficacy = None;
    }
}

#[derive(Deserialize, Serialize)]
pub struct ZoneSniperApp {
    // UI state
    #[serde(default = "default_selected_pair")]
    pub(super) selected_pair: Option<String>,
    #[serde(default = "default_zone_count")]
    pub(super) zone_count: usize,
    #[serde(default = "default_time_decay_factor")]
    pub(super) time_decay_factor: f64,
    #[serde(default = "default_time_horizon_days")]
    pub(super) time_horizon_days: u64,
    #[serde(default)]
    pub(super) auto_duration_config: crate::domain::auto_duration::AutoDurationConfig,

    // Data state - skip serialization since it contains runtime-only data
    #[serde(skip)]
    pub(super) computed_slice_indices: Option<Vec<(usize, usize)>>,
    #[serde(skip)]
    pub(super) last_price_range: Option<(f64, f64)>,
    #[serde(skip)]
    pub(super) data_state: DataState,
    #[serde(skip)]
    pub(super) plot_view: PlotView,

    // Track the last calculated params to detect real changes
    #[serde(skip)]
    pub(super) last_calculated_params: Option<DataParams>,

    // Per-pair snapshot of the last accepted DataParams so that we can
    // distinguish real changes from simple focus switches between pairs.
    #[serde(skip)]
    pub(super) last_calculated_params_by_pair: HashMap<String, DataParams>,
    // Per-pair snapshot of params that most recently failed so we can avoid
    // immediate retries without an input change.
    #[serde(skip)]
    pub(super) last_failed_params_by_pair: HashMap<String, DataParams>,
    // Per-pair cache of the most recent CVA outputs so switching focus can
    // reuse prior results without forcing an immediate recalculation.
    #[serde(skip)]
    pub(super) cva_results_by_pair: HashMap<String, Arc<CVACore>>,

    // Per-pair cache of the most recent journey executions so we preserve
    // granular model outputs (per key-zone journeys) independently of UI.
    #[serde(skip)]
    pub(super) journey_executions_by_pair: HashMap<String, Vec<JourneyExecution>>,

    // Help panel visibility (available in all builds for better UX)
    #[serde(skip)]
    pub(super) show_debug_help: bool,

    // Async calculation state
    #[serde(skip)]
    pub(super) calculation_promise: Option<Promise<AsyncCalcResult>>,

    // Live price indicator (not part of CVA calculations)
    #[serde(skip)]
    pub(super) current_pair_price: Option<f64>,

    // WebSocket price stream manager
    #[serde(skip)]
    pub(super) price_stream: Option<PriceStreamManager>,

    // Multi-pair monitoring for opportunity detection
    #[serde(skip)]
    pub(super) multi_pair_monitor: MultiPairMonitor,
    #[serde(skip)]
    pub(super) pair_triggers: HashMap<String, PairTriggerState>,

    // Journey analysis trigger state and queue
    #[serde(skip)]
    pub(super) journey_triggers: HashMap<String, crate::ui::app_triggers::JourneyTriggerState>,
    #[serde(skip)]
    pub(super) journey_queue: VecDeque<String>,
    #[serde(skip)]
    pub(super) journey_summaries: HashMap<String, JourneySummary>,
    #[serde(skip)]
    pub(super) journey_aggregate: Option<JourneyAggregateSummary>,

    // Track if we've initialized the monitor with all pairs
    #[serde(skip)]
    pub(super) monitor_initialized: bool,

    // Simulation mode state
    #[serde(skip)]
    pub(super) is_simulation_mode: bool,
    #[serde(skip)]
    pub(super) simulated_prices: std::collections::HashMap<String, f64>,
    #[serde(skip)]
    pub(super) sim_direction: SimDirection,
    #[serde(skip)]
    pub(super) sim_step_size: SimStepSize,
}

/// Default value for zone count - used by serde and initialization
fn default_zone_count() -> usize {
    DEFAULT_PRICE_ZONE_COUNT
}

/// Default value for selected pair - used by serde and initialization
fn default_selected_pair() -> Option<String> {
    Some("BTCUSDT".to_string())
}

fn default_time_decay_factor() -> f64 {
    1.0
}

fn default_time_horizon_days() -> u64 {
    TIME_HORIZON_DEFAULT_DAYS
}

impl ZoneSniperApp {
    pub(super) fn record_journey_summary_success(
        &mut self,
        pair: &str,
        executions: &[JourneyExecution],
        elapsed: Duration,
    ) {
        if executions.is_empty() {
            self.record_journey_summary_no_data(pair, UI_TEXT.journey_status_no_data);
            return;
        }

        let zones_analyzed = executions.len();
        let mut total_attempts = 0usize;
        let mut successes = 0usize;
        let mut timeouts = 0usize;
        let mut stopped_out = 0usize;
        let mut success_days: Vec<f64> = Vec::new();

        for execution in executions {
            let stats = &execution.analysis.stats;
            total_attempts += stats.total_attempts;
            successes += stats.success_count;

            for outcome in &execution.analysis.outcomes {
                match outcome.outcome {
                    Outcome::TimedOut { .. } => timeouts += 1,
                    Outcome::StoppedOut { .. } => stopped_out += 1,
                    Outcome::Success { .. } => {
                        if let Some(days) = outcome.days_to_target {
                            success_days.push(days as f64);
                        }
                    }
                }
            }
        }

        let (median_ttt_days, p90_ttt_days) = if success_days.is_empty() {
            (None, None)
        } else {
            success_days.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let len = success_days.len();
            let median_idx = len / 2;
            let p90_pos = ((len - 1) as f64 * 0.9).round() as usize;
            let median = success_days.get(median_idx).copied().unwrap_or(0.0);
            let p90 = success_days.get(p90_pos).copied().unwrap_or(median);
            (Some(median), Some(p90))
        };

        let summary = JourneySummary::completed(
            pair.to_string(),
            zones_analyzed,
            total_attempts,
            successes,
            timeouts,
            stopped_out,
            elapsed,
            median_ttt_days,
            p90_ttt_days,
        );
        self.journey_executions_by_pair
            .insert(pair.to_string(), executions.to_vec());
        self.journey_summaries.insert(pair.to_string(), summary);
        self.recompute_journey_aggregate();
    }

    pub(super) fn record_journey_summary_no_data(&mut self, pair: &str, note: impl Into<String>) {
        let summary = JourneySummary::no_data(pair.to_string(), note);
        self.journey_summaries.insert(pair.to_string(), summary);
        self.recompute_journey_aggregate();
    }

    pub(super) fn record_journey_summary_failure(&mut self, pair: &str, note: impl Into<String>) {
        let summary = JourneySummary::failed(pair.to_string(), note);
        self.journey_summaries.insert(pair.to_string(), summary);
        self.recompute_journey_aggregate();
    }

    pub(super) fn apply_journey_summary_updates(
        &mut self,
        updates: Vec<(String, JourneySummaryUpdate)>,
    ) {
        for (pair, update) in updates {
            match update {
                JourneySummaryUpdate::NoData(note) => {
                    self.record_journey_summary_no_data(&pair, note);
                }
                JourneySummaryUpdate::Failure(note) => {
                    self.record_journey_summary_failure(&pair, note);
                }
                JourneySummaryUpdate::Success {
                    executions,
                    elapsed,
                } => {
                    self.record_journey_summary_success(&pair, &executions, elapsed);
                }
            }
        }
    }

    fn recompute_journey_aggregate(&mut self) {
        if self.journey_summaries.is_empty() {
            self.journey_aggregate = None;
            return;
        }

        let pair_count = self.journey_summaries.len();
        let mut completed_pairs = 0usize;
        let mut zones_analyzed = 0usize;
        let mut total_attempts = 0usize;
        let mut successes = 0usize;
        let mut timeouts = 0usize;
        let mut stopped_out = 0usize;
        let mut elapsed = Duration::default();
        let mut latest_completion: Option<AppInstant> = None;
        let mut success_days: Vec<f64> = Vec::new();

        for summary in self.journey_summaries.values() {
            if summary.status == JourneySummaryStatus::Completed {
                completed_pairs += 1;
                zones_analyzed += summary.zones_analyzed;
                total_attempts += summary.total_attempts;
                successes += summary.successes;
                timeouts += summary.timeouts;
                stopped_out += summary.stopped_out;
                elapsed += summary.elapsed;
                if let Some(execs) = self.journey_executions_by_pair.get(&summary.pair) {
                    for execution in execs {
                        for outcome in &execution.analysis.outcomes {
                            if let Outcome::Success { .. } = outcome.outcome {
                                if let Some(days) = outcome.days_to_target {
                                    success_days.push(days as f64);
                                }
                            }
                        }
                    }
                }
                latest_completion = Some(match latest_completion {
                    Some(existing) if existing >= summary.completed_at => existing,
                    _ => summary.completed_at,
                });
            }
        }

        let last_run_ago = latest_completion.and_then(|inst| now().checked_duration_since(inst));

        let (median_ttt_days, p90_ttt_days) = if success_days.is_empty() {
            (None, None)
        } else {
            success_days.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let len = success_days.len();
            let median_idx = len / 2;
            let p90_pos = ((len - 1) as f64 * 0.9).round() as usize;
            let median = success_days.get(median_idx).copied().unwrap_or(0.0);
            let p90 = success_days.get(p90_pos).copied().unwrap_or(median);
            (Some(median), Some(p90))
        };

        self.journey_aggregate = Some(JourneyAggregateSummary {
            pair_count,
            completed_pairs,
            zones_analyzed,
            total_attempts,
            successes,
            timeouts,
            stopped_out,
            elapsed,
            last_run_ago,
            median_ttt_days,
            p90_ttt_days,
        });
    }

    #[cfg(debug_assertions)]
    pub(super) fn journey_status_line(&self) -> (egui::Color32, String) {
        if let Some(pair) = &self.selected_pair {
            if let Some(summary) = self.journey_summaries.get(pair) {
                return match summary.status {
                    JourneySummaryStatus::Completed => (
                        egui::Color32::from_rgb(130, 200, 140),
                        Self::format_current_journey_line(summary),
                    ),
                    JourneySummaryStatus::NoData => (
                        egui::Color32::from_rgb(200, 200, 160),
                        Self::format_journey_note_line(summary),
                    ),
                    JourneySummaryStatus::Failed => (
                        egui::Color32::from_rgb(220, 120, 120),
                        Self::format_journey_note_line(summary),
                    ),
                };
            }

            return (
                egui::Color32::from_rgb(160, 160, 160),
                format!(
                    "{} {}: {}",
                    UI_TEXT.journey_status_current_prefix, pair, UI_TEXT.journey_status_waiting
                ),
            );
        }

        (
            egui::Color32::from_rgb(160, 160, 160),
            format!(
                "{}: {}",
                UI_TEXT.journey_status_current_prefix, UI_TEXT.journey_status_waiting
            ),
        )
    }

    #[cfg(debug_assertions)]
    pub(super) fn journey_aggregate_line(&self) -> (egui::Color32, String) {
        if let Some(aggregate) = &self.journey_aggregate {
            if let Some(line) = Self::format_aggregate_journey_line(aggregate) {
                return (egui::Color32::from_rgb(150, 200, 255), line);
            }
        }

        (
            egui::Color32::from_rgb(160, 160, 160),
            format!(
                "{}: {}",
                UI_TEXT.journey_status_aggregate_prefix, UI_TEXT.journey_status_waiting
            ),
        )
    }

    #[cfg(debug_assertions)]
    fn format_current_journey_line(summary: &JourneySummary) -> String {
        let mut parts = vec![format!(
            "{} {}",
            summary.zones_analyzed, UI_TEXT.journey_status_zones_label
        )];

        if summary.total_attempts > 0 {
            let total = summary.total_attempts as f64;
            let success_pct = (summary.successes as f64 / total) * 100.0;
            let timeout_pct = (summary.timeouts as f64 / total) * 100.0;
            let stopped_pct = (summary.stopped_out as f64 / total) * 100.0;
            parts.push(format!(
                "{} {} ({} {:.1}% | {} {:.1}% | {} {:.1}%)",
                UI_TEXT.journey_status_attempts_label,
                summary.total_attempts,
                UI_TEXT.journey_status_success_label,
                success_pct,
                UI_TEXT.journey_status_timeout_label,
                timeout_pct,
                UI_TEXT.journey_status_stopped_label,
                stopped_pct,
            ));
            if let (Some(median), Some(p90)) = (summary.median_ttt_days, summary.p90_ttt_days) {
                parts.push(format!("TTT median {:.1}d | p90 {:.1}d", median, p90));
            }
        } else {
            parts.push(UI_TEXT.journey_status_no_data.to_string());
        }

        format!(
            "{} {}: {}",
            UI_TEXT.journey_status_current_prefix,
            summary.pair,
            parts.join(" • "),
        )
    }

    #[cfg(debug_assertions)]
    pub(super) fn current_journey_zone_lines(&self) -> Vec<(egui::Color32, String)> {
        let mut lines = Vec::new(); // yes

        let pair = match &self.selected_pair {
            Some(p) => p,
            None => return lines,
        };

        let executions = match self.journey_executions_by_pair.get(pair) {
            Some(execs) if !execs.is_empty() => execs,
            _ => return lines,
        };

        for execution in executions {
            let stats = &execution.analysis.stats;
            if stats.total_attempts == 0 {
                continue;
            }

            let mut timeouts = 0usize;
            let mut stopped_out = 0usize;
            let mut success_days: Vec<f64> = Vec::new();
            for outcome in &execution.analysis.outcomes {
                match outcome.outcome {
                    Outcome::TimedOut { .. } => timeouts += 1,
                    Outcome::StoppedOut { .. } => stopped_out += 1,
                    Outcome::Success { .. } => {
                        if let Some(days) = outcome.days_to_target {
                            success_days.push(days as f64);
                        }
                    }
                }
            }

            let total = stats.total_attempts as f64;
            let success_pct = (stats.success_count as f64 / total) * 100.0;
            let timeout_pct = (timeouts as f64 / total) * 100.0;
            let stopped_pct = (stopped_out as f64 / total) * 100.0;
            let ev_annual = stats.expected_annualized_return;
            let risk = &stats.risk_metrics;
            let worst_loss = risk.worst_case_loss;
            let avg_drawdown = risk.avg_max_drawdown;
            let (median_ttt, p90_ttt) = if success_days.is_empty() {
                (None, None)
            } else {
                success_days.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let len = success_days.len();
                let median_idx = len / 2;
                let p90_pos = ((len - 1) as f64 * 0.9).round() as usize;
                let median = success_days.get(median_idx).copied().unwrap_or(0.0);
                let p90 = success_days.get(p90_pos).copied().unwrap_or(median);
                (Some(median), Some(p90))
            };

            let mut line = format!(
                "{} #{:02} [{:.4}-{:.4}] {} {} ({} {:.1}% | {} {:.1}% | {} {:.1}%) • {} {:.2}%/yr • {} {:.1}% • {} {:.1}%",
                UI_TEXT.journey_zone_line_prefix,
                execution.zone_index,
                execution.zone_bottom,
                execution.zone_top,
                UI_TEXT.journey_zone_label_attempts_short,
                stats.total_attempts,
                UI_TEXT.journey_zone_label_successes_short,
                success_pct,
                UI_TEXT.journey_zone_label_timeouts_short,
                timeout_pct,
                UI_TEXT.journey_zone_label_stops_short,
                stopped_pct,
                UI_TEXT.journey_zone_label_ev_annual_short,
                ev_annual,
                UI_TEXT.journey_zone_label_worst_loss_short,
                worst_loss,
                UI_TEXT.journey_zone_label_avg_drawdown_short,
                avg_drawdown,
            );

            if let (Some(median), Some(p90)) = (median_ttt, p90_ttt) {
                line.push_str(&format!(" • TTT median {:.1}d | p90 {:.1}d", median, p90));
            }

            let color = if execution.direction_up {
                UI_CONFIG.colors.journey_bull
            } else {
                UI_CONFIG.colors.journey_bear
            };

            lines.push((color, line));

            if lines.len() >= UI_CONFIG.max_journey_zone_lines {
                break;
            }
        }

        lines
    }

    #[cfg(debug_assertions)]
    pub(super) fn model_status_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();

        if self.calculation_promise.is_some() {
            lines.push("Model: updating CVA parameters…".to_string());
        }

        let queued = self.journey_queue.len();

        if let Some(aggregate) = &self.journey_aggregate {
            if aggregate.pair_count > 0 {
                let pct = (aggregate.completed_pairs as f64 / aggregate.pair_count as f64) * 100.0;
                if queued > 0 {
                    lines.push(format!(
                        "Journeys: {}/{} pairs ({:.0}%) complete • {} queued",
                        aggregate.completed_pairs, aggregate.pair_count, pct, queued,
                    ));
                } else if aggregate.completed_pairs < aggregate.pair_count {
                    lines.push(format!(
                        "Journeys: {}/{} pairs ({:.0}%) complete",
                        aggregate.completed_pairs, aggregate.pair_count, pct,
                    ));
                }
            }
        } else if queued > 0 {
            lines.push(format!("Journeys: {} pair(s) queued for analysis", queued));
        }

        lines
    }

    pub(super) fn model_status_summary(&self) -> Option<String> {
        let cva_pending = self.calculation_promise.is_some();
        let queued = self.journey_queue.len();

        let completed_pairs = self
            .journey_aggregate
            .as_ref()
            .map(|agg| agg.completed_pairs)
            .unwrap_or(0);

        let total_jobs = completed_pairs + queued;

        // If there's no work in flight and nothing queued, report an explicit idle state.
        if !cva_pending && queued == 0 {
            if let Some(agg) = &self.journey_aggregate {
                if agg.pair_count > 0 && agg.completed_pairs == agg.pair_count {
                    return Some("Model Status: idle (all pairs up to date).".to_string());
                }
            }

            return Some("Model Status: idle.".to_string());
        }

        // If we only know that CVA is running but have no journey progress yet, keep it simple.
        if total_jobs == 0 {
            return Some("Model Update in Progress".to_string());
        }

        let pct = (completed_pairs as f64 / total_jobs as f64) * 100.0;

        Some(format!(
            "Model Update in Progress: {}/{} = {:.0}%.",
            completed_pairs, total_jobs, pct,
        ))
    }

    #[cfg(debug_assertions)]
    fn format_journey_note_line(summary: &JourneySummary) -> String {
        let note = summary
            .note
            .as_deref()
            .unwrap_or(UI_TEXT.journey_status_no_data);

        format!(
            "{} {}: {}",
            UI_TEXT.journey_status_current_prefix, summary.pair, note
        )
    }

    #[cfg(debug_assertions)]
    fn format_aggregate_journey_line(aggregate: &JourneyAggregateSummary) -> Option<String> {
        if aggregate.total_attempts == 0 {
            return None;
        }

        let total = aggregate.total_attempts as f64;
        let success_pct = (aggregate.successes as f64 / total) * 100.0;
        let timeout_pct = (aggregate.timeouts as f64 / total) * 100.0;
        let stopped_pct = (aggregate.stopped_out as f64 / total) * 100.0;

        let mut parts = vec![format!(
            "{}/{} {}",
            aggregate.completed_pairs, aggregate.pair_count, UI_TEXT.journey_status_pairs_label
        )];

        if aggregate.zones_analyzed > 0 {
            parts.push(format!(
                "{} {}",
                aggregate.zones_analyzed, UI_TEXT.journey_status_zones_label
            ));
        }

        parts.push(format!(
            "{} {} ({} {:.1}% | {} {:.1}% | {} {:.1}%)",
            UI_TEXT.journey_status_attempts_label,
            aggregate.total_attempts,
            UI_TEXT.journey_status_success_label,
            success_pct,
            UI_TEXT.journey_status_timeout_label,
            timeout_pct,
            UI_TEXT.journey_status_stopped_label,
            stopped_pct,
        ));

        if let (Some(median), Some(p90)) = (aggregate.median_ttt_days, aggregate.p90_ttt_days) {
            parts.push(format!("TTT median {:.1}d | p90 {:.1}d", median, p90));
        }

        if !aggregate.elapsed.is_zero() {
            parts.push(format!(
                "{} {}",
                UI_TEXT.journey_status_elapsed_label,
                Self::format_duration_short(aggregate.elapsed)
            ));
        }

        if let Some(ago) = aggregate.last_run_ago {
            parts.push(format!(
                "{} {} {}",
                UI_TEXT.journey_status_last_run_label,
                Self::format_duration_short(ago),
                UI_TEXT.journey_status_ago_suffix
            ));
        }

        Some(format!(
            "{}: {}",
            UI_TEXT.journey_status_aggregate_prefix,
            parts.join(" • ")
        ))
    }

    #[cfg(debug_assertions)]
    fn format_duration_short(duration: Duration) -> String {
        if duration.is_zero() {
            return "0s".to_string();
        }

        let millis = duration.as_millis();
        if millis < 1_000 {
            return format!("{}ms", millis);
        }

        let seconds = duration.as_secs_f64();
        if seconds < 60.0 {
            format!("{:.1}s", seconds)
        } else if seconds < 3_600.0 {
            format!("{:.1}m", seconds / 60.0)
        } else if seconds < 86_400.0 {
            format!("{:.1}h", seconds / 3_600.0)
        } else {
            format!("{:.1}d", seconds / 86_400.0)
        }
    }

    pub fn new(
        cc: &eframe::CreationContext<'_>,
        timeseries_collection: TimeSeriesCollection,
    ) -> Self {
        let mut zone_sniper_app: ZoneSniperApp;

        // Attempt to load the persisted state
        if let Some(storage) = cc.storage {
            if let Some(value) = eframe::get_value(storage, eframe::APP_KEY) {
                #[cfg(debug_assertions)]
                if DEBUG_FLAGS.print_state_serde {
                    log::info!("Successfully loaded persisted state");
                }
                zone_sniper_app = value;
            } else {
                #[cfg(debug_assertions)]
                if DEBUG_FLAGS.print_state_serde {
                    log::info!("Failed to get Zone Sniper App state from storage. Creating anew.");
                }
                zone_sniper_app = ZoneSniperApp::new_with_initial_state();
            }
        } else {
            zone_sniper_app = ZoneSniperApp::new_with_initial_state();
        }

        // Initialize the data state with fresh timeseries and generator
        let generator = ZoneGenerator::default();
        zone_sniper_app.data_state = DataState::new(timeseries_collection, generator);

        // Explicitly reinitialize plot_view (it's skipped during serialization)
        zone_sniper_app.plot_view = PlotView::new();

        // Initialize trigger maps with all available pairs
        zone_sniper_app.pair_triggers = HashMap::new();
        zone_sniper_app.journey_triggers = HashMap::new();
        zone_sniper_app.last_calculated_params_by_pair = HashMap::new();
        zone_sniper_app.cva_results_by_pair = HashMap::new();
        zone_sniper_app.journey_summaries = HashMap::new();
        zone_sniper_app.journey_aggregate = None;

        // Validate that we have available pairs and sync trigger maps
        let available_pairs = zone_sniper_app.sync_pair_triggers();
        zone_sniper_app.sync_journey_triggers();
        if available_pairs.is_empty() {
            zone_sniper_app.data_state.last_error = Some(AppError::DataNotAvailable);
            #[cfg(debug_assertions)]
            log::error!("No trading pairs available in timeseries collection");
            return zone_sniper_app;
        }

        // Validate that the selected pair exists in current data, or pick the first one
        if let Some(selected_pair) = &zone_sniper_app.selected_pair {
            if !available_pairs.contains(selected_pair) {
                #[cfg(debug_assertions)]
                log::info!(
                    "Selected pair '{}' not found, defaulting to first available pair",
                    selected_pair
                );
                zone_sniper_app.selected_pair = available_pairs.first().cloned();
            }
        } else {
            // No pair selected, pick the first one
            zone_sniper_app.selected_pair = available_pairs.first().cloned();
        }

        // Ensure initial trigger state is marked for recalculation
        zone_sniper_app.mark_selected_pair_stale("initial load");

        // Validate zone count
        if zone_sniper_app.zone_count == 0 {
            #[cfg(debug_assertions)]
            if DEBUG_FLAGS.print_ui_interactions {
                log::info!("Warning: Zone count is 0, setting to default");
            }
            zone_sniper_app.zone_count = default_zone_count();
        }

        // Generate initial data
        zone_sniper_app.generate_data_based_on_start_ui(cc);

        // Note: We'll initialize the multi-pair monitor after we have prices from WebSocket
        // This happens in the update() loop when prices first arrive

        zone_sniper_app
    }

    pub fn new_with_initial_state() -> Self {
        Self {
            selected_pair: default_selected_pair(),
            zone_count: default_zone_count(),
            time_decay_factor: default_time_decay_factor(),
            time_horizon_days: default_time_horizon_days(),
            auto_duration_config: crate::domain::auto_duration::AutoDurationConfig::default(),
            computed_slice_indices: None,
            last_price_range: None,
            data_state: DataState::default(),
            plot_view: PlotView::default(),
            last_calculated_params: None,
            last_calculated_params_by_pair: HashMap::new(),
            last_failed_params_by_pair: HashMap::new(),
            cva_results_by_pair: HashMap::new(),
            journey_executions_by_pair: HashMap::new(),
            show_debug_help: false,
            calculation_promise: None,
            current_pair_price: None,
            price_stream: None,
            multi_pair_monitor: MultiPairMonitor::new(),
            pair_triggers: HashMap::new(),
            journey_triggers: HashMap::new(),
            journey_queue: VecDeque::new(),
            journey_summaries: HashMap::new(),
            journey_aggregate: None,
            monitor_initialized: false,
            is_simulation_mode: false,
            simulated_prices: std::collections::HashMap::new(),
            sim_direction: SimDirection::default(),
            sim_step_size: SimStepSize::default(),
        }
    }

    fn generate_data_based_on_start_ui(&mut self, _cc: &eframe::CreationContext<'_>) {
        self.schedule_selected_pair_recalc("initial load");
    }

    pub(super) fn initialize_multi_pair_monitor(&mut self) {
        let mut all_pairs = self.data_state.timeseries_collection.unique_pair_names();

        // Swap selected pair to the front
        if let Some(selected) = &self.selected_pair {
            if let Some(idx) = all_pairs.iter().position(|p| p == selected) {
                all_pairs.swap(0, idx);
            }
        }

        let initial_count = self.multi_pair_monitor.pair_count();

        if let Some(ref stream) = self.price_stream {
            for pair_name in &all_pairs {
                // Skip if already tracked
                if self.multi_pair_monitor.get_context(pair_name).is_some() {
                    continue;
                }

                // Only add if we have a price
                if let Some(price) = stream.get_price(pair_name) {
                    // Calculate auto-duration slice for this pair
                    if let Ok(timeseries) = crate::models::timeseries::find_matching_ohlcv(
                        &self.data_state.timeseries_collection.series_data,
                        pair_name,
                        crate::config::INTERVAL_WIDTH_TO_ANALYSE_MS,
                    ) {
                        let (ranges, price_range) =
                            crate::domain::auto_duration::auto_select_ranges(
                                timeseries,
                                price,
                                &self.auto_duration_config,
                            );

                        // Calculate CVA for this pair
                        if let Ok(cva_results) = self.data_state.generator.get_cva_results(
                            pair_name,
                            self.zone_count,
                            self.time_decay_factor,
                            &self.data_state.timeseries_collection,
                            ranges,
                            price_range,
                        ) {
                            // Create trading model and context
                            let trading_model = TradingModel::from_cva(cva_results, Some(price));
                            let context = PairContext::new(trading_model, price);
                            self.multi_pair_monitor.add_pair(context);

                            #[cfg(debug_assertions)]
                            if DEBUG_FLAGS.print_monitor_progress {
                                log::info!(
                                    "✨ Added {} to monitor (price: {:.2})",
                                    pair_name,
                                    price
                                );
                            }
                        }
                    }
                }
            }
        }

        let final_count = self.multi_pair_monitor.pair_count();
        if final_count > initial_count {
            #[cfg(debug_assertions)]
            if DEBUG_FLAGS.print_monitor_progress {
                log::info!(
                    "📊 Monitor now tracking {}/{} pairs",
                    final_count,
                    all_pairs.len()
                );
            }
        }

        // Mark as initialized once we have most pairs (or after a few attempts)
        if final_count >= all_pairs.len() * 8 / 10 {
            let was_initialized = self.monitor_initialized;
            self.monitor_initialized = true;
            #[cfg(debug_assertions)]
            if DEBUG_FLAGS.print_monitor_progress {
                log::info!(
                    "✅ Monitor initialization complete: {}/{} pairs tracked\n",
                    final_count,
                    all_pairs.len()
                );
            }

            if !was_initialized {
                let contexts = self.multi_pair_monitor.get_all_contexts();
                let pair_names: Vec<String> = contexts
                    .into_iter()
                    .map(|ctx| ctx.pair_name.clone())
                    .collect();

                for pair_name in pair_names {
                    self.mark_journey_stale_and_enqueue(&pair_name, "initialisation");
                }
            }
        }
    }
}

impl eframe::App for ZoneSniperApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Cancel and clean up any ongoing async calculation
        if let Some(promise) = self.calculation_promise.take() {
            drop(promise);
        }

        // Drop all promise-related state to prevent "Sender dropped" panic
        self.calculation_promise = None;

        #[cfg(debug_assertions)]
        if DEBUG_FLAGS.print_shutdown {
            log::info!("Application shutdown complete.");
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        setup_custom_visuals(ctx);

        // Poll async calculation
        self.poll_async_calculation(ctx);

        // Drain any pending journey work for pairs whose CVA params have
        // changed or that were enqueued during monitor initialisation.
        self.drain_journey_queue();

        self.handle_global_shortcuts(ctx);

        self.render_side_panel(ctx);
        self.render_central_panel(ctx);
        self.render_status_panel(ctx);
        if self.show_debug_help {
            self.render_help_panel(ctx);
        }

        self.drain_trigger_queue();
    }
}
