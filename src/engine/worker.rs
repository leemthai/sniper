use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

// Only import thread on non-WASM targets
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

use super::messages::{JobRequest, JobResult};

use crate::analysis::horizon_profiler::generate_profile;
use crate::analysis::pair_analysis::pair_analysis_pure;
use crate::analysis::scenario_simulator::{ScenarioSimulator};

use crate::config::AnalysisConfig;

use crate::domain::price_horizon;

use crate::models::OhlcvTimeSeries;
use crate::models::cva::CVACore;
use crate::models::horizon_profile::HorizonProfile;
use crate::models::timeseries::find_matching_ohlcv;
use crate::models::trading_view::{SuperZone, TradeOpportunity};

use crate::TradingModel;

use crate::utils::time_utils::AppInstant;

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

// Helper: Runs Ghost Runner simulations for all visible sticky zones
fn run_pathfinder_simulations(
    ohlcv: &OhlcvTimeSeries,
    sticky_zones: &[SuperZone],
    current_price: f64,
    config: &AnalysisConfig,
) -> Vec<TradeOpportunity> {
    let mut opportunities = Vec::new();

    // 1. Calculate Adaptive Trend Lookback
    let lookback = AnalysisConfig::calculate_trend_lookback(config.price_horizon.threshold_pct);

    // --- LOG START ---
    #[cfg(debug_assertions)]
    log::info!(
        "PATHFINDER START: Scanning {} zones. Lookback: {} candles.",
        sticky_zones.len(),
        lookback
    );
    // ----------------

    let duration_ms = config.journey.max_journey_time.as_millis() as i64;
    // Use the actual interval from the data series to determine candle count
    let interval_ms = ohlcv.pair_interval.interval_ms; 
    let duration_candles = (duration_ms / interval_ms) as usize;
    
    let stop_pct = config.journey.stop_loss_pct / 100.0;
    
    // Get current index once
    let current_idx = ohlcv.klines().saturating_sub(1);

    // --- OPTIMIZATION START ---
    // Scan History ONCE.
    let historical_matches = ScenarioSimulator::find_historical_matches(
        ohlcv,
        current_idx,
        lookback,
        duration_candles,
        config.journey.sample_count 
    );

    if historical_matches.is_empty() {
        #[cfg(debug_assertions)]
        log::warn!("PATHFINDER ABORT: Insufficient data or matches (Lookback {}).", lookback);
        return Vec::new();
    }
    // --- OPTIMIZATION END ---

    // 3. Scan Targets
    for (_i, zone) in sticky_zones.iter().enumerate() {
        let (is_valid, target_price, stop_price, direction) = if current_price < zone.price_bottom {
             (true, zone.price_bottom, current_price * (1.0 - stop_pct), "Long".to_string())
        } else if current_price > zone.price_top {
             (true, zone.price_top, current_price * (1.0 + stop_pct), "Short".to_string())
        } else {
             (false, 0.0, 0.0, String::new())
        };

        if is_valid {
            // Replay using the pre-calculated historical_matches
            if let Some(result) = ScenarioSimulator::analyze_outcome(
                ohlcv,
                &historical_matches,
                current_price,
                target_price,
                stop_price,
                duration_candles,
                &direction
            ) {
                // LOG SUCCESS for first few
                #[cfg(debug_assertions)]
                if _i < 3 {
                     log::info!("   -> Zone {}: {} Win Rate {:.1}% (Samples: {})", _i, direction, result.success_rate * 100.0, result.sample_size);
                }

                opportunities.push(TradeOpportunity {
                    target_zone_id: zone.id,
                    direction,
                    target_price,
                    stop_price,
                    simulation: result,
                });
            }
        }
    }
    opportunities
}

pub fn process_request_sync(req: JobRequest, tx: Sender<JobResult>) {
    let ph_pct = req.config.price_horizon.threshold_pct * 100.0;
    let base_label = format!("{} @ {:.2}%", req.pair_name, ph_pct);

    crate::trace_time!(&format!("Total JOB [{}]", base_label), 5000, {
        let start = AppInstant::now();

        // 1. Resolve Inputs
        let price = resolve_analysis_price(&req);

        // 2. Exact Count (Range Logic)
        let count = crate::trace_time!(&format!("1. Exact Count [{}]", base_label), 500, {
            calculate_exact_candle_count(&req, price)
        });

        let full_label = format!("{} ({} candles)", base_label, count);

        // 3. CVA Logic
        let result_cva = crate::trace_time!(&format!("2. CVA Calc [{}]", full_label), 1000, {
            pair_analysis_pure(req.pair_name.clone(), &req.timeseries, price, &req.config)
        });

        // 4. Profiler (Smart Cache)
        let profile = crate::trace_time!(&format!("3. Profiler [{}]", full_label), 1000, {
            get_or_generate_profile(&req, price)
        });

        let elapsed = start.elapsed().as_millis();

        // 5. Final Assembly
        let response = match result_cva {
            Ok(cva) => build_success_result(&req, cva, profile, price, count, elapsed),
            Err(e) => JobResult {
                pair_name: req.pair_name.clone(),
                duration_ms: elapsed,
                result: Err(e.to_string()),
                cva: None,
                profile: Some(profile),
                candle_count: count,
            },
        };

        let _ = tx.send(response);
    });
}

