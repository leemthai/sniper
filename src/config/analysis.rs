//! Analysis and computation constants (Immutable Blueprints)

use serde::{Deserialize, Serialize};
use std::time::Duration;
use strum_macros::{Display, EnumIter};

use crate::ui::config::UI_TEXT;

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

#[derive(Clone, Debug)]
pub struct TradeProfile {
    pub min_roi: f64,  
    pub min_aroi: f64, 
    pub weight_roi: f64,  
    pub weight_aroi: f64, 
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunerStation {
    pub id: StationId,
    pub name: String,
    pub target_min_hours: f64, 
    pub target_max_hours: f64,
    pub scan_ph_min: f64, 
    pub scan_ph_max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeTunerConfig {
    pub stations: Vec<TunerStation>,
}

impl TimeTunerConfig {
    pub const fn new_const() -> Self {
        Self {
            stations: Vec::new(), 
        }
    }

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