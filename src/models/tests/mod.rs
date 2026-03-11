//! Unit tests for pure model functions.
//! Lives in a separate file — no test code in production source files.

use crate::{
    app::{
        AroiPct, DurationMs, HighPrice, JourneySettings, LowPrice, OptimalSearchSettings, Pct,
        PhPct, RoiPct, TradeProfile, VolatilityPct,
    },
    models::{AdaptiveParameters, CVACore, ScoreType, trading_model::find_target_zones},
};
use std::time::Duration;

// ─── helpers ────────────────────────────────────────────────────────────────

/// Build a minimal CVACore spanning [min_price, max_price] with `zones` bins.
fn make_core(min: f64, max: f64, zones: usize) -> CVACore {
    CVACore::new(
        LowPrice::new(min),
        HighPrice::new(max),
        zones,
        "TEST".into(),
        1.0,
        100,
        100,
        60_000,
        VolatilityPct::new(0.01),
    )
}

// ─── find_target_zones ───────────────────────────────────────────────────────

#[test]
fn ftz_empty_when_nothing_above_threshold() {
    let scores = vec![0.1, 0.2, 0.3];
    let result = find_target_zones(&scores, 0.5, 1);
    assert!(result.is_empty());
}

#[test]
fn ftz_single_island_contiguous() {
    // indices 2, 3, 4 are above threshold
    let scores = vec![0.0, 0.0, 1.0, 1.0, 1.0, 0.0];
    let result = find_target_zones(&scores, 0.5, 0);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].start_idx, 2);
    assert_eq!(result[0].end_idx, 4);
}

#[test]
fn ftz_two_islands_gap_too_wide() {
    // indices 1 and 5 are above threshold; gap = 3 bins, max_gap = 1 → two islands
    let scores = vec![0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    let result = find_target_zones(&scores, 0.5, 1);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].start_idx, 1);
    assert_eq!(result[0].end_idx, 1);
    assert_eq!(result[1].start_idx, 5);
    assert_eq!(result[1].end_idx, 5);
}

#[test]
fn ftz_gap_bridged_within_max_gap() {
    // indices 1 and 3; gap = 1 bin, max_gap = 1 → single island
    let scores = vec![0.0, 1.0, 0.0, 1.0, 0.0];
    let result = find_target_zones(&scores, 0.5, 1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].start_idx, 1);
    assert_eq!(result[0].end_idx, 3);
}

// ─── CVACore::distribute_conserved_volume ────────────────────────────────────

#[test]
fn dcv_total_score_conserved_full_range() {
    let zones = 10;
    let mut core = make_core(0.0, 100.0, zones);
    let total = 50.0;
    core.distribute_conserved_volume(
        ScoreType::FullCandleTVW,
        crate::app::Price::new(0.0),
        crate::app::Price::new(100.0),
        total,
    );
    let sum: f64 = core.candle_bodies_vw.iter().sum();
    assert!((sum - total).abs() < 1e-10, "sum={sum} expected={total}");
}

#[test]
fn dcv_total_score_conserved_partial_range() {
    let zones = 10;
    let mut core = make_core(0.0, 100.0, zones);
    // Spread across the middle half of the range
    let total = 30.0;
    core.distribute_conserved_volume(
        ScoreType::FullCandleTVW,
        crate::app::Price::new(25.0),
        crate::app::Price::new(75.0),
        total,
    );
    let sum: f64 = core.candle_bodies_vw.iter().sum();
    assert!((sum - total).abs() < 1e-10, "sum={sum} expected={total}");
}

#[test]
fn dcv_score_is_uniform_across_bins() {
    let zones = 4;
    let mut core = make_core(0.0, 40.0, zones);
    let total = 4.0;
    core.distribute_conserved_volume(
        ScoreType::FullCandleTVW,
        crate::app::Price::new(0.0),
        crate::app::Price::new(40.0),
        total,
    );
    // Each of the 4 bins should get exactly 1.0
    for (i, &v) in core.candle_bodies_vw.iter().enumerate() {
        assert!((v - 1.0).abs() < 1e-10, "bin {i} = {v}, expected 1.0");
    }
}

// ─── CVACore::apply_rejection_impact ─────────────────────────────────────────

#[test]
fn ari_all_covered_bins_get_full_score() {
    let zones = 5;
    let mut core = make_core(0.0, 50.0, zones);
    let score = 7.0;
    // Cover full range — all 5 bins should each receive the full score
    core.apply_rejection_impact(
        ScoreType::LowWickCount,
        crate::app::Price::new(0.0),
        crate::app::Price::new(50.0),
        score,
    );
    for (i, &v) in core.low_wick_counts.iter().enumerate() {
        assert!((v - score).abs() < 1e-10, "bin {i} = {v}, expected {score}");
    }
}

