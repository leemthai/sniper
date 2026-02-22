use {
    crate::{
        config::{DurationMs, JourneySettings, PhPct, VolatilityPct},
        utils::remap,
    },
    std::time::Duration,
};

pub struct AdaptiveParameters;

impl AdaptiveParameters {
    /// Calculates max duration using diffusive market physics (random walk).
    /// Formula: Candles = (Ratio + Bias)^2
    /// Adds +3 bias to give scalps breathing room without affecting swings.
    pub(crate) fn calculate_dynamic_journey_duration(
        ph_pct: PhPct,
        avg_volatility_pct: VolatilityPct,
        interval_ms: DurationMs,
        journey: &JourneySettings,
    ) -> Duration {
        // How many volatility units is the target away?
        let ratio = ph_pct.value() / avg_volatility_pct.as_safe_divisor();

        // Diffusive time with +3 bias (scalp: 25 candles vs 4, swing: negligible change)
        let candles = (ratio + 3.0).powi(2);
        let total_ms = candles * interval_ms.value() as f64;

        Duration::from_millis(total_ms as u64)
            .clamp(journey.min_journey_duration, journey.max_journey_time)
    }

    /// Maps price horizon % to trend lookback candles.
    /// Scalp: 2h-1day, Swing: 1day-1week, Macro: 1week-1month+
    pub(crate) fn calculate_trend_lookback_candles(ph_threshold: PhPct) -> usize {
        const DAY: f64 = 288.0; // 5m candles
        const WEEK: f64 = 2016.0;
        const MONTH: f64 = 8640.0;

        let result = if ph_threshold.value() < 0.05 {
            remap(ph_threshold.value(), 0.005, 0.05, 24.0, DAY)
        } else if ph_threshold.value() < 0.15 {
            remap(ph_threshold.value(), 0.05, 0.15, DAY, WEEK)
        } else {
            remap(ph_threshold.value(), 0.15, 0.50, WEEK, MONTH)
        };

        result.round() as usize
    }
}
