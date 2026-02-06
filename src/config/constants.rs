use std::time::Duration;

// Top Level Constants
pub const BASE_INTERVAL: Duration = Duration::from_secs(5 * 60); // 5 minutes. Used throughout app from this point forwards.

pub const ZONE_COUNT: usize = 256;
pub const TIME_DECAY_FACTOR: f64 = 1.5;
pub const TUNER_SCAN_STEPS: usize = 4;

pub mod journey {
    use crate::config::{JourneySettings, OptimalSearchSettings, TradeProfile};
    use std::time::Duration;

    pub const SAMPLE_COUNT: usize = 50;
    pub const RISK_REWARD_TESTS: &[f64] = &[1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 10.0];
    pub const MAX_JOURNEY_TIME: Duration = Duration::from_secs(86400 * 90);
    pub const VOLATILITY_ZIGZAG_FACTOR: f64 = 6.0;
    pub const MIN_JOURNEY_DURATION: Duration = Duration::from_secs(3600);

    pub mod profile {
        use crate::config::{AroiPct, RoiPct, Weight};
        pub const MIN_ROI: RoiPct = RoiPct::new(0.001);
        pub const MIN_AROI: AroiPct = AroiPct::new(0.20);
        pub const WEIGHT_ROI: Weight = Weight::new(1.0);
        pub const WEIGHT_AROI: Weight = Weight::new(0.002);
    }

    pub mod optimization {
        use crate::config::{PhPct, Pct};
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

    /// A pre-constructed JourneySettings struct for legacy code that passes it around
    pub const DEFAULT: JourneySettings = JourneySettings {
        sample_count: SAMPLE_COUNT,
        risk_reward_tests: RISK_REWARD_TESTS,
        volatility_zigzag_factor: VOLATILITY_ZIGZAG_FACTOR,
        min_journey_duration: MIN_JOURNEY_DURATION,
        max_journey_time: MAX_JOURNEY_TIME,
        profile: TradeProfile {
            min_roi_pct: self::profile::MIN_ROI,
            min_aroi_pct: self::profile::MIN_AROI,
            weight_roi: self::profile::WEIGHT_ROI,
            weight_aroi: self::profile::WEIGHT_AROI,
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
}

pub mod similarity {
    use crate::config::{SimilaritySettings, Weight};
    pub const WEIGHT_VOLATILITY: Weight = Weight::new(10.0);
    pub const WEIGHT_MOMENTUM: Weight = Weight::new(5.0);
    pub const WEIGHT_VOLUME: Weight = Weight::new(1.0);
    pub const CUTOFF_SCORE: f64 = 100.0;

    pub const DEFAULT: SimilaritySettings = SimilaritySettings {
        weight_volatility: WEIGHT_VOLATILITY,
        weight_momentum: WEIGHT_MOMENTUM,
        weight_volume: WEIGHT_VOLUME,
        cutoff_score: CUTOFF_SCORE,
    };
}

pub mod zones {
    use crate::config::{ZoneClassificationConfig, ZoneParams};
    pub mod sticky {
        use crate::config::{PhPct, Sigma};
        pub const SMOOTH_PCT: PhPct = PhPct::new(0.02);
        pub const GAP_PCT: PhPct = PhPct::new(0.01);
        pub const VIABILITY_PCT: PhPct = PhPct::new(0.001);
        pub const SIGMA: Sigma = Sigma::new(0.2);
    }

    pub mod reversal {
        use crate::config::{PhPct, Sigma};
        pub const SMOOTH_PCT: PhPct = PhPct::new(0.005);
        pub const GAP_PCT: PhPct = PhPct::new(0.0);
        pub const VIABILITY_PCT: PhPct = PhPct::new(0.0005);
        pub const SIGMA: Sigma = Sigma::new(1.5);
    }

    pub const DEFAULT: ZoneClassificationConfig = ZoneClassificationConfig {
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
}

pub mod cva {
    use crate::config::PhPct;
    use crate::utils::TimeUtils;
    // pub const PRICE_RECALC_THRESHOLD_PCT: PhPct = PhPct::new(0.01);
    pub const PRICE_RECALC_THRESHOLD_PCT: PhPct = PhPct::new(0.001); // TEMP put this value in for testing purposes if you want rapid re-triggering caused by prices...
    pub const MIN_CANDLES_FOR_ANALYSIS: usize = 500;
    pub const SEGMENT_MERGE_TOLERANCE_MS: i64 = TimeUtils::MS_IN_D;
}

pub mod tuner {
    use crate::config::PhPct;
    use crate::config::{StationId, TimeTunerConfig, TunerStation};

    pub const STATIONS: &[TunerStation] = &[
        TunerStation {
            id: StationId::Scalp,
            name: "⚡ SCALP",
            short_name: "⚡",
            target_min_hours: 1.0,
            target_max_hours: 6.0,
            scan_ph_min: PhPct::new(0.01),
            scan_ph_max: PhPct::new(0.04),
        },
        TunerStation {
            id: StationId::Day,
            name: "\u{f522} DAY",
            short_name: "\u{f522}",
            target_min_hours: 6.0,
            target_max_hours: 24.0,
            scan_ph_min: PhPct::new(0.03),
            scan_ph_max: PhPct::new(0.08),
        },
        TunerStation {
            id: StationId::Swing,
            name: "🌊 SWING",
            short_name: "🌊",
            target_min_hours: 24.0,
            target_max_hours: 120.0,
            scan_ph_min: PhPct::new(0.05),
            scan_ph_max: PhPct::new(0.15),
        },
        TunerStation {
            id: StationId::Macro,
            name: "\u{ef08} MACRO",
            short_name: "\u{ef08}",
            target_min_hours: 336.0,
            target_max_hours: 2160.0,
            scan_ph_min: PhPct::new(0.15),
            scan_ph_max: PhPct::new(0.60),
        },
    ];

    pub const CONFIG: TimeTunerConfig = TimeTunerConfig { stations: STATIONS };
}
