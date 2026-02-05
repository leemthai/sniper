//! Analysis and computation constants (Immutable Blueprints)

use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Mul, Sub};
use std::time::Duration;
use strum_macros::{Display, EnumIter};

use crate::ui::config::UI_TEXT;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter)]
pub enum CandleResolution {
    M5,
    M15,
    H1,
    H4,
    D1,
    D3,
    W1,
    M1,
}

impl Default for CandleResolution {
    fn default() -> Self {
        Self::D1 // Default to 1D candles for plot candles
    }
}

impl CandleResolution {
    pub fn duration(&self) -> Duration {
        match self {
            Self::M5 => Duration::from_secs(5 * 60),
            Self::M15 => Duration::from_secs(15 * 60),
            Self::H1 => Duration::from_secs(60 * 60),
            Self::H4 => Duration::from_secs(4 * 60 * 60),
            Self::D1 => Duration::from_secs(24 * 60 * 60),
            Self::D3 => Duration::from_secs(3 * 24 * 60 * 60),
            Self::W1 => Duration::from_secs(7 * 24 * 60 * 60),
            Self::M1 => Duration::from_secs(30 * 24 * 60 * 60), // approx
        }
    }
}

impl std::fmt::Display for CandleResolution {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::M5 => write!(f, "5m"),
            Self::M15 => write!(f, "15m"),
            Self::H1 => write!(f, "1h"),
            Self::H4 => write!(f, "4h"),
            Self::D1 => write!(f, "1D"),
            Self::D3 => write!(f, "3D"),
            Self::W1 => write!(f, "1W"),
            Self::M1 => write!(f, "1M"),
        }
    }
}

impl From<CandleResolution> for Duration {
    fn from(res: CandleResolution) -> Self {
        res.duration()
    }
}

impl CandleResolution {
    pub fn steps_from(&self, base: Duration) -> u64 {
        self.duration().as_secs() / base.as_secs()
    }
}

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

    #[inline]
    pub fn value(self) -> f64 {
        self.0
    }

}

impl Default for PhPct {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl std::fmt::Display for PhPct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.4}%", self.0 * 100.)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct Pct(f64);

impl Pct {
    // A 'general' % clamped between 0 and 1
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

    #[inline]
    pub fn value(self) -> f64 {
        self.0
    }

}

impl std::fmt::Display for Pct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.4}%", self.0 * 100.)
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

    #[inline]
    pub fn value(self) -> f64 {
        self.0
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

    #[inline]
    pub fn value(self) -> f64 {
        self.0
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

    #[inline]
    pub fn value(self) -> f64 {
        self.0
    }

    pub fn is_positive(&self) -> bool {
        self.0 > Self::MIN_EPSILON
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

    #[inline]
    pub fn value(self) -> f64 {
        self.0
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

    pub fn value(self) -> f64 {
        self.0
    }
}

impl std::fmt::Display for Prob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.1}%", self.0 * 100.0)
    }
}

use chrono::Duration as ChronoDuration;

impl From<DurationMs> for ChronoDuration {
    fn from(d: DurationMs) -> Self {
        ChronoDuration::milliseconds(d.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct DurationMs(i64);

impl DurationMs {
    const MS_IN_YEAR: f64 = 365.25 * 24.0 * 60.0 * 60.0 * 1000.0;

    pub const fn new(ms: i64) -> Self {
        Self(ms)
    }

    pub fn value(self) -> i64 {
        self.0
    }
    /// Converts duration to a float number of years (for annualized math).
    pub fn to_years_f64(&self) -> f64 {
        if self.0 <= 0 {
            0.0
        } else {
            self.0 as f64 / Self::MS_IN_YEAR
        }
    }
}

impl std::fmt::Display for DurationMs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}ms", self.0)
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

    #[inline]
    pub fn value(self) -> f64 {
        self.0
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

    #[inline]
    pub fn value(self) -> f64 {
        self.0
    }
}

impl std::fmt::Display for Sigma {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.1}σ", self.0)
    }
}

/// A behavioral contract for anything that behaves like a price.
pub trait PriceLike {
    fn value(&self) -> f64;

    const MIN_EPSILON: f64 = 1e-12;

    fn is_positive(&self) -> bool {
        self.value() > Self::MIN_EPSILON
    }

