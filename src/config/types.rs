//! Analysis and computation constants (Immutable Blueprints)

use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::time::Duration;
use strum_macros::{Display, EnumIter};

use crate::ui::config::UI_TEXT;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PhPct(pub f64);

impl PhPct {

    pub const DEFAULT_VALUE: f64 = 0.15;
    pub const DEFAULT: Self = Self(Self::DEFAULT_VALUE);

    pub fn new(val: f64) -> Self {
        debug_assert!(val >= 0.0 && val <= 1.0, "PhPct value {} out of logical range [0, 1]", val);
        Self(val.clamp(0.0, 1.0))
    }

    pub fn format_pct(&self) -> String {
        format!("{:.2}%", self.0 * 100.0)
    }
}

impl Default for PhPct {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Deref for PhPct {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for PhPct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.4}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct VolatilityPct(pub f64);

impl VolatilityPct {

    pub const MIN_EPSILON: f64 = 0.0001;

    pub fn new(val: f64) -> Self {
        debug_assert!(val >= 0.0, "Volatility cannot be negative: {}", val);
        debug_assert!(val < 10.0, "Insane volatility detected (>1000%): {}. Possible unit error?", val);
        Self(val.max(0.0))
    }

    pub fn as_safe_divisor(&self) -> f64 {
        self.0.max(Self::MIN_EPSILON)
    }
    
}

impl Deref for VolatilityPct {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for VolatilityPct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.3}%", self.0 * 100.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct MomentumPct(pub f64);

impl MomentumPct {

    pub fn new(val: f64) -> Self {
        debug_assert!(val.is_finite(), "Momentum must be a finite number");
        debug_assert!(val.abs() < 10.0, "Insane momentum detected: {}. Check math.", val);
        Self(val)
    }
}

impl Deref for MomentumPct {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for MomentumPct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:+.2}%", self.0 * 100.0)
    }
}

// --- ENUMS (Definitions) ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumIter)]
pub enum OptimizationStrategy {
    #[strum(to_string = "Max ROI")]
    MaxROI,
    #[strum(to_string = "Max AROI")]
    MaxAROI,
    #[strum(to_string = "Balanced")]
    Balanced,
}

impl Default for OptimizationStrategy {
    fn default() -> Self {
        Self::Balanced // The sensible middle ground
    }
}

impl OptimizationStrategy {
    pub fn icon(&self) -> String {
        match self {
            OptimizationStrategy::MaxROI => UI_TEXT.icon_strategy_roi.to_string(),
            OptimizationStrategy::MaxAROI => UI_TEXT.icon_strategy_aroi.to_string(),
            OptimizationStrategy::Balanced => UI_TEXT.icon_strategy_balanced.to_string(),
        }
    }
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
