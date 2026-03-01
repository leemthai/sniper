#[cfg(feature = "backtest")]
pub(crate) mod backtest;
mod core;
mod messages;
mod worker;

pub use core::SniperEngine;

pub(crate) use {
    messages::{JobMode, JobRequest, JobResult},
    worker::tune_to_station,
};

#[cfg(any(feature = "ph_audit", feature = "backtest"))]
pub(crate) use worker::run_pathfinder_simulations;

#[cfg(target_arch = "wasm32")]
pub(crate) use worker::process_request_sync;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use worker::spawn_worker_thread;
