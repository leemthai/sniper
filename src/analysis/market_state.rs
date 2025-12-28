use crate::models::OhlcvTimeSeries;
use crate::config::SimilaritySettings;

/// A normalized "Fingerprint" of the market conditions at a specific moment in time.
/// Used to find historical matches for the Ghost Runner simulation.
#[derive(Debug, Clone, Copy)]
pub struct MarketState {
    /// 1. Volatility (The "Temperature")
    /// Ratio of (High-Low) relative to the Close price.
    /// High = Violent/Fast market. Low = Quiet/Consolidation.
    pub volatility_pct: f64,

    /// 2. Momentum (The "Velocity")
    /// Percentage change over the last N candles (e.g. 12 candles / 1 hour).
    /// Positive = Rushing up. Negative = Crashing down.
    pub momentum_pct: f64,

    /// 3. Relative Volume (The "Fuel")
    /// Current Volume divided by Average Volume (e.g. 20-period MA).
    /// > 1.0 = High conviction. < 1.0 = Low liquidity/interest.
    pub relative_volume: f64,
}

impl MarketState {

    /// Helper for debugging: Returns the contribution of each factor
    pub fn debug_score_components(&self, other: &Self, config: &SimilaritySettings) -> (f64, f64, f64, f64) {
        let d_vol = (self.volatility_pct - other.volatility_pct).abs() * config.weight_volatility;
        let d_mom = (self.momentum_pct - other.momentum_pct).abs() * config.weight_momentum;
        let d_vol_ratio = (self.relative_volume - other.relative_volume).abs() * config.weight_volume;
        (d_vol + d_mom + d_vol_ratio, d_vol, d_mom, d_vol_ratio)
    }


    /// Calculates the fingerprint for a specific index.
    /// `lookback`: Number of candles to use for Momentum and Volume MA.
    pub fn calculate(ts: &OhlcvTimeSeries, idx: usize, trend_lookback: usize) -> Option<Self> {
        // Safety check
        if idx < trend_lookback || trend_lookback == 0 {
            return None;
        }

        let current = ts.get_candle(idx);
        
        // 1. Volatility (Unchanged)
        let volatility = if current.close_price > 0.0 {
            (current.high_price - current.low_price) / current.close_price
        } else {
            0.0
        };

        // 2. Momentum (Adaptive - O(1) Lookup)
        let prev_n = ts.get_candle(idx - trend_lookback);
        let momentum = if prev_n.close_price > 0.0 {
            (current.close_price - prev_n.close_price) / prev_n.close_price
        } else {
            0.0
        };

        // 3. Relative Volume (FIXED SHORT LOOKBACK - O(N) Loop)
        // We decouple this from trend_lookback to prevent the "Computational Bomb".
        // 20 candles (approx 1.5 hours on 5m) is standard for Rel Vol.
        let vol_lookback = 20; 
        
        // Safety for volume loop
        if idx < vol_lookback { return None; }

        let mut vol_sum = 0.0;
        for i in 0..vol_lookback {
            vol_sum += ts.base_asset_volumes[idx - i];
        }
        let avg_vol = vol_sum / vol_lookback as f64;
        
        let rel_vol = if avg_vol > 0.0 {
            current.base_asset_volume / avg_vol
        } else {
            0.0
        };

        Some(Self {
            volatility_pct: volatility,
            momentum_pct: momentum,
            relative_volume: rel_vol,
        })
    }
    


    pub fn similarity_score(&self, other: &Self, config: &SimilaritySettings) -> f64 {
        let d_vol = (self.volatility_pct - other.volatility_pct).abs() * config.weight_volatility;
        let d_mom = (self.momentum_pct - other.momentum_pct).abs() * config.weight_momentum;
        let d_vol_ratio = (self.relative_volume - other.relative_volume).abs() * config.weight_volume;

        d_vol + d_mom + d_vol_ratio
    }
}
