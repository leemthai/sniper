use std::fmt;
use std::time::Duration;

use chrono::Duration as ChronoDuration;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::analysis::market_state::MarketState;
use crate::analysis::scenario_simulator::SimulationResult;

use crate::config::{
    AroiPct, DurationMs, OptimizationStrategy, Price, RoiPct, StationId,
    StopPrice, TargetPrice, TradeProfile, ZoneClassificationConfig, ZoneParams, PhPct, Sigma, Pct, JourneySettings, OptimalSearchSettings
};

use crate::ui::config::UI_TEXT;

const SAMPLE_COUNT: usize = 50;
const RISK_REWARD_TESTS: &[f64] = &[1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 10.0];
const MAX_JOURNEY_TIME: Duration = Duration::from_secs(86400 * 90);
// const VOLATILITY_ZIGZAG_FACTOR: f64 = 6.0;
const MIN_JOURNEY_DURATION: Duration = Duration::from_secs(3600);

mod profile {
    use super::*;
    pub const MIN_ROI: RoiPct = RoiPct::new(0.001);
    pub const MIN_AROI: AroiPct = AroiPct::new(0.20);
    // pub const WEIGHT_ROI: Weight = Weight::new(1.0);
    // pub const WEIGHT_AROI: Weight = Weight::new(0.002);
}

mod optimization {
    use super::*;
    pub const SCOUT_STEPS: usize = 20;
    pub const DRILL_TOP_N: usize = 5;
    pub const DRILL_OFFSET_FACTOR: f64 = 0.25;
    pub const DRILL_CUTOFF_PCT: PhPct = PhPct::new(0.70);
    pub const VOLATILITY_LOOKBACK: usize = 50;
    pub const DIVERSITY_REGIONS: usize = 5;
    pub const DIVERSITY_CUT_OFF: PhPct = PhPct::new(0.5);
    pub const MAX_RESULTS: usize = 5;
    pub const PRICE_BUFFER_PCT: PhPct = PhPct::new(0.005);
    pub const FUZZY_MATCH_TOLERANCE: Pct = Pct::new(0.5);
    pub const PRUNE_INTERVAL_SEC: u64 = 10;
}

pub(crate) const DEFAULT_JOURNEY_SETTINGS: JourneySettings = JourneySettings {
    sample_count: SAMPLE_COUNT,
    risk_reward_tests: RISK_REWARD_TESTS,
    // volatility_zigzag_factor: VOLATILITY_ZIGZAG_FACTOR,
    min_journey_duration: MIN_JOURNEY_DURATION,
    max_journey_time: MAX_JOURNEY_TIME,
    profile: TradeProfile {
        min_roi_pct: profile::MIN_ROI,
        min_aroi_pct: profile::MIN_AROI,
        // weight_roi: profile::WEIGHT_ROI,
        // weight_aroi: profile::WEIGHT_AROI,
    },
    optimization: OptimalSearchSettings {
        scout_steps: optimization::SCOUT_STEPS,
        drill_top_n: optimization::DRILL_TOP_N,
        drill_offset_factor: optimization::DRILL_OFFSET_FACTOR,
        drill_cutoff_pct: optimization::DRILL_CUTOFF_PCT,
        volatility_lookback: optimization::VOLATILITY_LOOKBACK,
        diversity_regions: optimization::DIVERSITY_REGIONS,
        diversity_cut_off: optimization::DIVERSITY_CUT_OFF,
        max_results: optimization::MAX_RESULTS,
        price_buffer_pct: optimization::PRICE_BUFFER_PCT,
        fuzzy_match_tolerance: optimization::FUZZY_MATCH_TOLERANCE,
        prune_interval_sec: optimization::PRUNE_INTERVAL_SEC,
    },
};

mod sticky {
    use super::*;
    pub const SMOOTH_PCT: PhPct = PhPct::new(0.02);
    pub const GAP_PCT: PhPct = PhPct::new(0.01);
    pub const VIABILITY_PCT: PhPct = PhPct::new(0.001);
    pub const SIGMA: Sigma = Sigma::new(0.2);
}
mod reversal {
    use super::*;
    pub const SMOOTH_PCT: PhPct = PhPct::new(0.005);
    pub const GAP_PCT: PhPct = PhPct::new(0.0);
    pub const VIABILITY_PCT: PhPct = PhPct::new(0.0005);
    pub const SIGMA: Sigma = Sigma::new(1.5);
    }

pub(crate) const DEFAULT_ZONE_CONFIG: ZoneClassificationConfig = ZoneClassificationConfig {
    sticky: ZoneParams {
        smooth_pct: sticky::SMOOTH_PCT,
        gap_pct: sticky::GAP_PCT,
        viability_pct: sticky::VIABILITY_PCT,
        sigma: sticky::SIGMA,
    },
    reversal: ZoneParams {
        smooth_pct: reversal::SMOOTH_PCT,
        gap_pct: reversal::GAP_PCT,
        viability_pct: reversal::VIABILITY_PCT,
        sigma: reversal::SIGMA,
    },
};

