// Define the CandleType enum
#[derive(Debug, PartialEq)]
pub enum CandleType {
    Bullish,
    Bearish,
}

// Define the Candle struct with all its properties
pub struct Candle {
    pub open_price: f64,
    pub high_price: f64,
    pub low_price: f64,
    pub close_price: f64,

    pub base_volume: f64,
    #[allow(dead_code)] // May be useful for future analysis
    pub quote_volume: f64,
}

// Implement methods for the Candle struct
impl Candle {
    // A constructor for convenience
    pub fn new(
        open_price: f64,
        close_price: f64,
        low_price: f64,
        high_price: f64,
        base_volume: f64,
        quote_volume: f64,
    ) -> Self {
        Candle {
            open_price,
            high_price,
            low_price,
            close_price,
            base_volume,
            quote_volume,
        }
    }

    // A method to determine the type of candle
    pub fn get_type(&self) -> CandleType {
        if self.close_price >= self.open_price {
            CandleType::Bullish
        } else {
            CandleType::Bearish
        }
    }

    // Returns the low and high of the candle body as a tuple
    pub fn body_range(&self) -> (f64, f64) {
        match self.get_type() {
            CandleType::Bullish => (self.open_price, self.close_price),
            CandleType::Bearish => (self.close_price, self.open_price),
        }
    }

    // Calculates the low of the bottom wick.
    pub fn low_wick_low(&self) -> f64 {
        self.low_price
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
        self.high_price
    }
}
