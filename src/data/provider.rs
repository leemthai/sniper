use anyhow::Result;
use async_trait::async_trait;
use crate::domain::candle::Candle;

// Native-only imports
#[cfg(not(target_arch = "wasm32"))]
use {
    crate::data::rate_limiter::GlobalRateLimiter,
    crate::data::timeseries::bn_kline,
    crate::domain::pair_interval::PairInterval,
};

/// Abstract interface for fetching market data.
#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    /// Fetch candles for a pair starting from a specific timestamp.
    async fn fetch_candles(
        &self,
        pair: &str,
        interval_ms: i64,
        start_time: Option<i64>,
    ) -> Result<Vec<Candle>>;

    fn id(&self) -> &'static str;
}

// ============================================================================
// NATIVE IMPLEMENTATION (Binance)
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
impl BinanceProvider {
    pub fn new(limiter: GlobalRateLimiter) -> Self {
        Self { limiter }
    }
}

#[cfg(not(target_arch = "wasm32"))]
// Update Struct
pub struct BinanceProvider {
    limiter: GlobalRateLimiter,
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl MarketDataProvider for BinanceProvider {
    fn id(&self) -> &'static str {
        "Binance"
    }

    async fn fetch_candles(
        &self,
        pair: &str,
        interval_ms: i64,
        start_time: Option<i64>,
    ) -> Result<Vec<Candle>> {
        // We can import these here safely because this whole block is guarded

        let pair_interval = PairInterval {
            name: pair.to_string(),
            interval_ms,
        };

        // Call the legacy loader (modified to accept start_time)
        let result =
            bn_kline::load_klines(pair_interval, 1, start_time, self.limiter.clone()).await?;

        // Convert using the From impl
        let candles: Vec<Candle> = result
            .klines
            .into_iter()
            .map(|bn_kline| bn_kline.into())
            .collect();

        Ok(candles)
    }
}

// ============================================================================
// WASM IMPLEMENTATION (Dummy / Static)
// ============================================================================

#[cfg(target_arch = "wasm32")]
pub struct WasmProvider;

#[cfg(target_arch = "wasm32")]
#[async_trait]
impl MarketDataProvider for WasmProvider {
    fn id(&self) -> &'static str {
        "WasmStatic"
    }

    async fn fetch_candles(
        &self,
        _pair: &str,
        _interval_ms: i64,
        _start_time: Option<i64>,
    ) -> Result<Vec<Candle>> {
        // In the future, we could fetch from a static URL here.
        // For now, return empty or error? Empty is safer.
        Ok(Vec::new())
    }
}
