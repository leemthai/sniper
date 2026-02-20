mod core;
mod messages;
mod worker;

pub use core::SniperEngine;

pub(crate) use messages::{JobMode, JobRequest, JobResult};

#[cfg(target_arch = "wasm32")]
pub(crate) use worker::process_request_sync;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use worker::spawn_worker_thread;

pub(crate) use worker::tune_to_station;
