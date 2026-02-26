use crate::config::{
    BaseVol, ClosePrice, HighPrice, LowPrice, OpenPrice, Price, PriceLike, QuoteVol,
};

#[derive(Debug, PartialEq)]
pub(crate) enum CandleType {
    Bullish,
    Bearish,
}

pub struct Candle {
    pub timestamp_ms: i64,

    pub open_price: OpenPrice,
    pub high_price: HighPrice,
    pub low_price: LowPrice,
    pub close_price: ClosePrice,

    pub base_asset_volume: BaseVol,
    pub quote_asset_volume: QuoteVol,
}

impl Candle {
    pub fn new(
        timestamp_ms: i64,
        open: OpenPrice,
        high: HighPrice,
        low: LowPrice,
        close: ClosePrice,
        base_vol: BaseVol,
        quote_vol: QuoteVol,
    ) -> Self {
        Candle {
            timestamp_ms,
            open_price: open,
            high_price: high,
            low_price: low,
            close_price: close,
            base_asset_volume: base_vol,
            quote_asset_volume: quote_vol,
        }
    }

    fn get_type(&self) -> CandleType {
        if Price::from(self.close_price) >= Price::from(self.open_price) {
            CandleType::Bullish
        } else {
            CandleType::Bearish
        }
    }

    fn body_range(&self) -> (f64, f64) {
        match self.get_type() {
            CandleType::Bullish => (self.open_price.value(), self.close_price.value()),
            CandleType::Bearish => (self.close_price.value(), self.open_price.value()),
        }
    }

    pub(crate) fn low_wick_low(&self) -> f64 {
        self.low_price.value()
    }

    pub(crate) fn low_wick_high(&self) -> f64 {
        self.body_range().0
    }

    pub(crate) fn high_wick_low(&self) -> f64 {
        self.body_range().1
    }

    pub(crate) fn high_wick_high(&self) -> f64 {
        self.high_price.value()
    }
}