#[test]
fn ari_partial_range_only_touches_covered_bins() {
    let zones = 10;
    let mut core = make_core(0.0, 100.0, zones);
    // Each bin = 10 units wide; cover bins 2-4 (prices 20-50)
    core.apply_rejection_impact(
        ScoreType::LowWickCount,
        crate::app::Price::new(20.0),
        crate::app::Price::new(50.0),
        5.0,
    );
    // count_intersecting_chunks uses floor for both endpoints:
    //   floor((20-0)/10) = 2,  floor((50-0)/10) = 5  → bins 2,3,4,5 touched
    // Bins 0-1 and 6-9 must be zero
    for i in [0usize, 1, 6, 7, 8, 9] {
        assert_eq!(core.low_wick_counts[i], 0.0, "bin {i} should be untouched");
    }
    // Bins 2-5 must each hold the full score
    for i in 2..=5 {
        assert!(
            (core.low_wick_counts[i] - 5.0).abs() < 1e-10,
            "bin {i} = {} expected 5.0",
            core.low_wick_counts[i]
        );
    }
}

#[test]
fn ari_score_not_diluted_unlike_conserved_volume() {
    let zones = 10;
    let mut rej = make_core(0.0, 100.0, zones);
    let mut vol = make_core(0.0, 100.0, zones);
    let score = 10.0;

    rej.apply_rejection_impact(
        ScoreType::LowWickCount,
        crate::app::Price::new(0.0),
        crate::app::Price::new(100.0),
        score,
    );
    vol.distribute_conserved_volume(
        ScoreType::FullCandleTVW,
        crate::app::Price::new(0.0),
        crate::app::Price::new(100.0),
        score,
    );

    // rejection: every bin = 10.0; conserved volume: every bin = 1.0
    let rej_sum: f64 = rej.low_wick_counts.iter().sum();
    let vol_sum: f64 = vol.candle_bodies_vw.iter().sum();
    assert!((rej_sum - score * zones as f64).abs() < 1e-10);
    assert!((vol_sum - score).abs() < 1e-10);
}

// ─── AdaptiveParameters::calc_trend_lookback_candles ─────────────────────────
//
// BASE_INTERVAL = 5 min → interval_ms = 300_000
// day_candles  = 86400 / 300  =  288
// week_candles = 288 * 7      = 2016
// month_candles= 288 * 30     = 8640
//
// Segment boundaries (ph_pct):
//   [0.005, 0.05)  → remap to [24, 288]
//   [0.05,  0.15)  → remap to [288, 2016]
//   [0.15,  0.50]  → remap to [2016, 8640]

/// Helper: expected candle count via the same remap logic as the source.
fn expected_lookback(v: f64) -> usize {
    let day_candles = 288.0_f64;
    let week_candles = 2016.0_f64;
    let month_candles = 8640.0_f64;
    let remap = |val: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64| -> f64 {
        let t = (val - in_min) / (in_max - in_min);
        out_min + t * (out_max - out_min)
    };
    let result = if v < 0.05 {
        remap(v, 0.005, 0.05, 24.0, day_candles)
    } else if v < 0.15 {
        remap(v, 0.05, 0.15, day_candles, week_candles)
    } else {
        remap(v, 0.15, 0.50, week_candles, month_candles)
    };
    result.round() as usize
}

#[test]
fn ctlc_scalp_lower_bound_gives_minimum_lookback() {
    // v = 0.005 → remap floor → should be 24 candles
    let result = AdaptiveParameters::calc_trend_lookback_candles(PhPct::new(0.005));
    assert_eq!(result, 24, "at v=0.005 expected 24, got {result}");
}

#[test]
fn ctlc_scalp_upper_boundary_gives_day_candles() {
    // v just below 0.05 sits in the scalp segment; v = 0.05 crosses into swing
    // At exactly 0.05 the code uses the swing branch: remap(0.05, 0.05, 0.15, 288, 2016) = 288
    let result = AdaptiveParameters::calc_trend_lookback_candles(PhPct::new(0.05));
    assert_eq!(result, expected_lookback(0.05));
}

#[test]
fn ctlc_swing_midpoint_is_between_day_and_week() {
    // v = 0.10 (midpoint of [0.05, 0.15]) → midpoint of [288, 2016] = 1152
    let result = AdaptiveParameters::calc_trend_lookback_candles(PhPct::new(0.10));
    assert_eq!(result, expected_lookback(0.10));
}

#[test]
fn ctlc_macro_boundary_gives_week_candles() {
    // v = 0.15 enters the macro branch → remap(0.15, 0.15, 0.50, 2016, 8640) = 2016
    let result = AdaptiveParameters::calc_trend_lookback_candles(PhPct::new(0.15));
    assert_eq!(result, expected_lookback(0.15));
}

#[test]
fn ctlc_macro_upper_bound_gives_month_candles() {
    // v = 0.50 → remap ceiling → 8640 candles
    let result = AdaptiveParameters::calc_trend_lookback_candles(PhPct::new(0.50));
    assert_eq!(result, 8640, "at v=0.50 expected 8640, got {result}");
}

