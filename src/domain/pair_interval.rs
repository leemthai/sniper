use {
    crate::{config::BINANCE_QUOTE_ASSETS, utils::TimeUtils},
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct PairInterval {
    pub name: String,
    pub interval_ms: i64,
}

impl PairInterval {
    pub(crate) fn get_base(text: &str) -> Option<&str> {
        let quote = Self::get_quote(text)?;
        text.strip_suffix(quote)
    }

    pub(crate) fn get_quote(text: &str) -> Option<&str> {
        BINANCE_QUOTE_ASSETS
            .iter()
            .find(|&&ext| text.ends_with(ext))
            .copied()
    }

    // The name we pass into the Binance API (not necessarily display name)
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn bn_name(&self) -> &str {
        &self.name
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }
}

impl std::fmt::Display for PairInterval {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let base = Self::get_base(&self.name).unwrap_or("UNKNOWN_BASE");
        let quote = Self::get_quote(&self.name).unwrap_or("UNKNOWN_QUOTE");
        write!(
            f,
            "Base: {}, Quote: {}, full: {}, Interval: {}ms (or {}) ",
            base,
            quote,
            self.name,
            self.interval_ms,
            TimeUtils::interval_to_string(self.interval_ms)
        )
    }
}
