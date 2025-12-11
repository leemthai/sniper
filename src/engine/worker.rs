use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use crate::analysis::pair_analysis;
use crate::models::trading_view::TradingModel;
use super::messages::{JobRequest, JobResult};

pub fn spawn_worker_thread(
    rx: Receiver<JobRequest>,
    tx: Sender<JobResult>,
) {
    thread::spawn(move || {
        while let Ok(req) = rx.recv() {
            let start = Instant::now();
            
            // 1. Run the heavy calculation (Pure function)
            // Note: We need to expose a version of pair_analysis that accepts raw data, 
            // not the ZoneGenerator struct. (We will do this refactor next).
            
            // Placeholder until we refactor pair_analysis signature:
            let result_cva = pair_analysis::pair_analysis_pure(
                req.pair_name.clone(),
                &req.timeseries,
                // We need to calculate slice ranges here or inside pair_analysis
                req.current_price,
                &req.config.price_horizon,
            );

            let elapsed = start.elapsed().as_millis();

            match result_cva {
                Ok(cva) => {
                    let cva_arc = Arc::new(cva);
                    // The worker builds the data (the model)
                    let model = TradingModel::from_cva(cva_arc.clone(), Some(req.current_price));
                    // The worker wraps it in Arc::new() and sends it down the channel (tx)
                    tx.send(JobResult {
                        pair_name: req.pair_name,
                        duration_ms: elapsed,
                        result: Ok(Arc::new(model)), // <- Sneding the Arc
                        cva: Some(cva_arc),
                    }).unwrap();
                }
                Err(e) => {
                    tx.send(JobResult {
                        pair_name: req.pair_name,
                        duration_ms: elapsed,
                        result: Err(e.to_string()),
                        cva: None,
                    }).unwrap();
                }
            }
        }
    });
}