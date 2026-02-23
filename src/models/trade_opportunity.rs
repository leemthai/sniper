use {
    crate::{
        config::{
            AroiPct, DurationMs, JourneySettings, OptimalSearchSettings, OptimizationStrategy, Pct,
            PhPct, Price, RoiPct, Sigma, StationId, StopPrice, TargetPrice, TradeProfile,
            ZoneClassificationConfig, ZoneParams,
        },
        models::{EmpiricalOutcomeStats, MarketState},
        ui::UI_TEXT,
    },
    chrono::{DateTime, Utc},
    serde::{Deserialize, Serialize},
    std::{fmt, time::Duration},
};

#[cfg(not(target_arch = "wasm32"))]
use chrono::Duration as ChronoDuration;

const SAMPLE_COUNT: usize = 50;
const RISK_REWARD_TESTS: &[f64] = &[1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 10.0];
const MAX_JOURNEY_TIME: Duration = Duration::from_secs(86400 * 90);
const MIN_JOURNEY_DURATION: Duration = Duration::from_secs(3600);

mod profile {
    use super::*;
    pub const MIN_AROI: AroiPct = AroiPct::new(0.20);
    pub const MIN_ROI: RoiPct = RoiPct::new(0.001);
}

mod optimization {
    use super::*;
    pub const DIVERSITY_CUT_OFF: PhPct = PhPct::new(0.5);
    pub const DIVERSITY_REGIONS: usize = 5;
    pub const DRILL_CUTOFF_PCT: PhPct = PhPct::new(0.70);
    pub const DRILL_OFFSET_FACTOR: f64 = 0.25;
    pub const DRILL_TOP_N: usize = 5;
    pub const FUZZY_MATCH_TOLERANCE: Pct = Pct::new(0.5);
    pub const MAX_RESULTS: usize = 5;
    pub const PRICE_BUFFER_PCT: PhPct = PhPct::new(0.005);
    pub const PRUNE_INTERVAL_SEC: u64 = 10;
    pub const SCOUT_STEPS: usize = 20;
    pub const VOLATILITY_LOOKBACK: usize = 50;
}

pub(crate) const DEFAULT_JOURNEY_SETTINGS: JourneySettings = JourneySettings {
    sample_count: SAMPLE_COUNT,
    risk_reward_tests: RISK_REWARD_TESTS,
    min_journey_duration: MIN_JOURNEY_DURATION,
    max_journey_time: MAX_JOURNEY_TIME,
    profile: TradeProfile {
        min_roi_pct: profile::MIN_ROI,
        min_aroi_pct: profile::MIN_AROI,
    },
    optimization: OptimalSearchSettings {
        diversity_cut_off: optimization::DIVERSITY_CUT_OFF,
        diversity_regions: optimization::DIVERSITY_REGIONS,
        drill_cutoff_pct: optimization::DRILL_CUTOFF_PCT,
        drill_offset_factor: optimization::DRILL_OFFSET_FACTOR,
        drill_top_n: optimization::DRILL_TOP_N,
        fuzzy_match_tolerance: optimization::FUZZY_MATCH_TOLERANCE,
        max_results: optimization::MAX_RESULTS,
        price_buffer_pct: optimization::PRICE_BUFFER_PCT,
        prune_interval_sec: optimization::PRUNE_INTERVAL_SEC,
        scout_steps: optimization::SCOUT_STEPS,
        volatility_lookback: optimization::VOLATILITY_LOOKBACK,
    },
};

mod sticky {
    use super::*;
    pub const GAP_PCT: PhPct = PhPct::new(0.01);
    pub const SIGMA: Sigma = Sigma::new(0.2);
    pub const SMOOTH_PCT: PhPct = PhPct::new(0.02);
    pub const VIABILITY_PCT: PhPct = PhPct::new(0.001);
}
mod reversal {
    use super::*;
    pub const GAP_PCT: PhPct = PhPct::new(0.0);
    pub const SIGMA: Sigma = Sigma::new(1.5);
    pub const SMOOTH_PCT: PhPct = PhPct::new(0.005);
    pub const VIABILITY_PCT: PhPct = PhPct::new(0.0005);
}

pub(crate) const DEFAULT_ZONE_CONFIG: ZoneClassificationConfig = ZoneClassificationConfig {
    sticky: ZoneParams {
        gap_pct: sticky::GAP_PCT,
        sigma: sticky::SIGMA,
        smooth_pct: sticky::SMOOTH_PCT,
        viability_pct: sticky::VIABILITY_PCT,
    },
    reversal: ZoneParams {
        gap_pct: reversal::GAP_PCT,
        sigma: reversal::SIGMA,
        smooth_pct: reversal::SMOOTH_PCT,
        viability_pct: reversal::VIABILITY_PCT,
    },
};

impl OptimizationStrategy {
    pub fn objective_score_simple(&self, avg_pnl_pct: RoiPct, duration: DurationMs) -> f64 {
        let mean = avg_pnl_pct.value();

        match self {
            Self::MaxROI => mean,
            Self::MaxAROI => TradeProfile::calculate_annualized_roi(avg_pnl_pct, duration).value(),
            Self::Balanced => {
                let aroi = TradeProfile::calculate_annualized_roi(avg_pnl_pct, duration).value();
                if mean <= 0.0 {
                    mean
                } else {
                    (mean * aroi).sqrt()
                }
            }
            Self::LogGrowthConfidence => mean,
        }
    }

