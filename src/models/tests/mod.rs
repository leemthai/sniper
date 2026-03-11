//! Unit tests for pure model functions.
//! Lives in a separate file — no test code in production source files.

use crate::{
    app::{HighPrice, LowPrice, VolatilityPct},
    models::{CVACore, ScoreType, trading_model::find_target_zones},
};

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
