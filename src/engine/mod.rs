#[cfg(feature = "backtest")]
mod backtest;
mod core;
mod messages;
mod tuner;
mod worker;

pub use core::SniperEngine;

pub(crate) use {
    messages::{JobMode, JobRequest, JobResult},
    tuner::{StationId, TUNER_CONFIG, TimeTunerConfig, TunerStation, tune_to_station},
    worker::run_pathfinder_simulations,
};

#[cfg(feature = "backtest")]
pub(crate) use backtest::{
    BACKTEST_MODEL_DESC, BACKTEST_MODEL_VERSION, BACKTEST_PAIR_COUNT, BACKTEST_SKIP_DB_WRITE,
    BacktestConfig, run_backtest,
};

#[cfg(target_arch = "wasm32")]
pub(crate) use worker::process_request_sync;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use worker::spawn_worker_thread;
