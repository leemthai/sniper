#![allow(clippy::collapsible_if)]

#[cfg(not(target_arch = "wasm32"))]
use crate::config::debug::PRINT_PRICE_STREAM_UPDATES;
#[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
use crate::config::debug::PRINT_SIMULATION_EVENTS;
#[cfg(not(target_arch = "wasm32"))]
use crate::config::{
    BINANCE_WS_COMBINED_BASE, INITIAL_RECONNECT_DELAY_SECS, MAX_RECONNECT_DELAY_SECS,
};
#[cfg(not(target_arch = "wasm32"))]
use futures::StreamExt;
#[cfg(not(target_arch = "wasm32"))]
use serde::Deserialize;
#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use tokio_tungstenite::{connect_async, tungstenite::Message};

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
        if PRINT_SIMULATION_EVENTS {
            println!("🔇 WebSocket price updates suspended");
        }
    }

    /// Resume price updates (exit simulation mode)
    pub fn resume(&self) {
        *self.suspended.lock().unwrap() = false;
        #[cfg(debug_assertions)]
        if PRINT_SIMULATION_EVENTS {
            println!("🔊 WebSocket price updates resumed");
        }
    }

    /// Check if price updates are suspended
    pub fn is_suspended(&self) -> bool {
        *self.suspended.lock().unwrap()
    }

    // /// Get connection status for a symbol
    // pub fn get_status(&self, symbol: &str) -> ConnectionStatus {
    //     let symbol_lower = symbol.to_lowercase();
    //     self.connection_status
    //         .lock()
    //         .unwrap()
    //         .get(&symbol_lower)
    //         .copied()
    //         .unwrap_or(ConnectionStatus::Disconnected)
    // }

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

    /// Subscribe to multiple trading pairs at once
    /// Symbol format: "BTCUSDT" (will be converted to lowercase internally)
    pub fn subscribe_all(&self, symbols: Vec<String>) {
        let symbols_lower: Vec<String> = symbols.iter().map(|s| s.to_lowercase()).collect();

        // Check if already subscribed
        {
            let subscribed = self.subscribed_symbols.lock().unwrap();
            if *subscribed == symbols_lower {
                return; // Already subscribed to these symbols
            }
        }

        // Update subscribed list
        *self.subscribed_symbols.lock().unwrap() = symbols_lower.clone();

        #[cfg(debug_assertions)]
        if PRINT_PRICE_STREAM_UPDATES {
            println!(
                "Subscribing to {} price streams with auto-reconnect",
                symbols_lower.len()
            );
        }

        // Mark all symbols as connecting up-front so the UI can show progress while we establish the stream.
        {
            let mut status_map = self.connection_status.lock().unwrap();
            status_map.clear();
            for symbol in &symbols_lower {
                status_map.insert(symbol.clone(), ConnectionStatus::Connecting);
            }
        }

        let prices_arc = Arc::clone(&self.prices);
        let status_arc = Arc::clone(&self.connection_status);
        let suspended_arc = Arc::clone(&self.suspended);

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            rt.block_on(async move {
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
#[derive(Default, Clone)]
pub struct PriceStreamManager;

#[cfg(target_arch = "wasm32")]
impl PriceStreamManager {
    pub fn new() -> Self {
        Self
    }

    pub fn get_price(&self, _symbol: &str) -> Option<f64> {
        None
    }

    pub fn suspend(&self) {}

    pub fn resume(&self) {}

    pub fn is_suspended(&self) -> bool {
        true
    }

    pub fn connection_health(&self) -> f64 {
        0.0
    }

    pub fn subscribe_all(&self, _symbols: Vec<String>) {
        #[cfg(debug_assertions)]
        println!("Price stream disabled in WASM demo build.");
    }
}

/// Wrapper that handles reconnection logic with exponential backoff for a combined stream
#[cfg(not(target_arch = "wasm32"))]
async fn run_combined_price_stream_with_reconnect(
    symbols: Vec<String>,
    prices_arc: Arc<Mutex<HashMap<String, f64>>>,
    status_arc: Arc<Mutex<HashMap<String, ConnectionStatus>>>,
    suspended_arc: Arc<Mutex<bool>>,
) {
    let mut reconnect_delay = INITIAL_RECONNECT_DELAY_SECS;

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
                if PRINT_PRICE_STREAM_UPDATES {
                    println!("Connection closed for combined stream, reconnecting...");
                }

                // Reset delay on successful connection that later closes
                reconnect_delay = INITIAL_RECONNECT_DELAY_SECS;
            }
            Err(e) => {
                // Connection failed
                eprintln!("Price stream error: {}", e);

                // Update status for every symbol
                {
                    let mut status_map = status_arc.lock().unwrap();
                    for symbol in &symbols {
                        status_map.insert(symbol.clone(), ConnectionStatus::Disconnected);
                    }
                }

                // Exponential backoff
                if PRINT_PRICE_STREAM_UPDATES {
                    println!(
                        "Reconnecting combined price stream in {} seconds...",
                        reconnect_delay
                    );
                }
                tokio::time::sleep(Duration::from_secs(reconnect_delay)).await;

                // Increase delay for next attempt (capped at max)
                reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY_SECS);
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
    if PRINT_PRICE_STREAM_UPDATES {
        println!("Connecting to Binance combined WebSocket: {}", url);
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
    if PRINT_PRICE_STREAM_UPDATES {
        println!(
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
                                    if PRINT_PRICE_STREAM_UPDATES {
                                        println!(
                                            "[price-stream] {} -> {:.6}",
                                            wrapper.data.symbol, price
                                        );
                                    }
                                }

                                // #[cfg(debug_assertions)]
                                // println!("Updated price for {}: ${:.2}", ticker.symbol, price);
                            }
                        }
                        Err(parse_err) => {
                            eprintln!(
                                "⚠️ Failed to parse miniTicker price '{}' for {}: {}",
                                wrapper.data.close_price, wrapper.data.symbol, parse_err
                            );
                            #[cfg(debug_assertions)]
                            if PRINT_PRICE_STREAM_UPDATES {
                                println!("[price-stream] raw payload: {}", text);
                            }
                        }
                    }
                } else {
                    if PRINT_PRICE_STREAM_UPDATES {
                        eprintln!("⚠️ Unexpected combined stream payload: {}", text);
                    }
                }
            }
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {
                // WebSocket keepalive - handled automatically
            }
            Ok(Message::Close(_)) => {
                #[cfg(debug_assertions)]
                if PRINT_PRICE_STREAM_UPDATES {
                    println!("Combined WebSocket closed (likely 24hr timeout)");
                }
                break;
            }
            Err(e) => {
                eprintln!("WebSocket error: {}", e);
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

    format!("{}{}", BINANCE_WS_COMBINED_BASE, stream_descriptor)
}
