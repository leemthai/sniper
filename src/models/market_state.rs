use {
    crate::{
        config::{MomentumPct, PriceLike, VolRatio, VolatilityPct},
        models::OhlcvTimeSeries,
    },
    serde::{Deserialize, Serialize},
    std::fmt,
};

/// Market fingerprint used to find historical matches for Ghost Runner simulation.
/// Volatility (temperature): (High-Low)/Close. High = violent, Low = quiet.
/// Momentum (velocity): % change over N candles. Positive = up, Negative = down.
/// Relative Volume (fuel): Current/Average volume. >1 = high conviction, <1 = low interest.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct MarketState {
    pub volatility_pct: VolatilityPct,
    pub momentum_pct: MomentumPct,
    pub relative_volume: VolRatio,
}

impl fmt::Display for MarketState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "vol={:.4}|mom={:.4}|rv={:.4}",
            self.volatility_pct.value(),
            self.momentum_pct.value(),
            self.relative_volume.value(),
        )
    }
}

impl MarketState {
    /// Calculates market fingerprint at specific index.
    /// Returns None if idx < trend_lookback.
    pub(crate) fn calculate(
        ts: &OhlcvTimeSeries,
        idx: usize,
        trend_lookback: usize,
    ) -> Option<Self> {
        if idx < trend_lookback || trend_lookback == 0 {
            return None;
        }

        let current = ts.get_candle(idx);
        let volatility = VolatilityPct::calculate(
            current.high_price.value(),
            current.low_price.value(),
            current.close_price.value(),
        );

        let prev_n = ts.get_candle(idx - trend_lookback);
        let momentum =
            MomentumPct::calculate(current.close_price.value(), prev_n.close_price.value());

        let rel_vol = ts.relative_volumes[idx];

        Some(Self {
            volatility_pct: volatility,
            momentum_pct: momentum,
            relative_volume: rel_vol,
        })
    }
}
