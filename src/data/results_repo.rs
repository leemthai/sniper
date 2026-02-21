use {
    crate::{
        config::{
            OptimizationStrategy, PhPct, Price, PriceLike, StationId, StopPrice, TargetPrice,
        },
        models::{MarketState, TradeDirection, TradeOutcome},
    },
    anyhow::{Result, anyhow},
    async_trait::async_trait,
    serde::{Deserialize, Serialize},
    sqlx::sqlite::{
        SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
    },
    std::{str::FromStr, time::Duration},
    tokio::sync::mpsc,
};

#[cfg(debug_assertions)]
use crate::config::DF;

/// A finalized trade record ready for persistent storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TradeResult {
    pub trade_id: String, // Original UUID
    pub pair_name: String,
    pub direction: TradeDirection,
    pub entry_price: Price,
    pub exit_price: Price,
    pub stop_price: StopPrice,
    pub target_price: TargetPrice,

    pub exit_reason: TradeOutcome,

    pub entry_time: i64,
    pub exit_time: i64,
    pub planned_expiry_time: i64,

    pub strategy: OptimizationStrategy,
    pub station_id: StationId,
    pub market_state: MarketState,

    pub ph_pct: PhPct,
}

/// Abstract interface for results storage (Native vs WASM)
#[async_trait]
pub(crate) trait ResultsRepositoryTrait: Send + Sync {
    async fn initialize(&self) -> Result<()>;
    fn enqueue(&self, trade: TradeResult) -> Result<()>;
}

pub struct SqliteResultsRepository {
    pool: SqlitePool,
    sender: mpsc::UnboundedSender<TradeResult>,
}

impl SqliteResultsRepository {
    pub async fn new(db_path: &str) -> Result<Self> {
        let connection_options = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(10))
            .synchronous(SqliteSynchronous::Normal);

        let pool = SqlitePoolOptions::new()
            .max_connections(1) // single writer
            .connect_with(connection_options)
            .await?;

        let (tx, mut rx) = mpsc::unbounded_channel::<TradeResult>();
        let pool_clone = pool.clone();

        // Dedicated writer task
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("DB writer runtime");

            rt.block_on(async move {
                while let Some(trade) = rx.recv().await {
                    if let Err(e) = insert_trade(&pool_clone, trade).await {
                        log::error!("DB WRITE FAILED: {:?}", e);
                    }
                }
            });
        });

        let repo = Self { pool, sender: tx };
        repo.initialize().await?;

        Ok(repo)
    }
}

#[async_trait]
impl ResultsRepositoryTrait for SqliteResultsRepository {
    async fn initialize(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS trades (
                id                    INTEGER PRIMARY KEY AUTOINCREMENT,
                trade_id              TEXT NOT NULL,
                pair_name             TEXT NOT NULL,
                direction             TEXT NOT NULL,
                entry_price           REAL NOT NULL,
                exit_price            REAL NOT NULL,
                stop_price            REAL NOT NULL,
                target_price          REAL NOT NULL,
                exit_reason           TEXT NOT NULL,
                entry_time            INTEGER NOT NULL,
                exit_time             INTEGER NOT NULL,
                planned_expiry_time   INTEGER NOT NULL,
                strategy              TEXT NOT NULL,
                station_id            TEXT NOT NULL,
                market_state          TEXT NOT NULL,
                ph_pct                REAL NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
    fn enqueue(&self, trade: TradeResult) -> Result<()> {
        self.sender
            .send(trade)
            .map_err(|e| anyhow!("Channel send failed: {:?}", e))
    }
}

async fn insert_trade(pool: &SqlitePool, result: TradeResult) -> Result<()> {
    #[cfg(debug_assertions)]
    if DF.log_results_repo {
        log::info!(
            "RESULTS DB WRITE \
            | id={} \
            | pair={} \
            | dir={:?} \
            | entry={} \
            | exit={} \
            | stop={} \
            | target={} \
            | entry_time={} \
            | exit_time={} \
            | expiry_time={} \
            | reason={:?} \
            | strategy={:?} \
            | station={:?} \
            | market={:?} \
            | ph_pct={}",
            result.trade_id,
            result.pair_name,
            result.direction,
            result.entry_price,
            result.exit_price,
            result.stop_price,
            result.target_price,
            result.entry_time,
            result.exit_time,
            result.planned_expiry_time,
            result.exit_reason,
            result.strategy,
            result.station_id,
            result.market_state,
            result.ph_pct,
        );
    }
    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        INSERT INTO trades (
            trade_id,
            pair_name,
            direction,
            entry_price,
            exit_price,
            stop_price,
            target_price,
            exit_reason,
            entry_time,
            exit_time,
            planned_expiry_time,
            strategy,
            station_id,
            market_state,
            ph_pct
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15);
        "#,
    )
    .bind(&result.trade_id)
    .bind(&result.pair_name)
    .bind(format!("{:?}", result.direction))
    .bind(result.entry_price.value())
    .bind(result.exit_price.value())
    .bind(result.stop_price.value())
    .bind(result.target_price.value())
    .bind(format!("{:?}", result.exit_reason))
    .bind(result.entry_time)
    .bind(result.exit_time)
    .bind(result.planned_expiry_time)
    .bind(format!("{:?}", result.strategy))
    .bind(format!("{:?}", result.station_id))
    .bind(format!("{:?}", result.market_state))
    .bind(result.ph_pct.value())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    #[cfg(debug_assertions)]
    if DF.log_results_repo {
        log::info!("RESULTS DB COMMIT OK | id={}", result.trade_id);
    }

    Ok(())
}
