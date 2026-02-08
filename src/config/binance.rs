//! Binance-specific configuration constants and types.

/// Configuration for Binance REST API client
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

/// Configuration for REST API Limits and Weights
pub struct RestLimits {
    /// Default limit for number of klines returned in a single request (1000 is max)
    pub klines_limit: i32,
    /// Weight limit per minute as specified in Binance FAQ
    pub weight_limit_minute: u32,
    /// Weight cost for a single kline API call
    pub kline_call_weight: u32,
    /// Number of parallel threads running delta syncs
    pub concurrent_sync_tasks: usize,
}

/// Configuration for WebSocket Connections
pub struct WsConfig {
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
    /// Maximum number of pairs to load from the file
    pub max_pairs: usize,
    /// Name of the file containing the list of pairs
    pub pairs_filename: &'static str,
    /// List of valid quote assets (used for parsing pair names)
    pub quote_assets: &'static [&'static str],
}

pub const BINANCE: BinanceConfig = BinanceConfig {
    limits: RestLimits {
        klines_limit: 1000,
        weight_limit_minute: 6000,
        kline_call_weight: 2,
        concurrent_sync_tasks: 10,
    },
    ws: WsConfig {
        combined_base_url: "wss://stream.binance.com:9443/stream?streams=",
        max_reconnect_delay_sec: 300, // 5 minutes
        initial_reconnect_delay_sec: 1,
    },
    client: ClientDefaults {
        timeout_ms: 5000,
        retries: 5,
        backoff_ms: 5000,
    },
    max_pairs: 20, // 100,
    pairs_filename: "pairs.txt",
    quote_assets: &["USDT", "USDC", "FDUSD", "BTC", "ETH", "BNB", "EUR", "TRY", "JPY", "BRL", "USD", "USD1", "COP", "BRL", "ARS", "MXN"],
};