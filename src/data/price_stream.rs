#[cfg(not(target_arch = "wasm32"))]
use crate::models::timeseries::LiveCandle;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::Sender;
// Add these inside a cfg block for Native support
#[cfg(not(target_arch = "wasm32"))]
use std::thread;
#[cfg(not(target_arch = "wasm32"))]
use tokio::runtime::Runtime;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use tokio::time::sleep;

// Native imports
#[cfg(not(target_arch = "wasm32"))]
use {
    crate::config::{BINANCE, BinanceApiConfig},
    binance_sdk::{
        config::ConfigurationRestApi,
        spot::{
            SpotRestApi,
            rest_api::{TickerPriceParams, TickerPriceResponse},
        },
    },
    futures::StreamExt,
    std::{
        collections::{HashMap, HashSet},
        sync::{Arc, Mutex},
    },
    tokio_tungstenite::{connect_async, tungstenite::Message},
};

// WASM imports
#[cfg(target_arch = "wasm32")]
use {serde_json, std::collections::HashMap};

// WASM + debug imports
#[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
use crate::config::DF;

#[cfg(target_arch = "wasm32")]
const DEMO_PRICES_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/",
    crate::kline_data_dir!(), // Expands to "kline_data"
    "/",
    crate::demo_prices_file!() // Expands to "demo_prices.json"
));

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Connecting,
    Disconnected,
}

/// Manages WebSocket connections to Binance for live price updates
/// Subscribes to all pairs upfront with automatic reconnection
#[cfg(not(target_arch = "wasm32"))]
pub struct PriceStreamManager {
    // Map of symbol -> current price
    prices: Arc<Mutex<HashMap<String, f64>>>,
    // Map of symbol -> connection status
    connection_status: Arc<Mutex<HashMap<String, ConnectionStatus>>>,
    subscribed_symbols: Arc<Mutex<Vec<String>>>,
    // Suspension flag - when true, price updates are ignored
    suspended: Arc<Mutex<bool>>,
    candle_tx: Option<Sender<LiveCandle>>,
}

// ... build_combined_stream_url ...
#[cfg(not(target_arch = "wasm32"))]
fn build_combined_stream_url(symbols: &[String]) -> String {
    use crate::config::constants;

    let interval = crate::utils::TimeUtils::interval_to_string(constants::INTERVAL_WIDTH_MS);

    // CHANGE: Only subscribe to kline
    let streams: Vec<String> = symbols
        .iter()
        .map(|symbol| {
            let s = symbol.to_lowercase();
            format!("{}@kline_{}", s, interval)
        })
        .collect();

    format!("{}{}", BINANCE.ws.combined_base_url, streams.join("/"))
}

#[cfg(not(target_arch = "wasm32"))]
impl PriceStreamManager {
    pub fn new() -> Self {
        Self {
            prices: Arc::new(Mutex::new(HashMap::new())),
            connection_status: Arc::new(Mutex::new(HashMap::new())),
            subscribed_symbols: Arc::new(Mutex::new(Vec::new())),
            suspended: Arc::new(Mutex::new(false)),
            candle_tx: None,
        }
    }

    /// Get the current live price for a symbol
    pub fn get_price(&self, symbol: &str) -> Option<f64> {
        let symbol_lower = symbol.to_lowercase();
        self.prices.lock().unwrap().get(&symbol_lower).copied()
    }

    pub fn set_candle_sender(&mut self, tx: Sender<LiveCandle>) {
        self.candle_tx = Some(tx);
    }

    /// Suspend price updates (for simulation mode)
    pub fn suspend(&self) {
        *self.suspended.lock().unwrap() = true;
        #[cfg(debug_assertions)]
        if DF.log_simulation_events {
            log::info!("🔇 WebSocket price updates suspended");
        }
    }

    /// Resume price updates (exit simulation mode)
    pub fn resume(&self) {
        *self.suspended.lock().unwrap() = false;
        #[cfg(debug_assertions)]
        if DF.log_simulation_events {
            log::info!("🔊 WebSocket price updates resumed");
        }
    }

    /// Check if price updates are suspended
    pub fn is_suspended(&self) -> bool {
        *self.suspended.lock().unwrap()
    }

    /// Get overall connection health (percentage of connected streams)
    pub fn connection_health(&self) -> f64 {
        let status_map = self.connection_status.lock().unwrap();
        if status_map.is_empty() {
            return 0.0;
        }
        let connected = status_map
            .values()
            .filter(|&&s| s == ConnectionStatus::Connected)
            .count();
        (connected as f64 / status_map.len() as f64) * 100.0
    }

