use std::sync::{Arc, RwLock};

use crate::config::{OptimizationStrategy, PhPct, Price, StationId};

use crate::data::timeseries::TimeSeriesCollection;

use crate::models::TradingModel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JobMode {
    /// Standard operation: Calculate CVA, then run Pathfinder (Scouts/Drills)
    FullAnalysis,

    /// Visualization only: Calculate CVA (Zones/Volume Profile) but SKIP Pathfinder.
    /// Used when clicking an existing trade to restore the chart context.
    ContextOnly,
}

/// A request to calculate a model for a specific pair
/// /// Invariant:
/// - Each JobRequest is immutable
/// - Exactly one JobRequest per pair may be in-flight
#[derive(Debug, Clone)]
pub(crate) struct JobRequest {
    pub pair_name: String,
    pub current_price: Option<Price>,
    pub timeseries: Arc<RwLock<TimeSeriesCollection>>,
    pub ph_pct: PhPct,
    pub strategy: OptimizationStrategy,
    pub station_id: StationId,
    pub mode: JobMode,
}

/// The result returned by the worker
#[derive(Debug, Clone)]
pub(crate) struct JobResult {
    pub pair_name: String,
    pub result: Result<Arc<TradingModel>, String>,
}
