//! Analysis and computation configuration

use serde::{Deserialize, Serialize}; // Add Import

use crate::utils::TimeUtils;

pub const DEFAULT_PH_THRESHOLD: f64 = 0.15;
pub const DEFAULT_TIME_DECAY: f64 = 1.5; // Manually synced to match 0.15 logic

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

/// Settings specific to Journey Analysis
#[derive(Clone, Debug, Serialize, Deserialize)] // Add Serde
pub struct JourneySettings {
    // Tolerance when matching historical prices for journey analysis (percentage)
    pub start_price_tolerance_pct: f64,
    pub stop_loss_pct: f64,
}

/// Settings for CVA (Cumulative Volume Analysis)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CvaSettings {
    // Price change threshold (fractional) to trigger CVA recomputation
    pub price_recalc_threshold_pct: f64,
    // Minimum number of candles required for valid CVA analysis. Below this threshold, the system lacks sufficient data for reliable zone detection => error
    pub min_candles_for_analysis: usize,
    pub segment_merge_tolerance_ms: i64, // Accordion Merge Tolerance
}

/// Parameters for a specific zone type (Sticky, Reversal, etc.)
#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub struct ZoneParams {
    /// Smoothing Window % (0.0 to 1.0).
    /// Turn UP to merge jagged spikes into hills. Turn DOWN for sharp precision.
    pub smooth_pct: f64,

    /// Gap Tolerance % (0.0 to 1.0).
    /// Turn UP to bridge gaps and create larger "continents". Turn DOWN (or to 0.0) to keep islands separated.
    pub gap_pct: f64,

    // NEW: Absolute Gate.
    // A bin must contain at least this % of the total resource (Volume or Candles) to be valid.
    pub viability_pct: f64,

    // NEW: Relative Gate (Standard Deviations).
    // 0.0 = Above Average.
    // 1.0 = Significantly High.
    // 2.0 = Rare Peak.
    pub sigma: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZoneClassificationConfig {
    pub sticky: ZoneParams,
    pub reversal: ZoneParams,
}

/// The Master Analysis Configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisConfig {
    // This defines the candle interval for all analysis (1h, 5m, 15m, etc.)
    pub interval_width_ms: i64,
    // Number of price zones for analysis (actually constant rn, never updated)
    pub zone_count: usize,

    pub time_decay_factor: f64,

    // Sub-groups
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
        // Helper for linear mapping
        let remap = |val: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64| -> f64 {
            let t = (val - in_min) / (in_max - in_min);
            out_min + t * (out_max - out_min)
        };

        if ph_threshold >= 0.50 {
            // Macro Mode: Pure History
            1.0
        } else if ph_threshold >= 0.15 {
            // Transition: Swing (1.5) -> Macro (1.0)
            // As PH gets larger (0.15 -> 0.50), Decay gets smaller (1.5 -> 1.0)
            remap(ph_threshold, 0.15, 0.50, 1.5, 1.0)
        } else if ph_threshold >= 0.05 {
            // Transition: Aggressive (2.0) -> Swing (1.5)
            remap(ph_threshold, 0.05, 0.15, 2.0, 1.5)
        } else {
            // Transition: Sniper (5.0) -> Aggressive (2.0)
            // We clamp the lower PH bound at 0.0 for math safety
            remap(ph_threshold, 0.0, 0.05, 5.0, 2.0)
        }
    }
}

pub const ANALYSIS: AnalysisConfig = AnalysisConfig {
    interval_width_ms: TimeUtils::MS_IN_5_MIN,
    zone_count: 256,

    // 2. Derive the default automatically
    // 2. Use the Constant
    time_decay_factor: DEFAULT_TIME_DECAY,

    zones: ZoneClassificationConfig {
        // STICKY ZONES (Volume Weighted)
        sticky: ZoneParams {
            smooth_pct: 0.02, // 2% smoothing makes hills out of spikes
            gap_pct: 0.01,    // 1% gap bridging merges nearby structures

            // Absolute: Bin must hold > 0.1% of Total Volume
            viability_pct: 0.001,

            // Sigma 0.5 (Broad Acceptance):
            // Volume is "Fat". Market structure isn't just the single highest peak;
            // it is the broad shoulders of activity around it.
            // We use a low Sigma to capture the "Bulk" of the volume profile,
            // ensuring we see the full context of where trading has occurred,
            // not just the extreme outliers.
            // sigma: 0.5,
            sigma: 0.2, // Trying to capture zones with less amplitude e.g. PAXGUSDT at 8.122%
        },

        // REVERSAL ZONES (Wick Counts)
        reversal: ZoneParams {
            smooth_pct: 0.005, // 0.5% (Low) - Keep wicks sharp
            gap_pct: 0.0,      // 0.0% - Strict separation. Don't create ghost zones.

            viability_pct: 0.0005, //  // Absolute: Bin must be hit by > 0.05% of Total Candles (this used to be 0.05% but that was considered too constrictive)
            // Sigma 1.5 (Strict Filtering):
            // Wicks are "Sharp". Price wicks constantly due to noise.
            // A Rejection Zone is only valid if it represents a statistical anomaly
            // (a coordinated rejection at a specific level).
            // We use a high Sigma to filter out the background noise of random wicks
            // and only highlight areas with significant, repeated rejection intensity.
            sigma: 1.5,
        },
    },

    journey: JourneySettings {
        start_price_tolerance_pct: 0.5,
        // Stop-loss threshold (percentage move against position) for journey failures
        stop_loss_pct: 5.0,
    },

    cva: CvaSettings {
        price_recalc_threshold_pct: 0.01,
        min_candles_for_analysis: 100,
        segment_merge_tolerance_ms: TimeUtils::MS_IN_D, // Merging time segments. Set 1 Day default.
    },

    price_horizon: PriceHorizonConfig {
        threshold_pct: DEFAULT_PH_THRESHOLD,
        min_threshold_pct: 0.001, // = 0.10% minimum - seems fine for stablecoins even, let's see
        max_threshold_pct: 1.0, // 1.0 = 100% Range (From 0 to 2x Current Price. Can increase this if we want to set range higher than 2x current price).
        profiler_steps: 1000,   // With 50% range, this is 0.05% per bucket
    },
};
