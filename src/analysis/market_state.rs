use serde::{Deserialize, Serialize};


use crate::models::OhlcvTimeSeries;
use crate::config::{MomentumPct, VolatilityPct, VolRatio, PriceLike};

/// A normalized "Fingerprint" of the market conditions at a specific moment in time.
/// Used to find historical matches for the Ghost Runner simulation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct MarketState {
    /// Volatility (The "Temperature")
    /// Ratio of (High-Low) relative to the Close price.
    /// High = Violent/Fast market. Low = Quiet/Consolidation.
    pub volatility_pct: VolatilityPct,

    /// Momentum (The "Velocity")
    /// Percentage change over the last N candles (e.g. 12 candles / 1 hour).
    /// Positive = Rushing up. Negative = Crashing down.
    pub momentum_pct: MomentumPct,

    /// Relative Volume (The "Fuel")
    /// Current Volume divided by Average Volume (e.g. 20-period MA).
    /// > 1.0 = High conviction. < 1.0 = Low liquidity/interest.
    pub relative_volume: VolRatio,
}

impl MarketState {
    /// Calculates the fingerprint for a specific index.
    /// `lookback`: Number of candles to use for Momentum and Volume MA.
    pub(crate) fn calculate(ts: &OhlcvTimeSeries, idx: usize, trend_lookback: usize) -> Option<Self> {
        // Safety check
        if idx < trend_lookback || trend_lookback == 0 {
            return None;
        }

        let current = ts.get_candle(idx);
        
        // 1. Volatility (Unchanged)
        let volatility = VolatilityPct::calculate(current.high_price.value(), current.low_price.value(), current.close_price.value());

        // 2. Momentum (Adaptive - O(1) Lookup)
        let prev_n = ts.get_candle(idx - trend_lookback);
        let momentum = MomentumPct::calculate(current.close_price.value(), prev_n.close_price.value());

        // 3. Relative Volume (O(1) Lookup)
        // We now read the pre-calculated value directly.
        let rel_vol = ts.relative_volumes.get(idx).copied().unwrap_or_else(|| panic!("something gone wrong with idx value, {},  in market_state::calculate ", idx));

        Some(Self {
            volatility_pct: volatility,
            momentum_pct: momentum,
            relative_volume: rel_vol,
        })
    }
}