    pub fn objective_score(&self, stats: &EmpiricalOutcomeStats, duration: DurationMs) -> f64 {
        let mean = stats.avg_pnl_pct.value();

        match self {
            Self::MaxROI => mean,
            Self::MaxAROI => {
                TradeProfile::calculate_annualized_roi(stats.avg_pnl_pct, duration).value()
            }
            Self::Balanced => {
                let aroi =
                    TradeProfile::calculate_annualized_roi(stats.avg_pnl_pct, duration).value();
                if mean <= 0.0 {
                    mean
                } else {
                    (mean * aroi).sqrt()
                }
            }
            Self::LogGrowthConfidence => {
                if stats.sample_size < 2 {
                    return mean;
                }
                let n = stats.sample_size as f64;
                let confidence = 1.0 - 1.0 / n.sqrt();
                let mean_adj = mean * confidence;
                mean_adj - 0.5 * stats.return_variance
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TradeVariant {
    pub ratio: f64,
    pub roi_pct: RoiPct,
    pub simulation: EmpiricalOutcomeStats,
    pub stop_price: StopPrice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum TradeDirection {
    Long,
    Short,
}

impl fmt::Display for TradeDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Long => write!(f, "{}", UI_TEXT.label_long),
            Self::Short => write!(f, "{}", UI_TEXT.label_short),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VisualFluff {
    pub volume_profile: Vec<f64>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) enum TradeOutcome {
    TargetHit,
    StopHit,
    Timeout,
    ManualClose,
}

#[cfg(not(target_arch = "wasm32"))]
impl fmt::Display for TradeOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetHit => write!(f, "TARGET"),
            Self::StopHit => write!(f, "STOP"),
            Self::Timeout => write!(f, "TIMEOUT"),
            Self::ManualClose => write!(f, "MANUAL"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TradeOpportunity {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub ph_pct: PhPct,
    pub pair_name: String,
    pub direction: TradeDirection,
    pub start_price: Price,
    pub target_price: TargetPrice,
    pub stop_price: StopPrice,
    pub max_duration: DurationMs,
    pub avg_duration: DurationMs,
    pub strategy: OptimizationStrategy,
    pub station_id: StationId,
    pub market_state: MarketState,
    pub visuals: Option<VisualFluff>,
    pub simulation: EmpiricalOutcomeStats,
    pub variants: Vec<TradeVariant>,
}

impl TradeOpportunity {
    /// Returns true if opportunities can be compared/merged.
    /// Invariant: comparable iff same pair, direction, strategy, and station.
    pub(crate) fn is_comparable_to(&self, other: &Self) -> bool {
        self.pair_name == other.pair_name
            && self.direction == other.direction
            && self.strategy == other.strategy
            && self.station_id == other.station_id
    }

    #[cfg(debug_assertions)]
    pub(crate) fn assert_comparable_to(&self, other: &Self) {
        debug_assert!(
            self.is_comparable_to(other),
            "Ledger invariant violated: attempted to compare non-comparable opportunities"
        );
    }

    pub(crate) fn calculate_quality_score(&self) -> f64 {
        self.strategy
            .objective_score_simple(self.expected_roi(), self.avg_duration)
    }
    /// Determines if trade has exited based on current price action and time.
    /// Checks stop before target (pessimistic).
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn check_exit_condition(
        &self,
        current_high: Price,
        current_low: Price,
        current_time: DateTime<Utc>,
    ) -> Option<TradeOutcome> {
        if current_time > self.created_at + ChronoDuration::from(self.max_duration) {
            return Some(TradeOutcome::Timeout);
        }

        match self.direction {
            TradeDirection::Long => {
                // Check stop first (pessimistic)
                if current_low <= Price::from(self.stop_price) {
                    return Some(TradeOutcome::StopHit);
                }
                if current_high >= Price::from(self.target_price) {
                    return Some(TradeOutcome::TargetHit);
                }
            }
            TradeDirection::Short => {
                if current_high >= Price::from(self.stop_price) {
                    return Some(TradeOutcome::StopHit);
                }
                if current_low <= Price::from(self.target_price) {
                    return Some(TradeOutcome::TargetHit);
                }
            }
        }

        None
    }

    pub(crate) fn variant_count(&self) -> usize {
        self.variants.len()
    }

    pub(crate) fn is_worthwhile(&self, profile: &TradeProfile) -> bool {
        let roi = self.expected_roi();
        let aroi = TradeProfile::calculate_annualized_roi(roi, self.avg_duration);
        profile.is_worthwhile(roi, aroi)
    }

    pub(crate) fn expected_roi(&self) -> RoiPct {
        self.simulation.avg_pnl_pct
    }

    /// Calculates expected ROI adjusted for price movement since entry.
    pub(crate) fn live_roi(&self, current_price: Price) -> RoiPct {
        let base_roi = self.expected_roi();
        let price_drift_pct = match self.direction {
            TradeDirection::Long => (current_price - self.start_price) / self.start_price,
            TradeDirection::Short => (self.start_price - current_price) / self.start_price,
        };
        RoiPct::new(base_roi.value() + price_drift_pct)
    }

    pub(crate) fn live_annualized_roi(&self, current_price: Price) -> AroiPct {
        let roi = self.live_roi(current_price);
        TradeProfile::calculate_annualized_roi(roi, self.avg_duration)
    }
}

impl fmt::Display for TradeOpportunity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ID {} (pair: {})", self.id, self.pair_name)
    }
}
