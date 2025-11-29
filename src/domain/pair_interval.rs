use serde::{Deserialize, Serialize};

use crate::utils::TimeUtils;

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct PairInterval {
    pub name: String,
    pub interval_ms: i64,
}

impl PairInterval {
    pub fn get_base(text: &str) -> Option<&str> {
        let quote = Self::get_quote(text)?;
        // `strip_suffix` returns `None` if the suffix is not found.
        // If get_quote returned Some(quote), strip_suffix can still return None
        // if the quote is not at the end (e.g., malformed pair name).
        text.strip_suffix(quote)
    }

    // Finds the trading quote at the end of the pair name and returns it.
    // Returns None if no matching quote is found.
    pub fn get_quote(text: &str) -> Option<&str> {
        static PAIR_QUOTES: &[&str] = &["USDT", "USDC", "FDUSD", "BTC", "ETH"];
        PAIR_QUOTES
            .iter()
            .find(|&&ext| text.ends_with(ext))
            .copied()
    }

    /* # Where we use base_asset and quote_asset in the app:
    1. Creating permutations of `base` and `quote` to easily create lots of pairs
    2. (No) -  BN API takes a single symbol as well.
    3. BN does actually output details in kline results denominated in either base or quote, thus::
        base_asset_volumes:
        quote_asset_volumes:
      So use get_base_and_quote() to split a string up into its constituent parts  */
    pub fn get_base_and_quote(text: &str) -> Option<(&str, &str)> {
        let base = Self::get_base(text)?;
        let quote = Self::get_quote(text)?;
        Some((base, quote))
    }

    // Split the name into base and quote assets.
    pub fn split_pair_name(pair_name: &str) -> (&str, &str) {
        match Self::get_base_and_quote(pair_name) {
            Some((base, quote)) => (base, quote),
            None => ("Invalid", "Name"),
        }
    }

    // The name we pass into the Binance API (not necessarily display name)
    pub fn bn_name(&self) -> &str {
        &self.name
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

// This appears to be unused.... hard to show in Egui coz selected_pair is Option<String>. It does not include the Interval at all..... yet.
impl std::fmt::Display for PairInterval {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let base = Self::get_base(&self.name).unwrap_or("UNKNOWN_BASE");
        let quote = Self::get_quote(&self.name).unwrap_or("UNKNOWN_QUOTE");
        write!(
            f,
            "Base: {}, Quote: {}, full: {}, Interval: {}ms (or {}) ",
            base,
            quote,
            self.name(),
            self.interval_ms,
            TimeUtils::interval_ms_to_string(self.interval_ms)
        )
    }
}
