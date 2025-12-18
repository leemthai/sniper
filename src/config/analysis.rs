//! Analysis and computation configuration

use serde::{Deserialize, Serialize}; // Add Import

use crate::utils::TimeUtils;

pub const DEFAULT_PH_THRESHOLD: f64 = 0.15;
pub const DEFAULT_TIME_DECAY: f64 = 1.5; // Manually synced to match 0.15 logic

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualityZone {
    pub max_count: usize,        // Upper bound (e.g. 100)
    pub label: String,           // "No Res"
    pub color_rgb: (u8, u8, u8), // (255, 100, 100)

    #[serde(default)]
    pub description: String,
}

/// Configuration for the Price Horizon.
/// Determines the vertical price range of interest relative to the current price.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceHorizonConfig {
    /// Percentage threshold for price relevancy (e.g. 0.15 = 15%)
    pub threshold_pct: f64,

    // UI Bounds
    pub min_threshold_pct: f64,
    pub max_threshold_pct: f64,

    // Configurable Resolution
    pub profiler_steps: usize,
}

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

impl AnalysisConfig {
    /// The Source of Truth for the "Adaptive Decay" curve.
    /// Maps Price Horizon % -> Time Decay Factor.
    /// Used by both Config Initialization and UI Runtime updates.
    pub fn calculate_time_decay(ph_threshold: f64) -> f64 {
        if ph_threshold >= 0.50 {
            1.0 // Macro
        } else if ph_threshold >= 0.15 {
            1.5 // Swing
        } else if ph_threshold >= 0.05 {
            2.0 // Aggressive
        } else {
            5.0 // Sniper
        }
    }

    pub fn get_quality_zones() -> Vec<QualityZone> {
        vec![
            QualityZone {
                max_count: 100,
                label: "No-Res".to_string(),
                color_rgb: (200, 50, 50),
                description: "Insufficient Data (Noise)".to_string(),
            },
            QualityZone {
                max_count: 1000,
                label: "Low-Res".to_string(),
                color_rgb: (200, 200, 50),
                description: "Low Definition (Scalp)".to_string(),
            },
            QualityZone {
                max_count: 10000,
                label: "Med-Res".to_string(),
                color_rgb: (50, 200, 50),
                description: "Medium Definition (Swing)".to_string(),
            },
            QualityZone {
                max_count: 100000,
                label: "Hi-Res".to_string(),
                color_rgb: (50, 200, 255),
                description: "High Definition (Macro)".to_string(),
            },
            QualityZone {
                max_count: usize::MAX,
                label: "Ultra-Res".to_string(),
                color_rgb: (200, 50, 255),
                description: "Deep History".to_string(),
            },
        ]
    }
}

pub const ANALYSIS: AnalysisConfig = AnalysisConfig {
    interval_width_ms: TimeUtils::MS_IN_5_MIN,
    zone_count: 256, // Goldilocks number (see private project-3eed40f.md for explanation)

    // 2. Derive the default automatically
    // 2. Use the Constant
    time_decay_factor: DEFAULT_TIME_DECAY,

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

    // NEW: Initialize Default PriceHorizon
    price_horizon: PriceHorizonConfig {
        threshold_pct: DEFAULT_PH_THRESHOLD,
        min_threshold_pct: 0.001,
        max_threshold_pct: 1.0, // 1.0 = 100% Range (From 0 to 2x Current Price. Can increase this if we want to set range higher than 2x current price).
        profiler_steps: 1000,   // With 50% range, this is 0.05% per bucket
    },
};
