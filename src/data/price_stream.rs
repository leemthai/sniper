#[cfg(not(target_arch = "wasm32"))]
use crate::config::BINANCE;
#[cfg(all(debug_assertions, not(target_arch = "wasm32")))] // Not needed for WASM
use crate::config::DEBUG_FLAGS;
#[cfg(not(target_arch = "wasm32"))]
use futures::StreamExt;
#[cfg(not(target_arch = "wasm32"))]
use serde::Deserialize;
#[cfg(target_arch = "wasm32")]
use serde_json;
#[cfg(target_arch = "wasm32")]
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use tokio_tungstenite::{connect_async, tungstenite::Message};
#[cfg(target_arch = "wasm32")]
const DEMO_PRICES_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/kline_data/demo_prices.json"
));

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Deserialize)]
struct MiniTickerData {
    #[serde(rename = "c")]
    close_price: String,
    #[serde(rename = "s")]
    symbol: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Deserialize)]
struct CombinedStreamMessage {
    #[serde(rename = "stream")]
    _stream: String,
    data: MiniTickerData,
}

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
}

#[cfg(not(target_arch = "wasm32"))]
impl PriceStreamManager {
    pub fn new() -> Self {
        Self {
            prices: Arc::new(Mutex::new(HashMap::new())),
            connection_status: Arc::new(Mutex::new(HashMap::new())),
            subscribed_symbols: Arc::new(Mutex::new(Vec::new())),
            suspended: Arc::new(Mutex::new(false)),
        }
    }

    /// Get the current live price for a symbol
    pub fn get_price(&self, symbol: &str) -> Option<f64> {
        let symbol_lower = symbol.to_lowercase();
        self.prices.lock().unwrap().get(&symbol_lower).copied()
    }

    /// Suspend price updates (for simulation mode)
    pub fn suspend(&self) {
        *self.suspended.lock().unwrap() = true;
        #[cfg(debug_assertions)]
        if DEBUG_FLAGS.print_simulation_events {
            log::info!("🔇 WebSocket price updates suspended");
        }
    }

    /// Resume price updates (exit simulation mode)
    pub fn resume(&self) {
        *self.suspended.lock().unwrap() = false;
        #[cfg(debug_assertions)]
        if DEBUG_FLAGS.print_simulation_events {
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
        if *subscribed == symbols_lower {
            return; 
        }
        
        log::info!(">>> PriceStream: Requesting {} pairs: {:?}", symbols_lower.len(), symbols_lower);

        *subscribed = symbols_lower.clone();
        
        // Clone Arcs to move into the background thread
        let prices_arc = self.prices.clone();
        let status_arc = self.connection_status.clone();
        let suspended_arc = self.suspended.clone();
        
        // Clone symbol list for the warmup call
        let symbols_for_warmup = symbols_lower.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            rt.block_on(async move {
                // 1. PULL (Batch Snapshot)
                // This runs ONCE at startup to populate the cache immediately
                warm_up_prices(prices_arc.clone(), &symbols_for_warmup).await;

                // 2. PUSH (Live Updates)
                // Then we enter the infinite WebSocket loop to keep prices fresh
                run_combined_price_stream_with_reconnect(
                    symbols_lower,
                    prices_arc,
                    status_arc,
                    suspended_arc,
                )
                .await;
            });
        });
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

/// Wrapper that handles reconnection logic with exponential backoff for a combined stream
#[cfg(not(target_arch = "wasm32"))]
async fn run_combined_price_stream_with_reconnect(
    symbols: Vec<String>,
    prices_arc: Arc<Mutex<HashMap<String, f64>>>,
    status_arc: Arc<Mutex<HashMap<String, ConnectionStatus>>>,
    suspended_arc: Arc<Mutex<bool>>,
) {
    let mut reconnect_delay = BINANCE.ws.initial_reconnect_delay_sec;

    loop {
        // Update status to connecting for every tracked symbol
        {
            let mut status_map = status_arc.lock().unwrap();
            for symbol in &symbols {
                status_map.insert(symbol.clone(), ConnectionStatus::Connecting);
            }
        }

        let url = build_combined_stream_url(&symbols);

        // Attempt connection
        match run_combined_price_stream(
            &symbols,
            &url,
            prices_arc.clone(),
            status_arc.clone(),
            suspended_arc.clone(),
        )
        .await
        {
            Ok(_) => {
                // Connection closed normally (24-hour timeout or server close)
                #[cfg(debug_assertions)]
                if DEBUG_FLAGS.print_price_stream_updates {
                    log::info!("Connection closed for combined stream, reconnecting...");
                }

                // Reset delay on successful connection that later closes
                reconnect_delay = BINANCE.ws.initial_reconnect_delay_sec;
            }
            Err(e) => {
                // Connection failed
                log::error!("Price stream error: {}", e);

                // Update status for every symbol
                {
                    let mut status_map = status_arc.lock().unwrap();
                    for symbol in &symbols {
                        status_map.insert(symbol.clone(), ConnectionStatus::Disconnected);
                    }
                }

                // Exponential backoff
                #[cfg(debug_assertions)]
                if DEBUG_FLAGS.print_price_stream_updates {
                    log::info!(
                        "Reconnecting combined price stream in {} seconds...",
                        reconnect_delay
                    );
                }
                tokio::time::sleep(Duration::from_secs(reconnect_delay)).await;

                // Increase delay for next attempt (capped at max)
                reconnect_delay = (reconnect_delay * 2).min(BINANCE.ws.max_reconnect_delay_sec);
            }
        }

        // Small delay before reconnecting even on normal close
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn run_combined_price_stream(
    symbols: &[String],
    url: &str,
    prices_arc: Arc<Mutex<HashMap<String, f64>>>,
    status_arc: Arc<Mutex<HashMap<String, ConnectionStatus>>>,
    suspended_arc: Arc<Mutex<bool>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    #[cfg(debug_assertions)]
    if DEBUG_FLAGS.print_price_stream_updates {
        log::info!("Connecting to Binance combined WebSocket: {}", url);
    }

    let (ws_stream, _) = connect_async(url).await?;

    // Update status to connected
    {
        let mut status_map = status_arc.lock().unwrap();
        for symbol in symbols {
            status_map.insert(symbol.clone(), ConnectionStatus::Connected);
        }
    }

    #[cfg(debug_assertions)]
    if DEBUG_FLAGS.print_price_stream_updates {
        log::info!(
            "✓ Connected to combined price stream for {} symbols",
            symbols.len()
        );
    }
    let (_write, mut read) = ws_stream.split();

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(wrapper) = serde_json::from_str::<CombinedStreamMessage>(&text) {
                    match wrapper.data.close_price.parse::<f64>() {
                        Ok(price) => {
                            // Only update prices if not suspended
                            let is_suspended = *suspended_arc.lock().unwrap();
                            if !is_suspended {
                                let symbol_lower = wrapper.data.symbol.to_lowercase();
                                if symbols.contains(&symbol_lower) {
                                    prices_arc
                                        .lock()
                                        .unwrap()
                                        .insert(symbol_lower.clone(), price);

                                    #[cfg(debug_assertions)]
                                    if DEBUG_FLAGS.print_price_stream_updates {
                                        log::info!(
                                            "[price-stream] {} -> {:.6}",
                                            wrapper.data.symbol,
                                            price
                                        );
                                    }
                                }
                            }
                        }
                        Err(parse_err) => {
                            log::error!(
                                "⚠️ Failed to parse miniTicker price '{}' for {}: {}",
                                wrapper.data.close_price,
                                wrapper.data.symbol,
                                parse_err
                            );
                            #[cfg(debug_assertions)]
                            if DEBUG_FLAGS.print_price_stream_updates {
                                log::info!("[price-stream] raw payload: {}", text);
                            }
                        }
                    }
                } else {
                    #[cfg(debug_assertions)]
                    if DEBUG_FLAGS.print_price_stream_updates {
                        log::error!("⚠️ Unexpected combined stream payload: {}", text);
                    }
                }
            }
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {
                // WebSocket keepalive - handled automatically
            }
            Ok(Message::Close(_)) => {
                #[cfg(debug_assertions)]
                if DEBUG_FLAGS.print_price_stream_updates {
                    log::info!("Combined WebSocket closed (likely 24hr timeout)");
                }
                break;
            }
            Err(e) => {
                log::error!("WebSocket error: {}", e);
                return Err(e.into());
            }
            _ => {}
        }
    }

