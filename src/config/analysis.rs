//! Analysis and computation configuration

use serde::{Deserialize, Serialize};
use std::time::Duration;
use strum_macros::{Display, EnumIter};

use crate::utils::TimeUtils;

pub const DEFAULT_PH_THRESHOLD: f64 = 0.15;
pub const DEFAULT_TIME_DECAY: f64 = 1.5; // Manually synced to match 0.15 logic

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumIter)]
pub enum OptimizationGoal {
    #[strum(to_string = "Max ROI")]
    MaxROI,
    #[strum(to_string = "Max AROI")]
    MaxAROI,
    #[strum(to_string = "Balanced")]
    Balanced,
}

#[derive(Clone, Debug)]
pub struct TradeProfile {
    pub min_roi: f64,  // e.g. 0.5%
    pub min_aroi: f64, // e.g. 20.0%

    pub goal: OptimizationGoal,

    // Scoring Weights
    pub weight_roi: f64,  // e.g. 1.0
    pub weight_aroi: f64, // e.g. 0.05 (AROI is usually huge, so we dampen it)
}

// impl Default for TradeProfile {
//     fn default() -> Self {
//         Self {
//             min_roi: 0.50,
//             min_aroi: 20.0,
//             goal: OptimizationGoal::Balanced,
//             weight_roi: 1.0,
//             weight_aroi: 0.05,
//         }
//     }
// }

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

/// Settings for CVA (Cumulative Volume Analysis)
#[derive(Clone, Debug)]
pub struct CvaSettings {
    // Price change threshold (fractional) to trigger CVA recomputation
    pub price_recalc_threshold_pct: f64,
    // Minimum number of candles required for valid CVA analysis. Below this threshold, the system lacks sufficient data for reliable zone detection => error
    pub min_candles_for_analysis: usize,
    pub segment_merge_tolerance_ms: i64, // Accordion Merge Tolerance
}

/// Parameters for a specific zone type (Sticky, Reversal, etc.)
#[derive(Clone, Debug, Copy)]
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

#[derive(Clone, Debug)]
pub struct SimilaritySettings {
    pub weight_volatility: f64,
    pub weight_momentum: f64,
    pub weight_volume: f64,
    pub cutoff_score: f64, // The "100.0" filter
}

#[derive(Clone, Debug)]
pub struct ZoneClassificationConfig {
    pub sticky: ZoneParams,
    pub reversal: ZoneParams,
}

/// The Master Analysis Configuration
#[derive(Clone, Debug)]
pub struct AnalysisConfig {
    // This defines the candle interval for all analysis (1h, 5m, 15m, etc.)
    pub interval_width_ms: i64,
    // Number of price zones for analysis (actually constant rn, never updated)
    pub zone_count: usize,

    pub time_decay_factor: f64,

    // Sub-groups
    // pub journey: JourneySettings,
    pub cva: CvaSettings,
    pub zones: ZoneClassificationConfig,

    pub price_horizon: PriceHorizonConfig,

    pub journey: JourneySettings,

    pub similarity: SimilaritySettings,
}

#[derive(Clone, Debug)]
pub struct OptimalSearchSettings {
    pub scout_steps: usize,
    pub drill_top_n: usize,
    pub drill_offset_factor: f64,
    pub volatility_lookback: usize,
    pub diversity_regions: usize, // Number of regions (e.g. 10)
    pub diversity_cut_off: f64,   // % of Top Score required to qualify (e.g. 0.5 = 50%)
    pub max_results: usize,
    pub price_buffer_pct: f64,
    pub fuzzy_match_tolerance: f64, // % tolerance for merging similar trade ideas (in evolve())
}

impl OptimalSearchSettings {
    // SSOT (Const Function)
    pub const fn new() -> Self {
        Self {
            scout_steps: 30,
            drill_top_n: 5,
            drill_offset_factor: 0.25,
            volatility_lookback: 50,
            // NEW: Regional Championship Settings
            diversity_regions: 10,
            diversity_cut_off: 0.5, // Trade must be at least 50% as good as the winner
            max_results: 10, // Absolute limi on how many trades can qualify (per candle update)
            price_buffer_pct: 0.005,
            fuzzy_match_tolerance: 0.5,
        }
    }
}

// Standard Default trait just calls the const constructor
impl Default for OptimalSearchSettings {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct JourneySettings {
    pub sample_count: usize,
    pub risk_reward_tests: &'static [f64],
    pub volatility_zigzag_factor: f64, // Multiplier for "Straight Line" time (e.g. 6.0)
    pub min_journey_duration: Duration, // Floor (e.g. 1 Hour)
    pub max_journey_time: Duration,    // Ceiling (increased to 90 days)
    pub profile: TradeProfile,
    pub optimization: OptimalSearchSettings,
}

pub const ANALYSIS: AnalysisConfig = AnalysisConfig {
    interval_width_ms: TimeUtils::MS_IN_5_MIN,
    zone_count: 256,

    // 2. Derive the default automatically
    // 2. Use the Constant
    time_decay_factor: DEFAULT_TIME_DECAY,

    journey: JourneySettings {
        sample_count: 50,
        risk_reward_tests: &[1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 10.0],
        max_journey_time: Duration::from_secs(86400 * 90), // Cap at 90 Days (Quarterly). Dynamic logic will usually result in much less.
        volatility_zigzag_factor: 6.0, // Markets move 6x slower than a straight line on average
        min_journey_duration: Duration::from_secs(3600), // Floor at 1 Hour. Don't simulate 5-minute trades.

        profile: TradeProfile {
            min_roi: 0.50,  // 0.5% Minimum yield
            min_aroi: 20.0, // 20% Annualized Minimum
            goal: OptimizationGoal::Balanced,
            weight_roi: 1.0, // Scoring: We value hard cash (ROI) 20x more than theoretical speed (AROI)
            weight_aroi: 0.05,
        },

        optimization: OptimalSearchSettings::new(),
    },

    similarity: SimilaritySettings {
        weight_volatility: 10.0,
        weight_momentum: 5.0,
        weight_volume: 1.0,
        cutoff_score: 100.0,
    },

    zones: ZoneClassificationConfig {
        // STICKY ZONES (Volume Weighted)
        sticky: ZoneParams {
            smooth_pct: 0.02, // 2% smoothing makes hills out of spikes
            gap_pct: 0.01,    // 1% gap bridging merges nearby structures

            // Absolute: Bin must hold > 0.1% of Total Volume
            viability_pct: 0.001,

            // Volume is "Fat". Market structure isn't just the single highest peak;
            // it is the broad shoulders of activity around it.
            // We use a low Sigma to capture the "Bulk" of the volume profile,
            // ensuring we see the full context of where trading has occurred,
            // not just the extreme outliers.
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

    cva: CvaSettings {
        price_recalc_threshold_pct: 0.01,
        min_candles_for_analysis: 2500,
        segment_merge_tolerance_ms: TimeUtils::MS_IN_D, // Merging time segments. Set 1 Day default.
    },

    price_horizon: PriceHorizonConfig {
        threshold_pct: DEFAULT_PH_THRESHOLD,
        min_threshold_pct: 0.001, // = 0.10% minimum - seems fine for stablecoins even, let's see
        max_threshold_pct: 1.0, // 1.0 = 100% Range (From 0 to 2x Current Price. Can increase this if we want to set range higher than 2x current price).
        profiler_steps: 1000,   // With 50% range, this is 0.05% per bucket
    },
};
