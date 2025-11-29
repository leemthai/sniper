use anyhow::Result;
use std::sync::Arc;

use crate::TimeSeriesCollection;
use crate::analysis::pair_analysis::ZoneGenerator;
use crate::config::ANALYSIS;
use crate::config::DEBUG_FLAGS;
use crate::journeys::zone_efficacy::{ZoneEfficacyStats, compute_zone_efficacy};
use crate::models::{CVACore, TradingModel, find_matching_ohlcv};

pub const DECAY_CANDIDATES: &[f64] = &[
    0.10, 0.30, 0.50, 0.70, 0.80, 0.85, 0.90, 0.93, 0.95, 0.97, 0.98, 0.985, 0.99, 0.995, 0.998,
];

const W_MEDIAN: f64 = 1.0;
const W_P90: f64 = 0.3;
const W_CADENCE: f64 = 0.5;
const W_TIME_DEFICIT: f64 = 2.0;
const W_STALE_PENALTY: f64 = 1.0;
const W_FLICKER_PENALTY: f64 = 0.7;
const CADENCE_FLOOR_PER_K: f64 = 0.5;
const SHORT_RUN_THRESHOLD: f64 = 0.4;

#[derive(Debug, Clone)]
pub struct ScoreBreakdown {
    pub total_score: f64,
    pub median_component: f64,
    pub p90_component: f64,
    pub cadence_component: f64,
    pub time_deficit_penalty: f64,
    pub stale_penalty: f64,
    pub flicker_penalty: f64,
}

#[derive(Debug, Clone)]
pub struct DecayCandidateEvaluation {
    pub decay: f64,
    pub stats: ZoneEfficacyStats,
    pub score: ScoreBreakdown,
}

#[derive(Debug, Clone)]
pub struct DecayCalibrationResult {
    pub best_decay: f64,
    pub best_stats: ZoneEfficacyStats,
    pub best_score: ScoreBreakdown,
    pub best_cva: Arc<CVACore>,
    pub candidates: Vec<DecayCandidateEvaluation>,
}

