//! Binance-specific configuration constants and types.

/// Maximum number of simultaneous Binance API calls allowed per batch
/// Theoretical limit is 1000, but 500 is safer for rate limiting
pub const SIMULTANEOUS_KLINE_CALLS_CEILING: usize = 500;

/// Maximum total number of pair/interval combinations to query from Binance API
/// This limits the total lookups regardless of permutation size
pub const MAX_BN_KLINES_LOOKUPS_TOTAL: usize = 1000;

/// Weight limit per minute as specified in Binance FAQ
pub const WEIGHT_LIMIT_MINUTE: u32 = 6000;

/// Weight cost for a single kline API call
pub const KLINE_CALL_WEIGHT: u32 = 2;

/// Maximum age of cached kline data before refetching from API (in seconds)
/// Default: 24 hours
pub const KLINE_ACCEPTABLE_AGE_SECONDS: i64 = 60 * 60 * 24;

/// WebSocket base URL for Binance streaming API (single stream)
pub const BINANCE_WS_BASE: &str = "wss://stream.binance.com:9443/ws";

/// WebSocket base URL for Binance combined streaming API
pub const BINANCE_WS_COMBINED_BASE: &str = "wss://stream.binance.com:9443/stream?streams=";

/// Maximum reconnection delay for WebSocket connections (in seconds)
/// Capped at 5 minutes to prevent excessive wait times
pub const MAX_RECONNECT_DELAY_SECS: u64 = 300;

/// Initial reconnection delay for WebSocket connections (in seconds)
pub const INITIAL_RECONNECT_DELAY_SECS: u64 = 1;

/// Configuration constants that vary between debug/release
pub mod debug {
    /// Interval for debug prints in development
    pub const DEBUG_PRINT_INTERVAL: u32 = 10;
}

/// Configuration for Binance REST API client
pub struct BinanceApiConfig {
    pub timeout_ms: u64,
    pub retries: u32,
    pub backoff_ms: u64,
}

impl Default for BinanceApiConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 5000,
            retries: 5,
            backoff_ms: 5000,
        }
    }
}
