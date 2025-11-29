//! Binance-specific configuration constants and types.

/// Configuration for Binance REST API client
/// (This is the runtime struct used by your Http Client)
pub struct BinanceApiConfig {
    pub timeout_ms: u64,
    pub retries: u32,
    pub backoff_ms: u64,
}

impl Default for BinanceApiConfig {
    fn default() -> Self {
        Self {
            timeout_ms: BINANCE.client.timeout_ms,
            retries: BINANCE.client.retries,
            backoff_ms: BINANCE.client.backoff_ms,
        }
    }
}

// Binance-specific configuration constants and types.

/// Configuration for REST API Limits and Weights
pub struct RestLimits {
    /// Default limit for number of klines returned in a single request
    pub klines_limit: i32,
    /// Maximum number of simultaneous Binance API calls allowed per batch
    pub simultaneous_calls_ceiling: usize,
    /// Maximum total number of pair/interval combinations to query
    pub max_lookups_total: usize,
    /// Weight limit per minute as specified in Binance FAQ
    pub weight_limit_minute: u32,
    /// Weight cost for a single kline API call
    pub kline_call_weight: u32,
    /// Maximum age of cached kline data (seconds)
    pub kline_acceptable_age_sec: i64,
}

/// Configuration for WebSocket Connections
pub struct WsConfig {
    /// WebSocket base URL for Binance streaming API (single stream)
    pub base_url: &'static str,
    /// WebSocket base URL for Binance combined streaming API
    pub combined_base_url: &'static str,
    /// Maximum reconnection delay (seconds)
    pub max_reconnect_delay_sec: u64,
    /// Initial reconnection delay (seconds)
    pub initial_reconnect_delay_sec: u64,
}

/// Default values for the Rest Client
pub struct ClientDefaults {
    pub timeout_ms: u64,
    pub retries: u32,
    pub backoff_ms: u64,
}

/// The Master Configuration Struct
pub struct BinanceConfig {
    pub limits: RestLimits,
    pub ws: WsConfig,
    pub client: ClientDefaults,
    /// Interval for debug prints in development
    pub debug_print_interval: u32,
    pub max_pairs: usize,
}

pub const BINANCE: BinanceConfig = BinanceConfig {
    limits: RestLimits {
        klines_limit: 1000,
        // Theoretical limit is 1000, but 500 is safer for rate limiting
        simultaneous_calls_ceiling: 500,
        max_lookups_total: 1000,
        weight_limit_minute: 6000,
        kline_call_weight: 2,
        // 24 hours (60 * 60 * 24)
        kline_acceptable_age_sec: 86_400,
    },
    ws: WsConfig {
        base_url: "wss://stream.binance.com:9443/ws",
        combined_base_url: "wss://stream.binance.com:9443/stream?streams=",
        max_reconnect_delay_sec: 300, // 5 minutes
        initial_reconnect_delay_sec: 1,
    },
    client: ClientDefaults {
        timeout_ms: 5000,
        retries: 5,
        backoff_ms: 5000,
    },
    debug_print_interval: 10,
    max_pairs: 20,
};
