use {
    crate::{
        app::{BASE_INTERVAL, DurationMs, JourneySettings, PhPct, VolatilityPct},
        utils::{TimeUtils, remap},
    },
    std::time::Duration,
};

pub struct AdaptiveParameters;

impl AdaptiveParameters {
    /// Calculates max duration using diffusive market physics (random walk).
    /// Formula: Candles = (Ratio + Bias)^2
    /// Adds +3 bias to give scalps breathing room without affecting swings.
    pub(crate) fn calc_dynamic_journey_duration(
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
            .clamp(journey.min_journey_time, journey.max_journey_time)
    }

    /// Maps price horizon % to trend lookback candle count.
    /// Scalp: 2 h - 1 day, Swing: 1 day - 1 week, Macro: 1 week-1 month+
    /// TEMP why 3 remappings if we have 4 different trades styles. Feels.... wrong. Please look into this one day before wrap-up TEMP
    pub(crate) fn calc_trend_lookback_candles(ph_threshold: PhPct) -> usize {
        let ms = BASE_INTERVAL.as_millis() as i64;
        let day_candles = TimeUtils::duration_to_candles(Duration::from_secs(86_400), ms) as f64;
        let week_candles =
            TimeUtils::duration_to_candles(Duration::from_secs(86_400 * 7), ms) as f64;
        let month_candles =
            TimeUtils::duration_to_candles(Duration::from_secs(86_400 * 30), ms) as f64;
        let v = ph_threshold.value();
        let result = if v < 0.05 {
            remap(v, 0.005, 0.05, 24.0, day_candles)
        } else if v < 0.15 {
            remap(v, 0.05, 0.15, day_candles, week_candles)
        } else {
            remap(v, 0.15, 0.50, week_candles, month_candles)
        };
        result.round() as usize
    }
}
