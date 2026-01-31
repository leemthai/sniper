use serde::{Deserialize, Serialize};


use crate::models::OhlcvTimeSeries;
use crate::config::{MomentumPct, SimilaritySettings, VolatilityPct, VolRatio};

/// A normalized "Fingerprint" of the market conditions at a specific moment in time.
/// Used to find historical matches for the Ghost Runner simulation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MarketState {
    /// 1. Volatility (The "Temperature")
    /// Ratio of (High-Low) relative to the Close price.
    /// High = Violent/Fast market. Low = Quiet/Consolidation.
    pub volatility_pct: VolatilityPct,

    /// 2. Momentum (The "Velocity")
    /// Percentage change over the last N candles (e.g. 12 candles / 1 hour).
    /// Positive = Rushing up. Negative = Crashing down.
    pub momentum_pct: MomentumPct,

    /// 3. Relative Volume (The "Fuel")
    /// Current Volume divided by Average Volume (e.g. 20-period MA).
    /// > 1.0 = High conviction. < 1.0 = Low liquidity/interest.
    pub relative_volume: VolRatio,
}

impl MarketState {

    /// Calculates the fingerprint for a specific index.
    /// `lookback`: Number of candles to use for Momentum and Volume MA.
    pub fn calculate(ts: &OhlcvTimeSeries, idx: usize, trend_lookback: usize) -> Option<Self> {
        // Safety check
        if idx < trend_lookback || trend_lookback == 0 {
            return None;
        }

        let current = ts.get_candle(idx);
        
        // 1. Volatility (Unchanged)
        let volatility = VolatilityPct::calculate(current.high_price, current.low_price, current.close_price);

        // 2. Momentum (Adaptive - O(1) Lookup)
        let prev_n = ts.get_candle(idx - trend_lookback);
        let momentum = MomentumPct::calculate(current.close_price, prev_n.close_price);

        // 3. Relative Volume (O(1) Lookup)
        // We now read the pre-calculated value directly.
        let rel_vol = ts.relative_volumes.get(idx).copied().expect(&format!("something gone wrong with idx value, {},  in market_state::calculate ", idx));

        Some(Self {
            volatility_pct: volatility,
            momentum_pct: momentum,
            relative_volume: rel_vol,
        })
    }
    


    pub fn similarity_score(&self, other: &Self, config: &SimilaritySettings) -> f64 {
        let d_vol = (*self.volatility_pct - *other.volatility_pct).abs() * *config.weight_volatility;
        let d_mom = (*self.momentum_pct - *other.momentum_pct).abs() * *config.weight_momentum;
        let d_vol_ratio = (*self.relative_volume - *other.relative_volume).abs() * *config.weight_volume;

        d_vol + d_mom + d_vol_ratio
    }
}
