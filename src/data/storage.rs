use {
    crate::{
        config::{BaseVol, ClosePrice, HighPrice, LowPrice, OpenPrice, PriceLike, QuoteVol},
        domain::Candle,
    },
    anyhow::Result,
    async_trait::async_trait,
    sqlx::{
        ConnectOptions, Pool, QueryBuilder, Row, Sqlite,
        sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    },
    std::{str::FromStr, time::Duration},
};

#[async_trait]
pub trait MarketDataStorage: Send + Sync {
    async fn initialize(&self) -> Result<()>;
    async fn get_last_candle_time(&self, pair: &str, interval: &str) -> Result<Option<i64>>;
    async fn insert_candles(&self, pair: &str, interval: &str, candles: &[Candle]) -> Result<u64>;
    async fn load_candles(
        &self,
        pair: &str,
        interval: &str,
        start_time: Option<i64>,
    ) -> Result<Vec<Candle>>;
}

pub struct SqliteStorage {
    pool: Pool<Sqlite>,
}

impl SqliteStorage {
    pub async fn new(db_path: &str) -> Result<Self> {
        let connection_options = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(60))
            .synchronous(SqliteSynchronous::Normal)
            .log_slow_statements(log::LevelFilter::Warn, Duration::from_secs(10));

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(connection_options)
            .await?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl MarketDataStorage for SqliteStorage {
    async fn initialize(&self) -> Result<()> {
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

        let last_time: Option<i64> = result.try_get("last_time")?;
        Ok(last_time)
    }

    /// Batches candles in chunks of 3000 to stay within SQLite's 32k parameter limit.
    async fn insert_candles(&self, pair: &str, interval: &str, candles: &[Candle]) -> Result<u64> {
        if candles.is_empty() {
            return Ok(0);
        }

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

            query_builder.build().execute(&self.pool).await?;
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