fn resolve_analysis_price(req: &JobRequest) -> f64 {
    req.current_price.unwrap_or_else(|| {
        if let Ok(ts) = find_matching_ohlcv(
            &req.timeseries.series_data,
            &req.pair_name,
            req.config.interval_width_ms,
        ) {
            ts.close_prices.last().copied().unwrap_or(0.0)
        } else {
            0.0
        }
    })
}

fn calculate_exact_candle_count(req: &JobRequest, price: f64) -> usize {
    let ohlcv_result = find_matching_ohlcv(
        &req.timeseries.series_data,
        &req.pair_name,
        req.config.interval_width_ms,
    );

    if let Ok(ohlcv) = ohlcv_result {
        let (ranges, _) =
            price_horizon::auto_select_ranges(ohlcv, price, &req.config.price_horizon);
        ranges.iter().map(|(s, e)| e - s).sum()
    } else {
        0
    }
}

fn get_or_generate_profile(req: &JobRequest, price: f64) -> HorizonProfile {
    if let Some(existing) = &req.existing_profile {
        let cfg = &req.config.price_horizon;
        let price_match = (existing.base_price - price).abs() < f64::EPSILON;
        let min_match = (existing.min_pct - cfg.min_threshold_pct).abs() < f64::EPSILON;
        let max_match = (existing.max_pct - cfg.max_threshold_pct).abs() < f64::EPSILON;

        if price_match && min_match && max_match {
            #[cfg(debug_assertions)]
            log::debug!("Worker: Reusing Cached Profile for {}", req.pair_name);
            return existing.clone();
        }
    }

    generate_profile(
        &req.pair_name,
        &req.timeseries,
        price,
        &req.config.price_horizon,
    )
}

fn build_success_result(
    req: &JobRequest,
    cva: CVACore,
    profile: HorizonProfile,
    price: f64,
    count: usize,
    elapsed: u128,
) -> JobResult {
    // 1. Fetch OHLCV (Must exist if CVA succeeded)
    let ohlcv = find_matching_ohlcv(
        &req.timeseries.series_data,
        &req.pair_name,
        req.config.interval_width_ms,
    )
    .expect("OHLCV data missing despite CVA success");

    let cva_arc = Arc::new(cva);

    // 2. Create Model
    let mut model = TradingModel::from_cva(cva_arc.clone(), profile.clone(), ohlcv, &req.config);

    // 3. Run Pathfinder
    // (Note: Requires the helper function 'run_pathfinder_simulations' we added earlier in this file)
    let opps = crate::trace_time!(&format!("5. Pathfinder [{}]", req.pair_name), 500, {
        run_pathfinder_simulations(
            ohlcv,
            &model.zones.sticky_superzones,
            price,
            &req.config,
        )
    });

    // --- DEBUG LOGGING ---
    #[cfg(debug_assertions)]
    if !opps.is_empty() {
        // Just log the top one to check sanity
        if let Some(best) = opps.iter().max_by(|a, b| {
            a.simulation
                .success_rate
                .partial_cmp(&b.simulation.success_rate)
                .unwrap()
        }) {
            log::info!(
                "🎯 PATHFINDER [{}]: Found {} opps. Best: {} to {:.2} (Win: {:.1}% | EV: {:.2} | Samples: {})",
                req.pair_name,
                opps.len(),
                best.direction,
                best.target_price,
                best.simulation.success_rate * 100.0,
                best.simulation.risk_reward_ratio, // or expected value if you calculated it
                best.simulation.sample_size
            );
        }
    } else {
        log::debug!(
            "PATHFINDER [{}]: No valid setups found (Price might be inside a zone/mud).",
            req.pair_name
        );
    }

    model.opportunities = opps;

    JobResult {
        pair_name: req.pair_name.clone(),
        duration_ms: elapsed,
        result: Ok(Arc::new(model)),
        cva: Some(cva_arc),
        profile: Some(profile),
        candle_count: count,
    }
}
