use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

// Only import thread on non-WASM targets
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

use super::messages::{JobRequest, JobResult};
use crate::analysis::horizon_profiler::generate_profile;
use crate::analysis::pair_analysis::pair_analysis_pure;
use crate::config::ANALYSIS;
use crate::models::timeseries::find_matching_ohlcv; // Needed for data lookup
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

// src/engine/worker.rs

pub fn process_request_sync(req: JobRequest, tx: Sender<JobResult>) {
    crate::trace_time!("Total Worker Job", 5000, {
        let start = AppInstant::now();

        // 1. Calculate Exact Count First (The Truth)
        // We run the range selection logic here to get the authoritative number
        // for the requested config.
        // 1. Calculate Exact Count First (The Truth)
        // We must look up the specific timeseries for this pair to run the range logic.
        let ohlcv_result = find_matching_ohlcv(
            &req.timeseries.series_data,
            &req.pair_name,
            ANALYSIS.interval_width_ms,
        );

        let exact_candle_count: usize = if let Ok(ohlcv) = ohlcv_result {
            let (ranges, _) = crate::domain::price_horizon::auto_select_ranges(
                ohlcv,
                req.current_price,
                &req.config.price_horizon,
            );
            ranges.iter().map(|(s, e)| e - s).sum()
        } else {
            0
        };

        // 2. Run CVA Logic
        let result_cva = pair_analysis_pure(
            req.pair_name.clone(),
            &req.timeseries,
            req.current_price,
            &req.config.price_horizon,
        );

        // 3. Run Profiler
        let mut profile = generate_profile(
            &req.pair_name,
            &req.timeseries,
            req.current_price,
            &req.config.price_horizon,
        );

        // --- THE FIX: PATCH THE PROFILE ---
        // Find the bucket corresponding to the current configuration and force it
        // to match the exact count we just calculated.
        // This ensures the "Map" perfectly matches the "Territory" at the current location.
        let current_pct = req.config.price_horizon.threshold_pct;

        if let Some(bucket) = profile.buckets.iter_mut().min_by(|a, b| {
            (a.threshold_pct - current_pct)
                .abs()
                .partial_cmp(&(b.threshold_pct - current_pct).abs())
                .unwrap()
        }) {
            // Overwrite with the Truth
            bucket.candle_count = exact_candle_count;
            // Optionally update duration_days too for consistency, but count is the critical one.
        }
        // ----------------------------------

        let elapsed = start.elapsed().as_millis();

        match result_cva {
            Ok(cva) => {
                let cva_arc = Arc::new(cva);
                let model = TradingModel::from_cva(cva_arc.clone(), profile.clone());

                let _ = tx.send(JobResult {
                    pair_name: req.pair_name,
                    duration_ms: elapsed,
                    result: Ok(Arc::new(model)),
                    cva: Some(cva_arc),
                    profile: Some(profile),
                    candle_count: exact_candle_count, // <--- Send it
                });
            }
            Err(e) => {
                let _ = tx.send(JobResult {
                    pair_name: req.pair_name,
                    duration_ms: elapsed,
                    result: Err(e.to_string()),
                    cva: None,
                    profile: Some(profile),
                    candle_count: exact_candle_count, // <--- Send it here too!
                });
            }
        }
    });
}
