use {
    crate::{
        data::{GlobalRateLimiter, load_klines},
        domain::{Candle, PairInterval},
    },
    anyhow::Result,
    async_trait::async_trait,
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
}

impl BinanceProvider {
    pub fn new(limiter: GlobalRateLimiter) -> Self {
        Self { limiter }
    }
}

// Update Struct
pub struct BinanceProvider {
    limiter: GlobalRateLimiter,
}

#[async_trait]
impl MarketDataProvider for BinanceProvider {
    async fn fetch_candles(
        &self,
        pair: &str,
        interval_ms: i64,
        start_time: Option<i64>,
    ) -> Result<Vec<Candle>> {
        let pair_interval = PairInterval {
            name: pair.into(),
            interval_ms,
        };

        // Call the legacy loader (modified to accept start_time)
        let result = load_klines(pair_interval, 1, start_time, self.limiter.clone()).await?;

        // Convert using the From impl
        let candles: Vec<Candle> = result
            .klines
            .into_iter()
            .map(|bn_kline| bn_kline.into())
            .collect();

        Ok(candles)
    }
}
