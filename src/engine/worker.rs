use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

// Only import thread on non-WASM targets
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

use super::messages::{JobRequest, JobResult};
use crate::analysis::horizon_profiler::generate_profile;
use crate::analysis::pair_analysis::pair_analysis_pure;
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

pub fn process_request_sync(req: JobRequest, tx: Sender<JobResult>) {
    crate::trace_time!("Total Worker Job", 5000, {
        let start = AppInstant::now();

        // 0. RESOLVE PRICE
        // Handle the Option<f64>. If None, look up the last close price from DB.
        let price_for_analysis = req.current_price.unwrap_or_else(|| {
            if let Ok(ts) = find_matching_ohlcv(
                &req.timeseries.series_data,
                &req.pair_name,
                req.config.interval_width_ms
            ) {
                 ts.close_prices.last().copied().unwrap_or(0.0)
            } else {
                0.0
            }
        });

        // 1. Calculate Exact Count First (The Truth)
        let ohlcv_result = find_matching_ohlcv(
            &req.timeseries.series_data,
            &req.pair_name,
            req.config.interval_width_ms, // Use config from request
        );

        let exact_candle_count: usize = if let Ok(ohlcv) = ohlcv_result {
            let (ranges, _) = crate::domain::price_horizon::auto_select_ranges(
                ohlcv,
                price_for_analysis,
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
            price_for_analysis,
            &req.config,
        );

        // 3. Run Profiler (SMART CACHING OPTIMIZATION)
        let profile = if let Some(existing) = req.existing_profile {
            // Check if valid: Same Price? Same Bounds?
            // Note: EPSILON check is safer for floats
            let price_match = (existing.base_price - price_for_analysis).abs() < f64::EPSILON;
            let min_match = (existing.min_pct - req.config.price_horizon.min_threshold_pct).abs() < f64::EPSILON;
            let max_match = (existing.max_pct - req.config.price_horizon.max_threshold_pct).abs() < f64::EPSILON;

            if price_match && min_match && max_match {
                #[cfg(debug_assertions)]
                log::debug!("Worker: Reusing Cached Profile for {}", req.pair_name);
                existing
            } else {
                generate_profile(
                    &req.pair_name,
                    &req.timeseries,
                    price_for_analysis,
                    &req.config.price_horizon,
                )
            }
        } else {
            generate_profile(
                &req.pair_name,
                &req.timeseries,
                price_for_analysis,
                &req.config.price_horizon,
            )
        };

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
                    candle_count: exact_candle_count, 
                });
            }
            Err(e) => {
                let _ = tx.send(JobResult {
                    pair_name: req.pair_name,
                    duration_ms: elapsed,
                    result: Err(e.to_string()),
                    cva: None,
                    profile: Some(profile),
                    candle_count: exact_candle_count, 
                });
            }
        }
    });
}