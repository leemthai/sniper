//! Analysis and computation constants (Immutable Blueprints)

use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::time::Duration;
use strum_macros::{Display, EnumIter};

use crate::ui::config::UI_TEXT;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PhPct(f64);

impl PhPct {

    pub const DEFAULT_VALUE: f64 = 0.15;
    pub const DEFAULT: Self = Self(Self::DEFAULT_VALUE);

    pub const fn new(val: f64) -> Self {
        let v = if val < 0.0 { 
            0.0 
        } else if val > 1.0 { 
            1.0 
        } else { 
            val 
        };
        Self(v)
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
pub struct VolatilityPct(f64);

impl VolatilityPct {

    pub const MIN_EPSILON: f64 = 0.0001;

    pub const fn new(val: f64) -> Self {
        let v = if val < 0.0 { 0.0 } else { val };
        Self(v)
    }

    pub fn as_safe_divisor(&self) -> f64 {
        self.0.max(Self::MIN_EPSILON)
    }

    /// Calculates Volatility % from candle data: (High - Low) / Close
    pub fn calculate(high: f64, low: f64, close: f64) -> Self {
        if close > f64::EPSILON {
            Self::new((high - low) / close)
        } else {
            Self::new(0.0)
        }
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
pub struct MomentumPct(f64);

impl MomentumPct {

    pub const fn new(val: f64) -> Self {
        Self(val)
    }

    /// Calculates Momentum %: (Current - Previous) / Previous
    pub fn calculate(current_close: f64, prev_close: f64) -> Self {
        if prev_close > f64::EPSILON {
            Self::new((current_close - prev_close) / prev_close)
        } else {
            Self::new(0.0)
        }
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

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct RoiPct(f64);

impl RoiPct {
    pub const MIN_EPSILON: f64 = 0.000001;

    pub const fn new(val: f64) -> Self {
        Self(val)
    }

    pub fn is_positive(&self) -> bool {
        self.0 > Self::MIN_EPSILON
    }
}

impl Deref for RoiPct {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for RoiPct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:+.2}%", self.0 * 100.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct AroiPct(f64);

impl AroiPct {
    pub const fn new(val: f64) -> Self {
        Self(val)
    }
}

impl Deref for AroiPct {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for AroiPct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:+.0}%", self.0 * 100.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct Prob(f64);

impl Prob {
    pub const fn new(val: f64) -> Self {
        let v = if val < 0.0 {
            0.0
        } else if val > 1.0 {
            1.0
        } else {
            val
        };
        Self(v)
    }
}

impl Deref for Prob {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for Prob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.1}%", self.0 * 100.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct VolRatio(f64);

impl VolRatio {
    pub const fn new(val: f64) -> Self {
        let v = if val < 0.0 { 0.0 } else { val };
        Self(v)
    }

    /// Calculates the ratio between current and average volume.
    /// Handles division by zero by returning 1.0 (neutral).
    pub fn calculate(current_vol: f64, avg_vol: f64) -> Self {
        if avg_vol > f64::EPSILON {
            Self::new(current_vol / avg_vol)
        } else {
            Self::new(1.0)
        }
    }
}

impl Deref for VolRatio {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for VolRatio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2}x", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct Sigma(f64);

impl Sigma {
    pub const fn new(val: f64) -> Self {
        // Sigma for thresholds is usually positive, but we'll allow 0.0
        let v = if val < 0.0 { 0.0 } else { val };
        Self(v)
    }
}

impl Deref for Sigma {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for Sigma {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.1}σ", self.0)
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

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct BaseVol(f64);

impl BaseVol {
    pub const fn new(val: f64) -> Self {
        let v = if val < 0.0 { 0.0 } else { val };
        Self(v)
    }

    /// Returns the volume as a weight for math (alias for deref)
    pub fn as_weight(&self) -> f64 {
        self.0
    }
}

impl Deref for BaseVol {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for BaseVol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.8}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct QuoteVol(f64);

impl QuoteVol {
    pub const fn new(val: f64) -> Self {
        let v = if val < 0.0 { 0.0 } else { val };
        Self(v)
    }
}

impl Deref for QuoteVol {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::AddAssign for QuoteVol {
    fn add_assign(&mut self, other: Self) {
        self.0 += other.0;
    }
}

impl std::fmt::Display for QuoteVol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let val = self.0;
        if val >= 1_000_000.0 {
            write!(f, "{:.1}M", val / 1_000_000.0)
        } else if val >= 1_000.0 {
            write!(f, "{:.0}K", val / 1_000.0)
        } else {
            write!(f, "{:.0}", val)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct Weight(f64);

impl Weight {
    pub const fn new(val: f64) -> Self {
        let v = if val < 0.0 { 0.0 } else { val };
        Self(v)
    }
}

impl Deref for Weight {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for Weight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.1}", self.0)
    }
}

#[derive(Clone, Debug, Copy)]
pub struct ZoneParams {
    pub smooth_pct: f64,
    pub gap_pct: f64,
    pub viability_pct: f64,
    pub sigma: Sigma,
}

#[derive(Clone, Debug)]
pub struct SimilaritySettings {
    pub weight_volatility: Weight,
    pub weight_momentum: Weight,
    pub weight_volume: Weight,
    pub cutoff_score: f64, 
}

#[derive(Clone, Debug)]
pub struct ZoneClassificationConfig {
    pub sticky: ZoneParams,
    pub reversal: ZoneParams,
}

#[derive(Clone, Debug)]
pub struct TradeProfile {
    pub min_roi_pct: RoiPct,  
    pub min_aroi_pct: AroiPct,
    pub weight_roi: Weight,  
    pub weight_aroi: Weight, 
}

impl TradeProfile {
    
    pub const MS_IN_YEAR: f64 = 365.25 * 24.0 * 60.0 * 60.0 * 1000.0;

    pub fn calculate_annualized_roi(roi: RoiPct, duration_ms: f64) -> AroiPct {
        if duration_ms < 1000.0 {
                return AroiPct::new(0.0);
            }
            let factors_per_year = Self::MS_IN_YEAR / duration_ms;
            AroiPct::new(*roi * factors_per_year)
        }
    /// Returns true if both ROI and AROI meet the minimum thresholds defined in this profile.
    pub fn is_worthwhile(&self, roi_pct: RoiPct, aroi_pct: AroiPct) -> bool {
        *roi_pct >= *self.min_roi_pct && *aroi_pct >= *self.min_aroi_pct
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
