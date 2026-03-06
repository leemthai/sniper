const TUNER_SCAN_STEPS: usize = 4;

use {
    crate::{
        app::{PhPct, Price},
        engine::run_pathfinder_simulations,
        models::{OhlcvTimeSeries, OptimizationStrategy},
        utils::AppInstant,
    },
    serde::{Deserialize, Serialize},
    std::{cmp::Ordering, fmt},
};

#[cfg(debug_assertions)]
use crate::config::DF;

#[cfg(debug_assertions)]
use crate::app::Pct;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum StationId {
    Scalp,
    Day,
    #[default]
    Swing,
    Macro,
}

impl fmt::Display for StationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.short_name())
    }
}

impl StationId {
    pub(crate) fn short_name(self) -> &'static str {
        STATIONS
            .iter()
            .find(|s| s.id == self)
            .map(|s| s.short_name)
            .unwrap_or("?")
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TunerStation {
    pub id: StationId,
    pub name: &'static str,
    pub short_name: &'static str,
    pub target_min_hours: f64,
    pub target_max_hours: f64,
    pub scan_ph_min: PhPct,
    pub scan_ph_max: PhPct,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TimeTunerConfig {
    pub stations: &'static [TunerStation],
}

pub(crate) const STATIONS: &[TunerStation] = &[
    TunerStation {
        id: StationId::Scalp,
        name: "\u{f00d5} SCALP",
        short_name: "\u{f00d5}",
        target_min_hours: 1.0,
        target_max_hours: 6.0,
        scan_ph_min: PhPct::new(0.01),
        scan_ph_max: PhPct::new(0.04),
    },
    TunerStation {
        id: StationId::Day,
        name: "\u{f522} DAY",
        short_name: "\u{f522}",
        target_min_hours: 6.0,
        target_max_hours: 24.0,
        scan_ph_min: PhPct::new(0.03),
        scan_ph_max: PhPct::new(0.08),
    },
    TunerStation {
        id: StationId::Swing,
        name: "\u{f095b} SWING",
        short_name: "\u{f095b}",
        target_min_hours: 24.0,
        target_max_hours: 120.0,
        scan_ph_min: PhPct::new(0.05),
        scan_ph_max: PhPct::new(0.15),
    },
    TunerStation {
        id: StationId::Macro,
        name: "\u{eda7} INVEST",
        short_name: "\u{eda7}",
        target_min_hours: 336.0,
        target_max_hours: 2160.0,
        scan_ph_min: PhPct::new(0.15),
        scan_ph_max: PhPct::new(0.60),
    },
];

pub const TUNER_CONFIG: TimeTunerConfig = TimeTunerConfig { stations: STATIONS };

/// Runs "Scan & Fit" algo to find the optimal Price Horizon to produce trades within the Station's target time range.
pub(crate) fn tune_to_station(
    ohlcv: &OhlcvTimeSeries,
    current_price: Price,
    station: &TunerStation,
    strategy: OptimizationStrategy,
) -> Option<PhPct> {
    struct ProbeResult {
        ph: PhPct,
        score: f64,
        duration_hours: f64,
    }

    let _t_start = AppInstant::now();
    let _pair_name = ohlcv.pair_interval.name();

    #[cfg(debug_assertions)]
    {
        if DF.log_tuner {
            log::info!(
                "📻 TUNER START [{}]: Station '{}' (Target: {:.1}-{:.1}h) | Scan Range: {}-{} | Strategy: {}",
                _pair_name,
                station.name,
                station.target_min_hours,
                station.target_max_hours,
                station.scan_ph_min,
                station.scan_ph_max,
                strategy
            );
        }
    }

    let steps = TUNER_SCAN_STEPS;
    let mut scan_points = Vec::with_capacity(steps);
    if steps > 1 {
        let step_size =
            (station.scan_ph_max.value() - station.scan_ph_min.value()) / (steps - 1) as f64;
        for i in 0..steps {
            scan_points.push(station.scan_ph_min.value() + (i as f64 * step_size));
        }
    } else {
        scan_points.push(station.scan_ph_min.value()); // Fallback
    }

    let mut results: Vec<ProbeResult> = Vec::new();
    for &ph in &scan_points {
        let result = run_pathfinder_simulations(
            ohlcv,
            current_price,
            PhPct::new(ph),
            strategy,
            station.id,
            None,
        );

        let count = result.opportunities.len();
        if count > 0 {
            let duration_hours = result
                .opportunities
                .iter()
                .map(|o| o.avg_duration.value())
                .sum::<i64>() as f64
                / count as f64
                / 3_600_000.0;

            let top_score = result.opportunities[0].calc_quality_score();

            results.push(ProbeResult {
                ph: PhPct::new(ph),
                score: top_score,
                duration_hours,
                // count,
            });

            #[cfg(debug_assertions)]
            if DF.log_tuner {
                log::info!(
                    "   📡 TUNER PROBE {}: Found {} ops | Top Score {:.2} | Avg Dur {:.1}h for {}",
                    Pct::new(ph),
                    count,
                    top_score,
                    duration_hours,
                    _pair_name,
                );
            }
        } else {
            #[cfg(debug_assertions)]
            if DF.log_tuner {
                log::info!(
                    "   📡 TUNER PROBE {}: No signals found (0 candidates) for {}",
                    Pct::new(ph),
                    _pair_name,
                );
            }
        }
    }

    if results.is_empty() {
        #[cfg(debug_assertions)]
        if DF.log_tuner {
            log::warn!(
                "⚠️ TUNER FAILED: No candidates found across entire range for {}",
                _pair_name
            );
        }
        return None;
    }

    let valid_fits: Vec<&ProbeResult> = results
        .iter()
        .filter(|r| {
            r.duration_hours >= station.target_min_hours
                && r.duration_hours <= station.target_max_hours
        })
        .collect();

    let best_match = if !valid_fits.is_empty() {
        valid_fits
            .into_iter()
            .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(Ordering::Equal))
            .unwrap()
    } else {
        #[cfg(debug_assertions)]
        if DF.log_tuner {
            log::warn!(
                "   ⚠️ No perfect time fit. Falling back to closest duration for {}",
                _pair_name
            );
        }
        let target_center = (station.target_min_hours + station.target_max_hours) / 2.0;

        results
            .iter()
            .min_by(|a, b| {
                let dist_a = (a.duration_hours - target_center).abs();
                let dist_b = (b.duration_hours - target_center).abs();
                dist_a.partial_cmp(&dist_b).unwrap_or(Ordering::Equal)
            })
            .unwrap()
    };

    #[cfg(debug_assertions)]
    {
        let elapsed = _t_start.elapsed();
        if DF.log_tuner {
            log::info!(
                "✅ TUNER LOCKED: {} (Score {:.2}, Duration {:.1}h) | Took {:?} for {}",
                best_match.ph,
                best_match.score,
                best_match.duration_hours,
                elapsed,
                _pair_name,
            );
        }
    }

    Some(best_match.ph)
}