pub fn calibrate_time_decay(
    generator: &ZoneGenerator,
    timeseries_collection: &TimeSeriesCollection,
    pair: &str,
    zone_count: usize,
    slice_ranges: &[(usize, usize)],
    price_range: (f64, f64),
    current_price: Option<f64>,
) -> Result<Option<DecayCalibrationResult>> {
    let ohlcv = match find_matching_ohlcv(
        &timeseries_collection.series_data,
        pair,
        ANALYSIS.interval_width_ms,
    ) {
        Ok(ts) => ts,
        Err(_) => return Ok(None),
    };

    let mut evaluations: Vec<DecayCandidateEvaluation> = Vec::new();
    let mut best_candidate: Option<(DecayCandidateEvaluation, Arc<CVACore>)> = None;

    for &decay in DECAY_CANDIDATES {
        let cva = match generator.get_cva_results(
            pair,
            zone_count,
            decay,
            timeseries_collection,
            slice_ranges.to_vec(),
            price_range,
        ) {
            Ok(cva) => cva,
            Err(err) => {
                if cfg!(debug_assertions) && DEBUG_FLAGS.print_decay_calibration {
                    log::error!(
                        "Time-decay sweep: failed to compute CVA for {} @ decay {:.3}: {}",
                        pair,
                        decay,
                        err
                    );
                }
                continue;
            }
        };

        let trading_model = TradingModel::from_cva(Arc::clone(&cva), current_price);
        let stats = match compute_zone_efficacy(
            ohlcv,
            &trading_model.zones.sticky_superzones,
            slice_ranges,
            price_range,
        ) {
            Some(stats) => stats,
            None => continue,
        };

        let score = score_candidate(&stats);
        let evaluation = DecayCandidateEvaluation {
            decay,
            stats: stats.clone(),
            score: score.clone(),
        };

        if best_candidate.is_none()
            || evaluation.score.total_score
                > best_candidate
                    .as_ref()
                    .map(|(best, _)| best.score.total_score)
                    .unwrap_or(f64::MIN)
            || ((evaluation.score.total_score
                - best_candidate
                    .as_ref()
                    .map(|(best, _)| best.score.total_score)
                    .unwrap_or(f64::MIN))
            .abs()
                <= f64::EPSILON
                && evaluation.decay
                    > best_candidate
                        .as_ref()
                        .map(|(best, _)| best.decay)
                        .unwrap_or(0.0))
        {
            best_candidate = Some((evaluation.clone(), Arc::clone(&cva)));
        }

        evaluations.push(evaluation);
    }

    if evaluations.is_empty() {
        return Ok(None);
    }

    evaluations.sort_by(|a, b| {
        b.score
            .total_score
            .partial_cmp(&a.score.total_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if let Some((best_eval, best_cva)) = best_candidate {
        let print_decay_calibration = cfg!(debug_assertions) && DEBUG_FLAGS.print_decay_calibration;
        if print_decay_calibration {
            log::info!("--- Time-decay calibration for {} ---", pair);
            log::info!(
                "Best decay: {:.3} (score {:.3}) | median {:.1}c | p90 {:.1}c | cadence {:.2}/1k | time∆ {:.1}%",
                best_eval.decay,
                best_eval.score.total_score,
                best_eval
                    .stats
                    .dwell_durations
                    .map(|d| d.median_candles)
                    .unwrap_or(0.0),
                best_eval
                    .stats
                    .dwell_durations
                    .map(|d| d.p90_candles)
                    .unwrap_or(0.0),
                best_eval
                    .stats
                    .dwell_durations
                    .map(|d| d.runs_per_1k_candles)
                    .unwrap_or(0.0),
                best_eval.stats.time_in_zones_pct - best_eval.stats.price_occupancy_pct
            );
            for candidate in &evaluations {
                log::info!(
                    "  decay {:.3} -> score {:.3} | median {:.1}c | p90 {:.1}c | cadence {:.2}/1k | stale {:.3} | flicker {:.3}",
                    candidate.decay,
                    candidate.score.total_score,
                    candidate
                        .stats
                        .dwell_durations
                        .map(|d| d.median_candles)
                        .unwrap_or(0.0),
                    candidate
                        .stats
                        .dwell_durations
                        .map(|d| d.p90_candles)
                        .unwrap_or(0.0),
                    candidate
                        .stats
                        .dwell_durations
                        .map(|d| d.runs_per_1k_candles)
                        .unwrap_or(0.0),
                    candidate.score.stale_penalty,
                    candidate.score.flicker_penalty
                );
            }
            log::info!("-------------------------------------------");
        }

        return Ok(Some(DecayCalibrationResult {
            best_decay: best_eval.decay,
            best_stats: best_eval.stats,
            best_score: best_eval.score,
            best_cva,
            candidates: evaluations,
        }));
    }

    Ok(None)
}

fn score_candidate(stats: &ZoneEfficacyStats) -> ScoreBreakdown {
    let dwell = stats.dwell_durations;
    let median = dwell.map(|d| d.median_candles).unwrap_or(0.0);
    let p90 = dwell.map(|d| d.p90_candles).unwrap_or(0.0);
    let cadence = dwell.map(|d| d.runs_per_1k_candles).unwrap_or(0.0);
    let short_ratio = dwell.map(|d| d.short_run_ratio).unwrap_or(1.0);

    let time_delta = stats.time_in_zones_pct - stats.price_occupancy_pct;
    let time_deficit_penalty = if time_delta < 0.0 {
        time_delta.abs() * W_TIME_DEFICIT
    } else {
        0.0
    };

    let stale_penalty = if cadence < CADENCE_FLOOR_PER_K {
        (CADENCE_FLOOR_PER_K - cadence) * W_STALE_PENALTY
    } else {
        0.0
    };

    let flicker_penalty = if short_ratio > SHORT_RUN_THRESHOLD {
        (short_ratio - SHORT_RUN_THRESHOLD) * W_FLICKER_PENALTY
    } else {
        0.0
    };

    let median_component = median * W_MEDIAN;
    let p90_component = p90 * W_P90;
    let cadence_component = cadence * W_CADENCE;

    let total_score = median_component + p90_component + cadence_component
        - time_deficit_penalty
        - stale_penalty
        - flicker_penalty;

    ScoreBreakdown {
        total_score,
        median_component,
        p90_component,
        cadence_component,
        time_deficit_penalty,
        stale_penalty,
        flicker_penalty,
    }
}
