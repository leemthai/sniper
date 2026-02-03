use anyhow::Result;
use async_trait::async_trait;

use crate::domain::candle::Candle;
// use ;

// WASM imports
#[cfg(not(target_arch = "wasm32"))]
use {
    crate::config::{BaseVol, QuoteVol, OpenPrice, HighPrice, LowPrice, ClosePrice, PriceLike},
    sqlx::ConnectOptions,
    sqlx::{
        Pool, QueryBuilder, Row, Sqlite,
        sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    },
    std::str::FromStr,
    std::time::Duration,
};

// --- 1. THE INTERFACE ---

/// The contract that any storage engine (SQLite or Memory) must obey.
#[async_trait]
pub trait MarketDataStorage: Send + Sync {
    /// Initialize the storage (Create tables / Load bytes)
    async fn initialize(&self) -> Result<()>;

    /// Get the timestamp of the last candle stored for this pair.
    async fn get_last_candle_time(&self, pair: &str, interval: &str) -> Result<Option<i64>>;

    /// Save new candles to storage (Insert / Append)
    async fn insert_candles(&self, pair: &str, interval: &str, candles: &[Candle]) -> Result<u64>;

    /// Load candles for analysis.
    async fn load_candles(
        &self,
        pair: &str,
        interval: &str,
        start_time: Option<i64>,
    ) -> Result<Vec<Candle>>;
}

// ============================================================================
// 2. NATIVE IMPLEMENTATION (SQLite)
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
pub struct SqliteStorage {
    pool: Pool<Sqlite>,
}

#[cfg(not(target_arch = "wasm32"))]
impl SqliteStorage {
    /// Connect to (or create) the database file
    pub async fn new(db_path: &str) -> Result<Self> {
        // Configure options to apply to EVERY connection in the pool
        let connection_options = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal) // Enable WAL
            .busy_timeout(Duration::from_secs(60)) // Wait 60s for locks
            .synchronous(SqliteSynchronous::Normal) // Faster writes
            .log_slow_statements(log::LevelFilter::Warn, Duration::from_secs(10));

        let pool = SqlitePoolOptions::new()
            .max_connections(5) // Limit max concurrent connections
            .connect_with(connection_options)
            .await?;

        Ok(Self { pool })
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl MarketDataStorage for SqliteStorage {
    async fn initialize(&self) -> Result<()> {
        // We don't need manual PRAGMA queries here anymore because
        // SqliteConnectOptions in new() handles them for every connection.

        // Create Table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS klines (
                symbol TEXT NOT NULL,
                interval TEXT NOT NULL,
                open_time INTEGER NOT NULL,
                open REAL NOT NULL,
                high REAL NOT NULL,
                low REAL NOT NULL,
                close REAL NOT NULL,
                base_vol REAL NOT NULL,
                quote_vol REAL NOT NULL,
                PRIMARY KEY (symbol, interval, open_time)
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_last_candle_time(&self, pair: &str, interval: &str) -> Result<Option<i64>> {
        let result = sqlx::query(
            r#"
            SELECT MAX(open_time) as last_time 
            FROM klines 
            WHERE symbol = ? AND interval = ?
            "#,
        )
        .bind(pair)
        .bind(interval)
        .fetch_one(&self.pool)
        .await?;

        // Extract value (might be NULL if new pair)
        let last_time: Option<i64> = result.try_get("last_time")?;
        Ok(last_time)
    }

    async fn insert_candles(&self, pair: &str, interval: &str, candles: &[Candle]) -> Result<u64> {
        if candles.is_empty() {
            return Ok(0);
        }

        // Use QueryBuilder for massive speedup (Single Query vs 500 Queries)
        // SQLite limit is usually 32k params, so we batch in chunks of ~3000 candles to be safe.
        // Each candle has 9 params. 3000 * 9 = 27000 < 32000.

        for chunk in candles.chunks(3000) {
            let mut query_builder = QueryBuilder::new(
                "INSERT OR IGNORE INTO klines (symbol, interval, open_time, open, high, low, close, base_vol, quote_vol) ",
            );

            query_builder.push_values(chunk, |mut b, c| {
                b.push_bind(pair)
                    .push_bind(interval)
                    .push_bind(c.timestamp_ms)
                    .push_bind(c.open_price.value())
                    .push_bind(c.high_price.value())
                    .push_bind(c.low_price.value())
                    .push_bind(c.close_price.value())
                    .push_bind(c.base_asset_volume.value())
                    .push_bind(c.quote_asset_volume.value());
            });

            let query = query_builder.build();
            query.execute(&self.pool).await?;
        }

        Ok(candles.len() as u64)
    }

    async fn load_candles(
        &self,
        pair: &str,
        interval: &str,
        start_time: Option<i64>,
    ) -> Result<Vec<Candle>> {
        let query_str = if start_time.is_some() {
            r#"
            SELECT open_time, open, high, low, close, base_vol, quote_vol
            FROM klines 
            WHERE symbol = ? AND interval = ? AND open_time >= ?
            ORDER BY open_time ASC
            "#
        } else {
            r#"
            SELECT open_time, open, high, low, close, base_vol, quote_vol
            FROM klines 
            WHERE symbol = ? AND interval = ? 
            ORDER BY open_time ASC
            "#
        };

        let mut query = sqlx::query(query_str).bind(pair).bind(interval);

        if let Some(ts) = start_time {
            query = query.bind(ts);
        }

        let rows = query.fetch_all(&self.pool).await?;

        let candles = rows
            .iter()
            .map(|row| {
                Candle::new(
                    row.get("open_time"),
                    OpenPrice::new(row.get("open")),
                    HighPrice::new(row.get("high")),
                    LowPrice::new(row.get("low")),
                    ClosePrice::new(row.get("close")),
                    BaseVol::new(row.get("base_vol")),
                    QuoteVol::new(row.get("quote_vol")),
                )
            })
            .collect();

        Ok(candles)
    }
}

// ============================================================================
// 3. WASM IMPLEMENTATION (In-Memory / Static)
// ============================================================================

#[cfg(target_arch = "wasm32")]
pub struct WasmStorage;

#[cfg(target_arch = "wasm32")]
impl WasmStorage {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait]
impl MarketDataStorage for WasmStorage {
    async fn initialize(&self) -> Result<()> {
        // In the future, this is where we parse the binary blob
        Ok(())
    }

    async fn get_last_candle_time(&self, _pair: &str, _interval: &str) -> Result<Option<i64>> {
        // WASM is static, so we don't update.
        Ok(None)
    }

    async fn insert_candles(
        &self,
        _pair: &str,
        _interval: &str,
        _candles: &[Candle],
    ) -> Result<u64> {
        // No-op for now
        Ok(0)
    }

    async fn load_candles(
        &self,
        _pair: &str,
        _interval: &str,
        _start_time: Option<i64>,
    ) -> Result<Vec<Candle>> {
        // TODO: Hook this up to the existing static memory cache
        Ok(Vec::new())
    }
}