    // fn percent_diff_from_0_100<R: PriceLike>(&self, reference: &R) -> f64 {
    //     if !reference.is_positive() {
    //         return 0.0;
    //     }

    //     (self.value() - reference.value()).abs() / reference.value() * 100.0
    // }

    fn percent_diff_from_0_1<R: PriceLike>(&self, reference: &R) -> f64 {
        if !reference.is_positive() {
            return 0.0;
        }

        (self.value() - reference.value()).abs() / reference.value()
    }

    /// Formats a price with "Trader Precision" adaptive decimals.
    fn format_price(&self) -> String {
        let price = self.value();
        if price == 0.0 {
            return "$0.00".to_string();
        }

        // Determine magnitude
        let abs_price = price.abs();

        if abs_price >= 1000.0 {
            format!("${:.2}", price)
        } else if abs_price >= 1.0 {
            format!("${:.4}", price)
        } else if abs_price >= 0.01 {
            format!("${:.5}", price)
        } else {
            format!("${:.8}", price)
        }
    }
}

macro_rules! impl_into_price {
    ($from:ident) => {
        impl From<$from> for Price {
            fn from(p: $from) -> Self {
                Price::new(p.value())
            }
        }
    };
}

macro_rules! impl_from_price {
    ($to:ident) => {
        impl From<Price> for $to {
            fn from(p: Price) -> Self {
                $to::new(p.value())
            }
        }
    };
}

macro_rules! define_price_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default)]
        #[serde(transparent)]
        pub struct $name(f64);

        impl From<f64> for $name {
            fn from(v: f64) -> Self {
                $name::new(v)
            }
        }

        impl $name {
            pub const fn new(val: f64) -> Self {
                // Absolute prices should not be negative
                let v = if val < 0.0 { 0.0 } else { val };
                Self(v)
            }
        }

        impl Add for $name {
            type Output = f64;

            fn add(self, rhs: Self) -> Self::Output {
                self.value() + rhs.value()
            }
        }

        impl Sub for $name {
            type Output = f64;

            fn sub(self, rhs: Self) -> Self::Output {
                self.value() - rhs.value()
            }
        }

        impl Div for $name {
            type Output = f64;

            fn div(self, rhs: Self) -> Self::Output {
                self.value() / rhs.value()
            }
        }

        impl Div<$name> for f64 {
            type Output = f64;

            fn div(self, rhs: $name) -> Self::Output {
                self / rhs.value()
            }
        }

        impl PriceLike for $name {
            fn value(&self) -> f64 {
                self.0
            }
        }

        impl Div<f64> for $name {
            type Output = $name;

            fn div(self, rhs: f64) -> Self::Output {
                $name::new(self.value() / rhs)
            }
        }

        impl Mul<$name> for f64 {
            type Output = $name;

            fn mul(self, rhs: $name) -> Self::Output {
                $name::new(self * rhs.value())
            }
        }

        impl $name {
            #[inline]
            pub fn abs(self) -> f64 {
                self.value().abs()
            }
        }

        impl Mul<f64> for $name {
            type Output = $name;

            fn mul(self, rhs: f64) -> Self::Output {
                $name::new(self.value() * rhs)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.format_price())
            }
        }
    };
}

macro_rules! impl_price_compare {
    ($a:ty, $b:ty) => {
        impl PartialEq<$b> for $a {
            fn eq(&self, other: &$b) -> bool {
                self.value() == other.value()
            }
        }

        impl PartialOrd<$b> for $a {
            fn partial_cmp(&self, other: &$b) -> Option<std::cmp::Ordering> {
                self.value().partial_cmp(&other.value())
            }
        }
    };
}

// Generate the Price Hierarchy
define_price_type!(Price);
define_price_type!(OpenPrice);
define_price_type!(HighPrice);
define_price_type!(LowPrice);
define_price_type!(ClosePrice);
define_price_type!(TargetPrice);
define_price_type!(StopPrice);

impl Price {
    #[inline]
    pub fn clamp(self, min: Price, max: Price) -> Price {
        if self < min {
            min
        } else if self > max {
            max
        } else {
            self
        }
    }
}

impl_into_price!(OpenPrice);
impl_into_price!(HighPrice);
impl_into_price!(LowPrice);
impl_into_price!(ClosePrice);
impl_into_price!(TargetPrice);
impl_into_price!(StopPrice);

