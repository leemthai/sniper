use crate::config::{BaseVol, QuoteVol, OpenPrice, HighPrice, LowPrice, ClosePrice, PriceLike, Price};

// Define the CandleType enum
#[derive(Debug, PartialEq)]
pub enum CandleType {
    Bullish,
    Bearish,
}

// Define the Candle struct with all its properties
pub struct Candle {
    pub timestamp_ms: i64,

    pub open_price: OpenPrice,
    pub high_price: HighPrice,
    pub low_price: LowPrice,
    pub close_price: ClosePrice,

    pub base_asset_volume: BaseVol,
    pub quote_asset_volume: QuoteVol,
}

// Implement methods for the Candle struct
impl Candle {
    // A constructor for convenience
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

    // A method to determine the type of candle
    pub fn get_type(&self) -> CandleType {
        if Price::from(self.close_price) >= Price::from(self.open_price) {
            CandleType::Bullish
        } else {
            CandleType::Bearish
        }
    }

    // Returns the low and high of the candle body as a tuple
    pub fn body_range(&self) -> (f64, f64) {
        match self.get_type() {
            CandleType::Bullish => (self.open_price.value(), self.close_price.value()),
            CandleType::Bearish => (self.close_price.value(), self.open_price.value()),
        }
    }

    // Calculates the low of the bottom wick.
    pub fn low_wick_low(&self) -> f64 {
        self.low_price.value()
    }

    // Calculates the high of the bottom wick.
    pub fn low_wick_high(&self) -> f64 {
        self.body_range().0
    }

    // Calculates the low of the top wick.
    pub fn high_wick_low(&self) -> f64 {
        self.body_range().1
    }

    // Calculates the high of the top wick.
    pub fn high_wick_high(&self) -> f64 {
        self.high_price.value()
    }
}
