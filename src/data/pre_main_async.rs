// Shared imports
use std::sync::mpsc::Sender;

use crate::Cli;
use crate::data::timeseries::TimeSeriesCollection;
use crate::models::ProgressEvent;

#[cfg(target_arch = "wasm32")]
use {crate::config::DEMO, crate::data::timeseries::wasm_demo::WasmDemoData};

#[cfg(not(target_arch = "wasm32"))]
use {
    crate::config::ANALYSIS,
    crate::config::BINANCE,
    crate::data::provider::{BinanceProvider, MarketDataProvider},
    crate::data::rate_limiter::GlobalRateLimiter,
    crate::data::storage::{MarketDataStorage, SqliteStorage},
    crate::domain::pair_interval::PairInterval,
    crate::models::OhlcvTimeSeries,
    crate::models::SyncStatus,
    anyhow::Result,
    futures::stream::{self, StreamExt},
    std::sync::Arc,
};

// ----------------------------------------------------------------------------
// NATIVE: Helper function
// ----------------------------------------------------------------------------
#[cfg(not(target_arch = "wasm32"))]
async fn sync_pair(
    pair: String,
    interval_ms: i64,
    storage: Arc<SqliteStorage>,
    provider: Arc<BinanceProvider>,
) -> Result<(OhlcvTimeSeries, usize)> {
    // Return count of new candles
    let interval_str = crate::utils::TimeUtils::interval_to_string(interval_ms);

    // 1. Check DB
    let last_time = storage.get_last_candle_time(&pair, interval_str).await?;
    let start_fetch = last_time.map(|t| t + 1);

    // 2. Fetch API
    let new_candles = provider
        .fetch_candles(&pair, interval_ms, start_fetch)
        .await?;

    let count = new_candles.len();

    if !new_candles.is_empty() {
        storage
            .insert_candles(&pair, interval_str, &new_candles)
            .await?;
    }

    // 3. Load from DB
    let full_history = storage.load_candles(&pair, interval_str, None).await?;

    let pair_interval = PairInterval {
        name: pair,
        interval_ms,
    };

    Ok((
        OhlcvTimeSeries::from_candles(pair_interval, full_history),
        count,
    ))
}

// ----------------------------------------------------------------------------
// MAIN ENTRY POINT
// ----------------------------------------------------------------------------
pub async fn fetch_pair_data(
    klines_acceptable_age_secs: i64,
    args: &Cli,
    progress_tx: Option<Sender<ProgressEvent>>, // <--- CHANGED TYPE
) -> (TimeSeriesCollection, &'static str) {
    // --- WASM IMPLEMENTATION ---
    #[cfg(target_arch = "wasm32")]
    {
        let _ = klines_acceptable_age_secs;
        let _ = args;
        let _ = progress_tx;

        // Use WasmDemoData directly (it is imported now)
        let mut timeseries_data =
            WasmDemoData::load().expect("failed to retrieve time series data for WASM");

        let original_len = timeseries_data.series_data.len();
        if original_len > DEMO.max_pairs {
            timeseries_data.series_data.truncate(DEMO.max_pairs);
        }

        return (timeseries_data, "WASM Static Cache");
    }

    // --- NATIVE IMPLEMENTATION ---
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = klines_acceptable_age_secs;
        let _ = args;

        let db_path = "klines.sqlite";
        let storage = Arc::new(
            SqliteStorage::new(db_path)
                .await
                .expect("Failed to init DB"),
        );
        storage
            .initialize()
            .await
            .expect("Failed to init DB schema");

        let safe_limit = (BINANCE.limits.weight_limit_minute as f32 * 0.8) as u32;
        let limiter = GlobalRateLimiter::new(safe_limit);

        let provider = Arc::new(BinanceProvider::new(limiter));
        let config = crate::config::BINANCE;

        // 2. Get Pairs
        let supply_pairs: Vec<String> = match std::fs::read_to_string("pairs.txt") {
            Ok(content) => content
                .lines()
                .map(|s| s.trim().to_uppercase())
                .filter(|s| !s.is_empty() && !s.starts_with('#'))
                .take(config.max_pairs)
                .collect(),
            Err(_) => vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
        };

        log::info!("Starting Delta Sync for {} pairs...", supply_pairs.len());

        // 3. INITIALIZE UI LIST
        // Tell the UI about all pairs immediately so they appear as "Pending"
        if let Some(ref tx) = progress_tx {
            for (i, pair) in supply_pairs.iter().enumerate() {
                let _ = tx.send(ProgressEvent {
                    index: i,
                    pair: pair.clone(),
                    status: SyncStatus::Pending,
                });
            }
        }

        // 3. Run in Parallel
        let interval = ANALYSIS.interval_width_ms;

        let results = stream::iter(supply_pairs)
            .enumerate()
            .map(|(i, pair)| {
                // Capture 'i' here
                let s = storage.clone();
                let p = provider.clone();
                let tx = progress_tx.clone();

                async move {
                    if let Some(ref tx) = tx {
                        let _ = tx.send(ProgressEvent {
                            index: i,
                            pair: pair.clone(),
                            status: SyncStatus::Syncing,
                        });
                    }

                    match sync_pair(pair.clone(), interval, s, p).await {
                        Ok((ts, new_count)) => {
                            if let Some(ref tx) = tx {
                                let _ = tx.send(ProgressEvent {
                                    index: i,
                                    pair: pair.clone(),
                                    status: SyncStatus::Completed(new_count),
                                });
                            }
                            Some(ts)
                        }
                        Err(e) => {
                            log::error!("Failed to sync {}: {}", pair, e);
                            if let Some(ref tx) = tx {
                                let _ = tx.send(ProgressEvent {
                                    index: i,
                                    pair: pair.clone(),
                                    status: SyncStatus::Failed(e.to_string()),
                                });
                            }
                            None
                        }
                    }
                }
            })
            .buffer_unordered(BINANCE.limits.concurrent_sync_tasks) // Parallelism Limit
            .collect::<Vec<_>>()
            .await;

        let series_data: Vec<_> = results.into_iter().flatten().collect();

        (
            TimeSeriesCollection {
                name: "SQLite-Synced Collection".to_string(),
                version: 1.0,
                series_data,
            },
            "SQLite + Binance",
        )
    }
}