impl_from_price!(LowPrice);
impl_from_price!(HighPrice);
impl_from_price!(OpenPrice);
impl_from_price!(ClosePrice);
impl_from_price!(TargetPrice);
impl_from_price!(StopPrice);

impl_price_compare!(LowPrice, HighPrice);
impl_price_compare!(HighPrice, LowPrice);

impl_price_compare!(LowPrice, Price);
impl_price_compare!(HighPrice, Price);
impl_price_compare!(Price, LowPrice);
impl_price_compare!(Price, HighPrice);
impl_price_compare!(OpenPrice, Price);
impl_price_compare!(ClosePrice, Price);
impl_price_compare!(TargetPrice, Price);
impl_price_compare!(StopPrice, Price);

#[derive(serde::Deserialize, serde::Serialize, Default, Debug, Clone)]
pub struct PriceRange<T: PriceLike> {
    pub start: T,
    pub end: T,
    pub n_chunks: usize,
}

impl<T: PriceLike> PriceRange<T> {
    pub fn new(start: T, end: T, n_chunks: usize) -> Self {
        Self {
            start,
            end,
            n_chunks,
        }
    }

    pub fn min_max(&self) -> (f64, f64) {
        (self.start.value(), self.end.value())
    }

    pub fn chunk_size(&self) -> f64 {
        (self.end.value() - self.start.value()) / self.n_chunks as f64
    }

    pub fn chunk_index(&self, value: T) -> usize {
        let index = (value.value() - self.start.value()) / self.chunk_size();
        (index as usize).min(self.n_chunks - 1)
    }

    pub fn chunk_bounds(&self, idx: usize) -> (f64, f64) {
        let low = self.start.value() + idx as f64 * self.chunk_size();
        let high = self.start.value() + (idx + 1) as f64 * self.chunk_size();
        (low, high)
    }

    pub fn count_intersecting_chunks(&self, low: T, high: T) -> usize {
        let mut x_low = low.value();
        let mut x_high = high.value();

        if x_high < x_low {
            (x_low, x_high) = (x_high, x_low);
        }

        let first = ((x_low - self.start.value()) / self.chunk_size()).floor() as isize;
        let last = ((x_high - self.start.value()) / self.chunk_size()).floor() as isize;

        let first = first.max(0);
        let last = last.min((self.n_chunks - 1) as isize);

        if last < first {
            return 0;
        }

        (last - first + 1) as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct PriceDelta(pub f64);

impl PriceDelta {
    pub const fn new(val: f64) -> Self {
        Self(val) // Deltas can be negative
    }

    #[inline]
    pub fn value(self) -> f64 {
        self.0
    }
}

impl std::fmt::Display for PriceDelta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Deltas show the sign but usually not the currency symbol
        write!(f, "{:+.4}", self.0)
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

    #[inline]
    pub fn value(self) -> f64 {
        self.0
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

    #[inline]
    pub fn value(self) -> f64 {
        self.0
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

    #[inline]
    pub fn value(self) -> f64 {
        self.0
    }
}

impl std::fmt::Display for Weight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.1}", self.0)
    }
}

#[derive(Clone, Debug, Copy)]
pub struct ZoneParams {
    pub smooth_pct: PhPct,
    pub gap_pct: PhPct,
    pub viability_pct: PhPct,
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
    pub fn calculate_annualized_roi(roi: RoiPct, duration: DurationMs) -> AroiPct {
        let years = duration.to_years_f64();
        if years <= 0.0000001 {
            return AroiPct::new(0.0);
        }
        let factor = 1.0 / years;
        AroiPct::new(roi.value() * factor)
    }
    /// Returns true if both ROI and AROI meet the minimum thresholds defined in this profile.
    pub fn is_worthwhile(&self, roi_pct: RoiPct, aroi_pct: AroiPct) -> bool {
        roi_pct >= self.min_roi_pct && aroi_pct >= self.min_aroi_pct
    }
}

#[derive(Clone, Debug)]
pub struct OptimalSearchSettings {
    pub scout_steps: usize,
    pub drill_top_n: usize,
    pub drill_offset_factor: f64,
    pub drill_cutoff_pct: PhPct,
    pub volatility_lookback: usize,
    pub diversity_regions: usize,
    pub diversity_cut_off: PhPct,
    pub max_results: usize,
    pub price_buffer_pct: PhPct,
    pub fuzzy_match_tolerance: PhPct,
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
