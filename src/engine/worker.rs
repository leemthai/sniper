use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

// Only import thread on non-WASM targets
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

use super::messages::{JobRequest, JobResult};
use crate::analysis::pair_analysis;
use crate::models::trading_view::TradingModel;
use crate::utils::app_time::AppInstant;

/// NATIVE ONLY: Spawns a background thread to process jobs
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_worker_thread(rx: Receiver<JobRequest>, tx: Sender<JobResult>) {
    thread::spawn(move || {
        while let Ok(req) = rx.recv() {
            process_request_sync(req, tx.clone());
        }
    });
}

/// WASM ONLY: No-op. 
/// The Engine holds the receiver and processes jobs manually in the update loop.
#[cfg(target_arch = "wasm32")]
pub fn spawn_worker_thread(_rx: Receiver<JobRequest>, _tx: Sender<JobResult>) {
    // Do nothing.
}

/// SHARED: The actual calculation logic (Sync)
/// Called by thread loop (Native) or update loop (WASM)
pub fn process_request_sync(req: JobRequest, tx: Sender<JobResult>) {
    crate::trace_time!("Total Worker Job", 5000, {
        let start = AppInstant::now();

        let result_cva = pair_analysis::pair_analysis_pure(
            req.pair_name.clone(),
            &req.timeseries,
            req.current_price,
            &req.config.price_horizon,
        );

        let elapsed = start.elapsed().as_millis();

        match result_cva {
            Ok(cva) => {
                let cva_arc = Arc::new(cva);
                // The worker builds the data (the model)
                let model = TradingModel::from_cva(cva_arc.clone());
                
                // The worker wraps it in Arc::new() and sends it down the channel (tx)
                // We use if-let or ignore error to prevent panic if receiver (Engine) is dropped
                let _ = tx.send(JobResult {
                    pair_name: req.pair_name,
                    duration_ms: elapsed,
                    result: Ok(Arc::new(model)),
                    cva: Some(cva_arc),
                });
            }
            Err(e) => {
                let _ = tx.send(JobResult {
                    pair_name: req.pair_name,
                    duration_ms: elapsed,
                    result: Err(e.to_string()),
                    cva: None,
                });
            }
        }
    });
}