    pub fn subscribe_all(&self, symbols: Vec<String>) {
        let symbols_lower: Vec<String> = symbols.iter().map(|s| s.to_lowercase()).collect();
        let mut subscribed = self.subscribed_symbols.lock().unwrap();

        // Avoid re-subscribing if list matches (optional optimization, but good practice)
        // For now, we assume a fresh start or simple overwrite as per original code.
        *subscribed = symbols_lower.clone();

        // Clone Arcs to move into the background thread
        let prices_arc = self.prices.clone();
        let status_arc = self.connection_status.clone();
        let suspended_arc = self.suspended.clone();

        // NEW: Clone the candle sender
        let candle_tx = self.candle_tx.clone();

        // Clone symbol list for the warmup call
        let symbols_for_warmup = symbols_lower.clone();

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Spawn a dedicated thread for the runtime
            thread::spawn(move || {
                let rt = Runtime::new().expect("Failed to create runtime");
                rt.block_on(async move {
                    // 1. PULL (Batch Snapshot)
                    warm_up_prices(prices_arc.clone(), &symbols_for_warmup).await;

                    // 2. PUSH (Live Updates)
                    // We now pass 'candle_tx' down the chain
                    run_combined_price_stream_with_reconnect(
                        &symbols_lower,
                        prices_arc,
                        status_arc,
                        suspended_arc,
                        candle_tx, // <--- PASSED HERE
                    )
                    .await;
                });
            });
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for PriceStreamManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
pub struct PriceStreamManager {
    prices: HashMap<String, f64>,
}

#[cfg(target_arch = "wasm32")]
impl Default for PriceStreamManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_arch = "wasm32")]
impl PriceStreamManager {
    pub fn new() -> Self {
        let parsed: HashMap<String, f64> =
            serde_json::from_str(DEMO_PRICES_JSON).unwrap_or_default();
        let mut prices = HashMap::new();
        for (symbol, price) in parsed {
            prices.insert(symbol.to_lowercase(), price);
        }
        Self { prices }
    }

    pub fn get_price(&self, symbol: &str) -> Option<f64> {
        let symbol_lower = symbol.to_lowercase();
        self.prices.get(&symbol_lower).copied()
    }

    pub fn suspend(&self) {}

    pub fn resume(&self) {}

    pub fn is_suspended(&self) -> bool {
        true
    }

    pub fn connection_health(&self) -> f64 {
        100.0
    }

    pub fn subscribe_all(&self, _symbols: Vec<String>) {}
}

