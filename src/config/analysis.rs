//! Analysis and computation configuration

use serde::{Deserialize, Serialize}; // Add Import

use crate::{
    domain::price_horizon::PriceHorizonConfig,
    utils::TimeUtils,
};

/// Configuration for the Time Horizon UI Slider
#[derive(Clone, Debug, Serialize, Deserialize)] // Add Serde
pub struct TimeHorizonConfig {
    // Time Horizon slider configuration
    pub min_days: u64,
    pub max_days: u64,
    pub default_days: u64,
}

/// Settings specific to Journey Analysis
#[derive(Clone, Debug, Serialize, Deserialize)] // Add Serde
pub struct JourneySettings {
    // Tolerance when matching historical prices for journey analysis (percentage)
    pub start_price_tolerance_pct: f64,
    pub stop_loss_pct: f64,
}

/// Settings for CVA (Cumulative Volume Analysis)
#[derive(Clone, Debug, Serialize, Deserialize)] // Add Serde
pub struct CvaSettings {
    // Price change threshold (fractional) to trigger CVA recomputation
    pub price_recalc_threshold_pct: f64,
    // Minimum number of candles required for valid CVA analysis
    // Below this threshold, the system lacks sufficient data for reliable zone detection
    pub min_candles_for_analysis: usize,
}

/// Parameters for a specific zone type (Sticky, Reversal, etc.)
#[derive(Clone, Debug, Copy, Serialize, Deserialize)] // Add Serde
pub struct ZoneParams {
    /// Smoothing Window % (0.0 to 1.0).
    /// Turn UP to merge jagged spikes into hills. Turn DOWN for sharp precision.
    pub smooth_pct: f64,

    /// Gap Tolerance % (0.0 to 1.0).
    /// Turn UP to bridge gaps and create larger "continents". Turn DOWN (or to 0.0) to keep islands separated.
    pub gap_pct: f64,

    /// Intensity Threshold.
    /// Turn UP to reduce coverage (only show strong zones). Turn DOWN to see fainter zones.
    pub threshold: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)] // Add Serde
pub struct ZoneClassificationConfig {
    pub sticky: ZoneParams,
    pub reversal: ZoneParams,
}

/// The Master Analysis Configuration
#[derive(Clone, Debug, Serialize, Deserialize)] // Add Serde
pub struct AnalysisConfig {
    // This defines the candle interval for all analysis (1h, 5m, 15m, etc.)
    pub interval_width_ms: i64,
    // Number of price zones for analysis (actually constant rn, never updated)
    pub zone_count: usize,

    pub time_decay_factor: f64,

    // Sub-groups
    pub time_horizon: TimeHorizonConfig,
    pub journey: JourneySettings,
    pub cva: CvaSettings,
    pub zones: ZoneClassificationConfig,

    pub price_horizon: PriceHorizonConfig,
}

pub const ANALYSIS: AnalysisConfig = AnalysisConfig {
    interval_width_ms: TimeUtils::MS_IN_30_MIN,
    zone_count: 256, // Goldilocks number (see private project-3eed40f.md for explanation)
    time_decay_factor: 1.0,

    zones: ZoneClassificationConfig {
        // STICKY ZONES (Volume Weighted)
        sticky: ZoneParams {
            smooth_pct: 0.02, // 2% smoothing makes hills out of spikes
            gap_pct: 0.01,    // 1% gap bridging merges nearby structures
            threshold: 0.25,  // (Squared). Only top 50% volume areas qualify.
        },

        // REVERSAL ZONES (Wick Counts)
        reversal: ZoneParams {
            smooth_pct: 0.005, // 0.5% (Low) - Keep wicks sharp
            gap_pct: 0.0,      // 0.0% - Strict separation. Don't create ghost zones.

            // THRESHOLD TUNING GUIDE:
            // 0.000400 = Requires ~2.0% Wick Density (Very Strict, few zones)
            // 0.000100 = Requires ~1.0% Wick Density
            // 0.000025 = Requires ~0.5% Wick Density
            // 0.000010 = Requires ~0.3% Wick Density (Noisier)
            threshold: 0.000025, // Defaulting to 0.5% based on your "too much coverage" feedback
        },
    },

    time_horizon: TimeHorizonConfig {
        min_days: 1,
        max_days: 100,
        default_days: 7,
    },

    journey: JourneySettings {
        start_price_tolerance_pct: 0.5,
        // Stop-loss threshold (percentage move against position) for journey failures
        stop_loss_pct: 5.0,
    },

    cva: CvaSettings {
        // CHANGE: 0.01 (1%) -> 0.0005 (0.05%)
        // This makes the model 20x more sensitive for testing.
        // TESTING ONLY CHANGE .... change back when not testing to 0.01
        // price_recalc_threshold_pct: 0.000003,
        price_recalc_threshold_pct: 0.01,
        min_candles_for_analysis: 100,
    },

    // NEW: Initialize Default AutoDuration
    price_horizon: PriceHorizonConfig {
        threshold_pct: 0.15,
        min_lookback_days: 7,
    },
};