impl OptimizationStrategy {
    /// Calculate a score based on the strategy
    pub fn calculate_score(&self, roi_pct: RoiPct, duration: DurationMs) -> f64 {
        match self {
            OptimizationStrategy::MaxROI => roi_pct.value(),
            OptimizationStrategy::MaxAROI => {
                // ROI acts as a hard filter (via Gatekeeper), but we maximize speed here
                TradeProfile::calculate_annualized_roi(roi_pct, duration).value()
            }
            OptimizationStrategy::Balanced => {
                // GEOMETRIC MEAN (Efficiency Score)
                let aroi_pct = TradeProfile::calculate_annualized_roi(roi_pct, duration);

                // If trade is losing, score is negative.
                if !roi_pct.is_positive() {
                    return roi_pct.value(); // Simple fallback for losers
                }

                // If ROI is positive but AROI is massive (tiny duration), sqrt dampens it.
                // If ROI is massive but AROI is low (long duration), sqrt dampens it.
                // It peaks when BOTH are healthy.
                (roi_pct.value() * aroi_pct.value()).sqrt()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TradeVariant {
    pub ratio: f64,
    pub stop_price: StopPrice,
    pub roi_pct: RoiPct,
    pub simulation: SimulationResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum TradeDirection {
    Long,
    Short,
}

impl fmt::Display for TradeDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TradeDirection::Long => write!(f, "{}", UI_TEXT.label_long),
            TradeDirection::Short => write!(f, "{}", UI_TEXT.label_short),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VisualFluff {
    // Purely for visualization. Not used for calculation.
    // The "Hills and Valleys" of volume (CVA Histogram).
    pub volume_profile: Vec<f64>,
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) enum TradeOutcome {
    TargetHit,
    StopHit,
    Timeout,
    ManualClose,
}

impl std::fmt::Display for TradeOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradeOutcome::TargetHit => write!(f, "TARGET"),
            TradeOutcome::StopHit => write!(f, "STOP"),
            TradeOutcome::Timeout => write!(f, "TIMEOUT"),
            TradeOutcome::ManualClose => write!(f, "MANUAL"),
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

    pub simulation: SimulationResult,
    pub variants: Vec<TradeVariant>,
}

impl TradeOpportunity {
    /// Returns true if two opportunities are allowed to be compared / merged.
    ///
    /// LEDGER INVARIANT:
    /// Opportunities are comparable IFF they belong to the same
    /// pair, direction, strategy, and station.
    #[inline]
    pub(crate) fn is_comparable_to(&self, other: &TradeOpportunity) -> bool {
        self.pair_name == other.pair_name
            && self.direction == other.direction
            && self.strategy == other.strategy
            && self.station_id == other.station_id
    }

    #[cfg(debug_assertions)]
    #[inline]
    pub(crate) fn assert_comparable_to(&self, other: &TradeOpportunity) {
        debug_assert!(
            self.is_comparable_to(other),
            "Ledger invariant violated: attempted to compare non-comparable opportunities"
        );
    }
}

impl fmt::Display for TradeOpportunity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ID {} (pair: {})", self.id, self.pair_name)
    }
}

impl TradeOpportunity {
    /// Calculates a composite Quality Score (0.0 to 100.0+)
    /// Used for "Auto-Tuning" and finding the best setups.
    pub(crate) fn calculate_quality_score(&self) -> f64 {
        self.strategy
            .calculate_score(self.expected_roi(), self.avg_duration)
    }

    /// Centralized "Referee" Logic.
    /// Determines if the trade is dead based on current price action and time.
    pub(crate)  fn check_exit_condition(
        &self,
        current_high: Price,
        current_low: Price,
        current_time: DateTime<Utc>,
    ) -> Option<TradeOutcome> {
        // 1. Check Expiry (Hard Limit)
        if current_time > self.created_at + ChronoDuration::from(self.max_duration) {
            return Some(TradeOutcome::Timeout);
        }

        // 2. Check Price Levels
        match self.direction {
            TradeDirection::Long => {
                // Pessimistic: Check Stop first
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

    /// Helper to get number of variants (including the active one)
    pub(crate)  fn variant_count(&self) -> usize {
        self.variants.len()
    }

    /// Checks if the SNAPSHOT (Creation) status was worthwhile.
    pub(crate)  fn is_worthwhile(&self, profile: &TradeProfile) -> bool {
        let roi = self.expected_roi();
        let aroi = TradeProfile::calculate_annualized_roi(roi, self.avg_duration);
        profile.is_worthwhile(roi, aroi)
    }

    /// Calculates the Expected ROI % per trade for this specific opportunity.
    pub(crate)  fn expected_roi(&self) -> RoiPct {
        // RETURN THE SIMULATION TRUTH. The simulation already calculated the true average PnL (including timeouts).
        self.simulation.avg_pnl_pct
    }

    /// Calculates the Expected ROI % using a dynamic live price.
    pub(crate)  fn live_roi(&self, current_price: Price) -> RoiPct {
        // 1. Get the baseline "True PnL" from the simulation (e.g. 7.0%)
        let base_roi = self.expected_roi();

        // 2. Calculate how much price has moved in our favor since entry
        // Long: (Current - Start) / Start
        // Short: (Start - Current) / Start
        let price_drift_pct = match self.direction {
            TradeDirection::Long => (current_price - self.start_price) / self.start_price,
            TradeDirection::Short => (self.start_price - current_price) / self.start_price,
        };

        // 3. Adjust the ROI
        // If price moved +1% in our favor, our expected return improves by +1% (simplification)
        RoiPct::new(base_roi.value() + price_drift_pct)
    }

    /// Calculates Annualized ROI based on LIVE price and AVERAGE duration.
    pub(crate)  fn live_annualized_roi(&self, current_price: Price) -> AroiPct {
        let roi = self.live_roi(current_price);
        TradeProfile::calculate_annualized_roi(roi, self.avg_duration)
    }
}