    // Update status on disconnect
    {
        let mut status_map = status_arc.lock().unwrap();
        for symbol in symbols {
            status_map.insert(symbol.clone(), ConnectionStatus::Disconnected);
        }
    }

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn build_combined_stream_url(symbols: &[String]) -> String {
    let stream_descriptor = symbols
        .iter()
        .map(|symbol| format!("{}@miniTicker", symbol))
        .collect::<Vec<_>>()
        .join("/");

    format!("{}{}", BINANCE.ws.combined_base_url, stream_descriptor)
}


#[cfg(not(target_arch = "wasm32"))]
use binance_sdk::spot::SpotRestApi;
#[cfg(not(target_arch = "wasm32"))]
use binance_sdk::config::ConfigurationRestApi;
#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashSet;
#[cfg(not(target_arch = "wasm32"))]
use crate::config::{BinanceApiConfig};
#[cfg(not(target_arch = "wasm32"))]
use binance_sdk::spot::rest_api::{TickerPriceParams, TickerPriceResponse};


#[cfg(not(target_arch = "wasm32"))]
async fn warm_up_prices(prices_arc: Arc<Mutex<HashMap<String, f64>>>, symbols: &[String]) {
    
    log::info!(">>> PriceStream: Warming up price cache via REST API...");

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
                            let mut updated_count = 0;
                            
                            let wanted_set: HashSet<String> = symbols.iter()
                                .map(|s| s.to_lowercase())
                                .collect();

                            for ticker in all_tickers {
                                // 4. Safely handle Option fields (symbol/price might be None)
                                if let (Some(s), Some(p)) = (&ticker.symbol, &ticker.price) {
                                    let symbol_lower = s.to_lowercase();
                                    
                                    if wanted_set.contains(&symbol_lower) {
                                        let price = p.parse::<f64>().unwrap_or(0.0);
                                        if price > 0.0 {
                                            p_lock.insert(symbol_lower, price);
                                            updated_count += 1;
                                        }
                                    }
                                }
                            }
                            log::info!(
                                ">>> PriceStream: Warmup complete. Updated {}/{} pairs.",
                                updated_count,
                                symbols.len()
                            );
                        },
                        TickerPriceResponse::TickerPriceResponse1(_) => {
                            log::warn!(">>> PriceStream: Unexpected 'Single' response type during batch warmup.");
                        },
                        _ => {
                            log::warn!(">>> PriceStream: Unexpected 'Other' response type.");
                        }
                    }
                },
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

