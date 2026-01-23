//! Analysis and computation constants (Immutable Blueprints)

use serde::{Deserialize, Serialize};
use std::time::Duration;
use strum_macros::{Display, EnumIter};

use crate::utils::TimeUtils;
use crate::ui::config::UI_TEXT;

// Global Defaults
// pub const DEFAULT_PH_THRESHOLD: f64 = 0.15;

// --- ENUMS (Definitions) ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumIter)]
pub enum OptimizationGoal {
    #[strum(to_string = "Max ROI")]
    MaxROI,
    #[strum(to_string = "Max AROI")]
    MaxAROI,
    #[strum(to_string = "Balanced")]
    Balanced,
}

impl Default for OptimizationGoal {
    fn default() -> Self {
        Self::Balanced // The sensible middle ground
    }
}

impl OptimizationGoal {
    pub fn icon(&self) -> String {
        match self {
            OptimizationGoal::MaxROI => UI_TEXT.icon_strategy_roi.to_string(),
            OptimizationGoal::MaxAROI => UI_TEXT.icon_strategy_aroi.to_string(),
            OptimizationGoal::Balanced => UI_TEXT.icon_strategy_balanced.to_string(),
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StationId {
    Scalp,
    Day,
    #[default]
    Swing,
    Macro,
}

// --- STRUCTS (Constants) ---

#[derive(Clone, Debug)]
pub struct TradeProfile {
    // Scoring Weights & Thresholds (Constant)
    pub min_roi: f64,  
    pub min_aroi: f64, 
    // REMOVED: goal (Strategy is a Runtime Variable)
    pub weight_roi: f64,  
    pub weight_aroi: f64, 
}

/// Settings for CVA (Cumulative Volume Analysis)
#[derive(Clone, Debug)]
pub struct CvaSettings {
    pub price_recalc_threshold_pct: f64,
    pub min_candles_for_analysis: usize,
    pub segment_merge_tolerance_ms: i64, 
}

#[derive(Clone, Debug, Copy)]
pub struct ZoneParams {
    pub smooth_pct: f64,
    pub gap_pct: f64,
    pub viability_pct: f64,
    pub sigma: f64,
}

#[derive(Clone, Debug)]
pub struct SimilaritySettings {
    pub weight_volatility: f64,
    pub weight_momentum: f64,
    pub weight_volume: f64,
    pub cutoff_score: f64, 
}

#[derive(Clone, Debug)]
pub struct ZoneClassificationConfig {
    pub sticky: ZoneParams,
    pub reversal: ZoneParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunerStation {
    pub id: StationId,
    pub name: String,
    // The Goal: User wants trades lasting this long
    pub target_min_hours: f64, 
    pub target_max_hours: f64,
    // The Engine Hint: Where should we look for these?
    pub scan_ph_min: f64, 
    pub scan_ph_max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeTunerConfig {
    pub stations: Vec<TunerStation>,
}

impl TimeTunerConfig {
    // 1. Const Constructor (Empty)
    pub const fn new_const() -> Self {
        Self {
            stations: Vec::new(), 
        }
    }

    // 2. Factory Defaults (Hydration)
    pub fn standard_defaults() -> Self {
        Self {
            stations: vec![
                TunerStation {
                    id: StationId::Scalp,
                    name: "⚡ SCALP".to_string(),
                    target_min_hours: 1.0,
                    target_max_hours: 6.0,
                    scan_ph_min: 0.01,
                    scan_ph_max: 0.04,
                },
                TunerStation {
                    id: StationId::Day,
                    name: "☀️ DAY".to_string(),
                    target_min_hours: 6.0,
                    target_max_hours: 24.0,
                    scan_ph_min: 0.03,
                    scan_ph_max: 0.08,
                },
                TunerStation {
                    id: StationId::Swing,
                    name: "🌊 SWING".to_string(),
                    target_min_hours: 24.0,
                    target_max_hours: 120.0,
                    scan_ph_min: 0.05,
                    scan_ph_max: 0.15,
                },
                TunerStation {
                    id: StationId::Macro,
                    name: "🏔️ MACRO".to_string(),
                    target_min_hours: 336.0,
                    target_max_hours: 2160.0,
                    scan_ph_min: 0.15,
                    scan_ph_max: 0.60,
                },
            ],
        }
    }
}

#[derive(Clone, Debug)]
pub struct OptimalSearchSettings {
    pub scout_steps: usize,
    pub drill_top_n: usize,
    pub drill_offset_factor: f64,
    pub drill_cutoff_pct: f64, 
    pub volatility_lookback: usize,
    pub diversity_regions: usize, 
    pub diversity_cut_off: f64,   
    pub max_results: usize,
    pub price_buffer_pct: f64,
    pub fuzzy_match_tolerance: f64, 
    pub prune_interval_sec: u64, 
}

impl OptimalSearchSettings {
    pub const fn new() -> Self {
        Self {
            scout_steps: 20, 
            drill_top_n: 5,
            drill_offset_factor: 0.25,
            drill_cutoff_pct: 0.70,
            volatility_lookback: 50,
            diversity_regions: 5, 
            diversity_cut_off: 0.5, 
            max_results: 5, 
            price_buffer_pct: 0.005,
            fuzzy_match_tolerance: 0.5,
            prune_interval_sec: 10,
        }
    }
}

impl Default for OptimalSearchSettings {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct JourneySettings {
    pub sample_count: usize,
    pub risk_reward_tests: &'static [f64],
    pub volatility_zigzag_factor: f64, 
    pub min_journey_duration: Duration, 
    pub max_journey_time: Duration,    
    pub profile: TradeProfile,
    pub optimization: OptimalSearchSettings,
}

/// The Master Analysis Constants (Immutable Blueprint)
#[derive(Clone, Debug)]
pub struct AppConstants {
    pub interval_width_ms: i64,
    pub zone_count: usize,
    pub time_decay_factor: f64,
    pub tuner_scan_steps: usize,

    pub cva: CvaSettings,
    pub zones: ZoneClassificationConfig,
    
    pub journey: JourneySettings,
    pub similarity: SimilaritySettings,
}

// THE STATIC INSTANCE
pub const CONSTANTS: AppConstants = AppConstants {

    interval_width_ms: TimeUtils::MS_IN_5_MIN,
    zone_count: 256,
    time_decay_factor: 1.5,
    tuner_scan_steps: 4,

    journey: JourneySettings {
        sample_count: 50,
        risk_reward_tests: &[1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 10.0],
        max_journey_time: Duration::from_secs(86400 * 90), 
        volatility_zigzag_factor: 6.0, 
        min_journey_duration: Duration::from_secs(3600), 

        profile: TradeProfile {
            min_roi: 0.10,  
            min_aroi: 20.0, 
            weight_roi: 1.0, 
            weight_aroi: 0.002,
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
        sticky: ZoneParams {
            smooth_pct: 0.02, 
            gap_pct: 0.01,    
            viability_pct: 0.001,
            sigma: 0.2, 
        },
        reversal: ZoneParams {
            smooth_pct: 0.005, 
            gap_pct: 0.0,      
            viability_pct: 0.0005, 
            sigma: 1.5,
        },
    },

    cva: CvaSettings {
        price_recalc_threshold_pct: 0.01,
        min_candles_for_analysis: 500,
        segment_merge_tolerance_ms: TimeUtils::MS_IN_D, 
    },
};

//Impl Default to hydrate the Vectors
impl Default for AppConstants {
    fn default() -> Self {
        // Clone the const primitives
        CONSTANTS // TEMP is this how you do it? lol
        // Hydrate the Tuner Definitions
        // c.tuner = TimeTunerConfig::standard_defaults();
        // c
    }
}