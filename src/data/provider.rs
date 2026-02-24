use {
    crate::{
        data::{GlobalRateLimiter, load_klines},
        domain::{Candle, PairInterval},
    },
    anyhow::Result,
    async_trait::async_trait,
};

#[async_trait]
pub trait MarketDataProvider: Send + Sync {
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

        let result = load_klines(pair_interval, start_time, self.limiter.clone()).await?;

        let candles: Vec<Candle> = result
            .klines
            .into_iter()
            .map(|bn_kline| bn_kline.into())
            .collect();

        Ok(candles)
    }
}
