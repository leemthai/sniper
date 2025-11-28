use std::collections::HashMap;

use crate::config::PRINT_ZONE_TRANSITION_SUMMARY;
use crate::models::{OhlcvTimeSeries, SuperZone};

/// Summary statistics describing how historical price action interacts with sticky zones.
#[derive(Debug, Clone, Default)]
pub struct ZoneEfficacyStats {
    /// Percentage of the configured price range covered by sticky zones (0–100).
    pub price_occupancy_pct: f64,
    /// Percentage of analyzed candles whose bodies intersect sticky zones (0–100).
    pub time_in_zones_pct: f64,
    /// Raw count of candles whose bodies intersected sticky zones.
    pub candles_in_zones: usize,
    /// Total number of candles inspected within the selected ranges.
    pub total_candles: usize,
    /// Aggregate dwell duration statistics across all sticky zones (in candles).
    pub dwell_durations: Option<DwellDurationStats>,
    /// Transition matrix summarising post-zone destinations and gaps.
    pub transitions: Vec<ZoneTransitionSummary>,
}

/// Summary of contiguous dwell durations (measured in candle counts).
#[derive(Debug, Clone, Copy, Default)]
pub struct DwellDurationStats {
    pub total_runs: usize,
    pub median_candles: f64,
    pub p90_candles: f64,
    pub max_candles: usize,
    pub runs_per_1k_candles: f64,
    pub short_run_ratio: f64,
}

/// Summary of price transitions after exiting a sticky superzone.
#[derive(Debug, Clone, Copy, Default)]
pub struct ZoneTransitionSummary {
    pub from_zone_id: usize,
    pub to_zone_id: Option<usize>,
    pub count: usize,
    pub median_gap_candles: f64,
    pub p90_gap_candles: f64,
}

#[derive(Debug, Clone, Copy)]
struct PendingTransition {
    gap_candles: usize,
}

/// Compute occupancy metrics for the provided sticky superzones against an OHLCV series.
///
/// * `price_range` is the inclusive `[min_price, max_price]` spanned by the zone model.
pub fn compute_zone_efficacy(
    timeseries: &OhlcvTimeSeries,
    sticky_superzones: &[SuperZone],
    slice_ranges: &[(usize, usize)],
    price_range: (f64, f64),
) -> Option<ZoneEfficacyStats> {
    if sticky_superzones.is_empty() {
        return None;
    }

    let available_candles = timeseries.close_prices.len();
    if available_candles == 0 {
        return None;
    }

    let (price_min, price_max) = price_range;
    if !price_min.is_finite() || !price_max.is_finite() || price_max <= price_min {
        return None;
    }

    let total_range_width = price_max - price_min;
    let aggregated_zone_width: f64 = sticky_superzones
        .iter()
        .map(|zone| (zone.price_top - zone.price_bottom).max(0.0))
        .sum();

    let price_occupancy_pct =
        ((aggregated_zone_width / total_range_width) * 100.0).clamp(0.0, 100.0);

    let mut total_candles_considered = 0usize;
    let mut candles_in_zones = 0usize;
    let mut active_runs = vec![0usize; sticky_superzones.len()];
    let mut pending_transitions: Vec<Option<PendingTransition>> =
        vec![None; sticky_superzones.len()];
    let mut run_lengths: Vec<usize> = Vec::new();
    let mut transition_records: HashMap<(usize, Option<usize>), Vec<usize>> = HashMap::new();

    let mut process_candle = |idx: usize| {
        if idx >= available_candles {
            return;
        }
        total_candles_considered += 1;

        let open = timeseries.open_prices[idx];
        let close = timeseries.close_prices[idx];
        let (body_low, body_high) = if open <= close {
            (open, close)
        } else {
            (close, open)
        };

        let mut candle_counted = false;
        let mut zones_hit_this_candle = Vec::new();

        for (zone_idx, zone) in sticky_superzones.iter().enumerate() {
            let intersects = if body_low == body_high {
                body_low >= zone.price_bottom && body_high <= zone.price_top
            } else {
                body_low <= zone.price_top && body_high >= zone.price_bottom
            };

            if intersects {
                candle_counted = true;
                active_runs[zone_idx] += 1;
                zones_hit_this_candle.push(zone_idx);
            } else if active_runs[zone_idx] > 0 {
                run_lengths.push(active_runs[zone_idx]);
                active_runs[zone_idx] = 0;
                pending_transitions[zone_idx] = Some(PendingTransition { gap_candles: 0 });
            }
        }

        if candle_counted {
            candles_in_zones += 1;
        }

        if !zones_hit_this_candle.is_empty() {
            let to_zone_id = sticky_superzones[zones_hit_this_candle[0]].id;
            for (from_idx, pending) in pending_transitions.iter_mut().enumerate() {
                if let Some(p) = pending.take() {
                    transition_records
                        .entry((sticky_superzones[from_idx].id, Some(to_zone_id)))
                        .or_default()
                        .push(p.gap_candles);
                }
            }
        }

        for pending in pending_transitions.iter_mut() {
            if let Some(p) = pending.as_mut() {
                p.gap_candles += 1;
            }
        }
    };

    if slice_ranges.is_empty() {
        for idx in 0..available_candles {
            process_candle(idx);
        }
    } else {
        for &(start, end) in slice_ranges {
            if start >= end {
                continue;
            }

            let bounded_end = end.min(available_candles);
            for idx in start..bounded_end {
                process_candle(idx);
            }
        }
    }

    if total_candles_considered == 0 {
        return None;
    }

    for (idx, run) in active_runs.into_iter().enumerate() {
        if run > 0 {
            run_lengths.push(run);
            pending_transitions[idx] = Some(PendingTransition { gap_candles: 0 });
        }
    }

    let dwell_durations = if run_lengths.is_empty() {
        None
    } else {
        run_lengths.sort_unstable();
        let total_runs = run_lengths.len();
        let max_candles = *run_lengths.last().unwrap_or(&0);
        let median_candles = percentile(&run_lengths, 0.5);
        let p90_candles = percentile(&run_lengths, 0.9);
        let short_runs = run_lengths.iter().filter(|len| **len <= 2).count();
        let runs_per_1k_candles = if total_candles_considered > 0 {
            (total_runs as f64) * 1000.0 / (total_candles_considered as f64)
        } else {
            0.0
        };
        let short_run_ratio = if total_runs > 0 {
            short_runs as f64 / total_runs as f64
        } else {
            0.0
        };

        Some(DwellDurationStats {
            total_runs,
            median_candles,
            p90_candles,
            max_candles,
            runs_per_1k_candles,
            short_run_ratio,
        })
    };

    for (idx, pending) in pending_transitions.into_iter().enumerate() {
        if let Some(p) = pending {
            transition_records
                .entry((sticky_superzones[idx].id, None))
                .or_default()
                .push(p.gap_candles);
        }
    }

    let transitions = build_transition_summaries(&transition_records);

    let time_in_zones_pct =
        (candles_in_zones as f64 / total_candles_considered as f64 * 100.0).clamp(0.0, 100.0);

    Some(ZoneEfficacyStats {
        price_occupancy_pct,
        time_in_zones_pct,
        candles_in_zones,
        total_candles: total_candles_considered,
        dwell_durations,
        transitions,
    })
}