// ─── AdaptiveParameters::calc_dynamic_journey_duration ───────────────────────
//
// Formula (before clamping): candles = (ratio + 3)^2,  ratio = ph_pct / vol_pct
// total_ms = candles * interval_ms
//
// Tests use a wide-clamp JourneySettings so the formula is never clamped,
// letting us verify raw arithmetic directly.

fn unclamped_journey() -> JourneySettings {
    JourneySettings {
        sample_count: 50,
        risk_reward_tests: &[],
        min_journey_time: Duration::ZERO,
        max_journey_time: Duration::from_secs(86400 * 365 * 10), // 10 years — effectively infinite
        profile: TradeProfile {
            min_roi_pct: RoiPct::new(0.0),
            min_aroi_pct: AroiPct::new(0.0),
        },
        optimization: OptimalSearchSettings {
            volatility_lookback: 50,
            scout_steps: 10,
            price_buffer_pct: PhPct::new(0.0),
            drill_offset_factor: 1.0,
            drill_cutoff_pct: PhPct::new(0.0),
            drill_top_n: 5,
            fuzzy_match_tolerance: Pct::new(0.0),
            diversity_regions: 3,
            diversity_cut_off: PhPct::new(0.5),
            max_results: 10,
            prune_interval_sec: 60,
        },
    }
}

/// Expected duration (ms) from the raw formula, no clamping.
fn expected_duration_ms(ph_pct: f64, vol_pct: f64, interval_ms: i64) -> u64 {
    let ratio = ph_pct / vol_pct.max(VolatilityPct::MIN_EPSILON);
    let candles = (ratio + 3.0).powi(2);
    (candles * interval_ms as f64) as u64
}

#[test]
fn cdjd_ratio_one_gives_sixteen_candles() {
    // ratio = 1.0  →  (1+3)^2 = 16 candles
    let interval_ms = 300_000_i64; // 5 min
    let journey = unclamped_journey();
    let result = AdaptiveParameters::calc_dynamic_journey_duration(
        PhPct::new(0.05),
        VolatilityPct::new(0.05),
        DurationMs::new(interval_ms),
        &journey,
    );
    let expected_ms = 16 * interval_ms as u64;
    assert_eq!(
        result.as_millis() as u64,
        expected_ms,
        "ratio=1 → 16 candles → {expected_ms}ms, got {}ms",
        result.as_millis()
    );
}

#[test]
fn cdjd_ratio_zero_gives_nine_candles() {
    // ratio → 0  (ph tiny, vol large)  →  (0+3)^2 = 9 candles
    let interval_ms = 300_000_i64;
    let journey = unclamped_journey();
    let result = AdaptiveParameters::calc_dynamic_journey_duration(
        PhPct::new(0.001),
        VolatilityPct::new(1.0), // vol >> ph → ratio ≈ 0.001
        DurationMs::new(interval_ms),
        &journey,
    );
    // ratio = 0.001/1.0 = 0.001, candles = (0.001+3)^2 ≈ 9.006, truncated to 9 * 300_000
    let expected_ms = expected_duration_ms(0.001, 1.0, interval_ms);
    assert_eq!(result.as_millis() as u64, expected_ms);
}

#[test]
fn cdjd_scales_with_interval() {
    // Same ratio, different interval → result scales linearly
    let ph = PhPct::new(0.10);
    let vol = VolatilityPct::new(0.10); // ratio = 1.0 → 16 candles
    let journey = unclamped_journey();

    let r5m = AdaptiveParameters::calc_dynamic_journey_duration(
        ph,
        vol,
        DurationMs::new(300_000),
        &journey,
    );
    let r15m = AdaptiveParameters::calc_dynamic_journey_duration(
        ph,
        vol,
        DurationMs::new(900_000),
        &journey,
    );
    // 15-min candles are 3× wider → duration should be 3× longer
    assert_eq!(
        r15m.as_millis(),
        r5m.as_millis() * 3,
        "15m interval should give 3× the duration of 5m"
    );
}

#[test]
fn cdjd_clamp_enforces_minimum() {
    // Tiny ph + tiny vol → formula produces a very short duration;
    // the real DEFAULT_JOURNEY_SETTINGS clamp (min = 50 min) must apply.
    use crate::models::DEFAULT_JOURNEY_SETTINGS;
    let result = AdaptiveParameters::calc_dynamic_journey_duration(
        PhPct::new(0.001),
        VolatilityPct::new(0.001),
        DurationMs::new(300_000),
        &DEFAULT_JOURNEY_SETTINGS,
    );
    assert!(
        result >= DEFAULT_JOURNEY_SETTINGS.min_journey_time,
        "result {:?} must be >= min {:?}",
        result,
        DEFAULT_JOURNEY_SETTINGS.min_journey_time
    );
}

// #[test]
// fn fail_please() {
//     let condition = true;
//     assert!(!condition, "The condition is true");
// }
