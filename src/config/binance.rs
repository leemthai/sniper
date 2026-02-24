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

/// REST constraints: 1000 klines, weight budget, call costs, and sync concurrency.
pub struct RestLimits {
    pub klines_limit: i32,
    pub weight_limit_minute: u32,
    pub kline_call_weight: u32,
    pub concurrent_sync_tasks: usize,
}

pub struct WsConfig {
    pub combined_base_url: &'static str,
    pub max_reconnect_delay_sec: u64,
    pub initial_reconnect_delay_sec: u64,
}

pub struct ClientDefaults {
    pub timeout_ms: u64,
    pub retries: u32,
    pub backoff_ms: u64,
}

pub const BINANCE_PAIRS_FILENAME: &str = "pairs.txt";
pub const BINANCE_MAX_PAIRS: usize = 20;

pub struct BinanceConfig {
    pub limits: RestLimits,
    pub ws: WsConfig,
    pub client: ClientDefaults,
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
};