fn percentile(sorted: &[usize], percentile: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }

    if sorted.len() == 1 {
        return sorted[0] as f64;
    }

    let clamped = percentile.clamp(0.0, 1.0);
    let max_index = (sorted.len() - 1) as f64;
    let position = clamped * max_index;
    let lower_index = position.floor() as usize;
    let upper_index = position.ceil() as usize;

    if lower_index == upper_index {
        sorted[lower_index] as f64
    } else {
        let lower_value = sorted[lower_index] as f64;
        let upper_value = sorted[upper_index] as f64;
        let weight = position - lower_index as f64;
        lower_value + (upper_value - lower_value) * weight
    }
}

fn build_transition_summaries(
    raw_records: &HashMap<(usize, Option<usize>), Vec<usize>>,
) -> Vec<ZoneTransitionSummary> {
    let mut summaries: Vec<ZoneTransitionSummary> = raw_records
        .iter()
        .map(|((from_zone_id, to_zone_id), gaps)| {
            let mut sorted_gaps = gaps.clone();
            sorted_gaps.sort_unstable();
            ZoneTransitionSummary {
                from_zone_id: *from_zone_id,
                to_zone_id: *to_zone_id,
                count: sorted_gaps.len(),
                median_gap_candles: percentile(&sorted_gaps, 0.5),
                p90_gap_candles: percentile(&sorted_gaps, 0.9),
            }
        })
        .collect();

    summaries.sort_by_key(|summary| (summary.from_zone_id, summary.to_zone_id));

    if PRINT_ZONE_TRANSITION_SUMMARY {
        #[cfg(debug_assertions)]
        {
            log::info!("--- Sticky Zone Transition Summary ---");
            for summary in &summaries {
                match summary.to_zone_id {
                    Some(to_id) => log::info!(
                        "Zone {:#04} ➜ Zone {:#04} :: count={} median_gap={:.1}c p90_gap={:.1}c",
                        summary.from_zone_id,
                        to_id,
                        summary.count,
                        summary.median_gap_candles,
                        summary.p90_gap_candles
                    ),
                    None => log::info!(
                        "Zone {:#04} ➜ (no subsequent zone) :: count={} median_gap={:.1}c p90_gap={:.1}c",
                        summary.from_zone_id,
                        summary.count,
                        summary.median_gap_candles,
                        summary.p90_gap_candles
                    ),
                }
            }
            log::info!("---------------------------------------");
        }
    }

    summaries
}