#[cfg(not(target_arch = "wasm32"))]
async fn run_combined_price_stream_with_reconnect(
    symbols: &[String],
    prices_arc: Arc<Mutex<HashMap<String, f64>>>,
    status_arc: Arc<Mutex<HashMap<String, ConnectionStatus>>>,
    suspended_arc: Arc<Mutex<bool>>,
    candle_tx: Option<Sender<LiveCandle>>, // <--- NEW ARGUMENT
) {
    let mut reconnect_delay = BINANCE.ws.initial_reconnect_delay_sec;
    let url = build_combined_stream_url(symbols); // Ensure your build_combined_stream_url includes klines now!

    loop {
        // Update status to connecting
        {
            let mut status_map = status_arc.lock().unwrap();
            for symbol in symbols {
                status_map.insert(symbol.clone(), ConnectionStatus::Connecting);
            }
        }

        #[cfg(debug_assertions)]
        if DF.log_price_stream_updates {
            log::info!("Attempting connection to Binance Stream...");
        }
        match run_combined_price_stream(
            symbols,
            &url,
            prices_arc.clone(),
            status_arc.clone(),
            suspended_arc.clone(),
            candle_tx.clone(), // <--- PASS IT DOWN
        )
        .await
        {
            Ok(_) => {
                log::warn!("WebSocket closed normally. Reconnecting...");
                reconnect_delay = BINANCE.ws.initial_reconnect_delay_sec;
            }
            Err(e) => {
                log::error!(
                    "WebSocket connection failed: {}. Retrying in {}s...",
                    e,
                    reconnect_delay
                );
            }
        }

        // Update status to disconnected
        {
            let mut status_map = status_arc.lock().unwrap();
            for symbol in symbols {
                status_map.insert(symbol.clone(), ConnectionStatus::Disconnected);
            }
        }

        sleep(Duration::from_secs(reconnect_delay)).await;
        reconnect_delay = (reconnect_delay * 2).min(BINANCE.ws.max_reconnect_delay_sec);
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn run_combined_price_stream(
    symbols: &[String],
    url: &str,
    prices_arc: Arc<Mutex<HashMap<String, f64>>>,
    status_arc: Arc<Mutex<HashMap<String, ConnectionStatus>>>,
    suspended_arc: Arc<Mutex<bool>>,
    candle_tx: Option<Sender<LiveCandle>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (ws_stream, _) = connect_async(url).await?;

    // Update status to connected
    {
        let mut status_map = status_arc.lock().unwrap();
        for symbol in symbols {
            status_map.insert(symbol.clone(), ConnectionStatus::Connected);
        }
    }

    let (_write, mut read) = ws_stream.split();

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    // Check Event Type "e"
                    if let Some(event_type) = v["data"]["e"].as_str() {
                        match event_type {
                            "kline" => {
                                // 1. SEND TO ENGINE (History/Heartbeat)
                                if let Some(tx) = &candle_tx {
                                    parse_and_send_kline(&v["data"], tx);
                                }

                                // 2. UPDATE LIVE PRICE CACHE (UI Display)
                                // We use the current candle close ("c") as the live price
                                if let Some(k) = v["data"].get("k") {
                                    if let Some(c_str) = k["c"].as_str() {
                                        if let Ok(price) = c_str.parse::<f64>() {
                                            // Check Suspension
                                            let is_suspended = *suspended_arc.lock().unwrap();
                                            if !is_suspended {
                                                let symbol = v["data"]["s"]
                                                    .as_str()
                                                    .unwrap_or("")
                                                    .to_lowercase();

                                                // Update Map
                                                prices_arc
                                                    .lock()
                                                    .unwrap()
                                                    .insert(symbol.clone(), price);

                                                #[cfg(debug_assertions)]
                                                if DF.log_price_stream_updates {
                                                    log::info!(
                                                        "[kline-tick] {} -> {:.6}",
                                                        symbol,
                                                        price
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // Ignore other events (like 24hrTicker if any sneak in)
                            _ => {}
                        }
                    }
                } else {
                    log::warn!("⚠️ Failed to parse WebSocket JSON message");
                }
            }
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => {
                break;
            }
            Err(e) => {
                log::error!("WebSocket error: {}", e);
                return Err(e.into());
            }
            _ => {}
        }
    }

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
async fn warm_up_prices(prices_arc: Arc<Mutex<HashMap<String, f64>>>, symbols: &[String]) {
    #[cfg(debug_assertions)]
    if DF.log_price_stream_updates {
        log::info!(">>> PriceStream: Warming up price cache via REST API...");
    }
    let config = BinanceApiConfig::default();

    let rest_conf = ConfigurationRestApi::builder()
        .timeout(config.timeout_ms)
        .retries(config.retries)
        .backoff(config.backoff_ms)
        .build()
        .expect("Failed to build Binance REST config");

    let client = SpotRestApi::production(rest_conf);

    let params = TickerPriceParams {
        symbol: None,
        symbols: None,
        symbol_status: None,
    };

    // 1. Make the Request
    match client.ticker_price(params).await {
        Ok(response) => {
            // 2. Await the data extraction (It returns a Result<TickerPriceResponse>)
            match response.data().await {
                Ok(ticker_data) => {
                    match ticker_data {
                        // 3. Match the Vector Variant
                        TickerPriceResponse::TickerPriceResponse2(all_tickers) => {
                            let mut p_lock = prices_arc.lock().unwrap();
                            let mut _updated_count = 0;

                            let wanted_set: HashSet<String> =
                                symbols.iter().map(|s| s.to_lowercase()).collect();

                            for ticker in all_tickers {
                                // 4. Safely handle Option fields (symbol/price might be None)
                                if let (Some(s), Some(p)) = (&ticker.symbol, &ticker.price) {
                                    let symbol_lower = s.to_lowercase();

                                    if wanted_set.contains(&symbol_lower) {
                                        let price = p.parse::<f64>().unwrap_or(0.0);
                                        if price > 0.0 {
                                            p_lock.insert(symbol_lower, price);
                                            _updated_count += 1;
                                        }
                                    }
                                }
                            }
                            #[cfg(debug_assertions)]
                            if DF.log_price_stream_updates {
                                log::info!(
                                    ">>> PriceStream: Warmup complete. Updated {}/{} pairs.",
                                    _updated_count,
                                    symbols.len()
                                );
                            }
                        }
                        TickerPriceResponse::TickerPriceResponse1(_) => {
                            log::warn!(
                                ">>> PriceStream: Unexpected 'Single' response type during batch warmup."
                            );
                        }
                        _ => {
                            log::warn!(">>> PriceStream: Unexpected 'Other' response type.");
                        }
                    }
                }
                Err(e) => {
                    log::error!(">>> PriceStream: Failed to parse response data: {:?}", e);
                }
            }
        }
        Err(e) => {
            log::error!(">>> PriceStream: Warmup request failed: {:?}", e);
        }
    }
}

// Helper Function
#[cfg(not(target_arch = "wasm32"))]
fn parse_and_send_kline(data: &serde_json::Value, tx: &Sender<LiveCandle>) {
    // "k" is the kline object in the payload
    let k = &data["k"];
    if k.is_null() {
        return;
    }

    // "x": true means the candle is closed. We generally want all updates (open and closed),
    // but the Engine handles the logic of update vs append.
    let is_closed = k["x"].as_bool().unwrap_or(false);

    let symbol = data["s"].as_str().unwrap_or("").to_string();
    let close = k["c"].as_str().unwrap_or("0").parse().unwrap_or(0.0);

    let candle = LiveCandle {
        symbol,
        open_time: k["t"].as_i64().unwrap_or(0),
        open: k["o"].as_str().unwrap_or("0").parse().unwrap_or(0.0),
        high: k["h"].as_str().unwrap_or("0").parse().unwrap_or(0.0),
        low: k["l"].as_str().unwrap_or("0").parse().unwrap_or(0.0),
        close,
        volume: k["v"].as_str().unwrap_or("0").parse().unwrap_or(0.0),
        quote_vol: k["q"].as_str().unwrap_or("0").parse().unwrap_or(0.0),
        is_closed,
    };

    // Send to Engine. If receiver is dropped, this fails silently (ok).
    let _ = tx.send(candle);
}
