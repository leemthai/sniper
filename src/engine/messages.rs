use {
    crate::{
        config::{PhPct, Price, StationId},
        data::TimeSeriesCollection,
        models::{OptimizationStrategy, TradingModel},
    },
    std::sync::{Arc, RwLock},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JobMode {
    FullAnalysis,
    ContextOnly,
}

/// Job request for pair analysis.
/// Invariant: Immutable, exactly one per pair in-flight.
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

#[derive(Debug, Clone)]
pub(crate) struct JobResult {
    pub pair_name: String,
    pub result: Result<Arc<TradingModel>, String>,
}
