use serde::{Deserialize, Serialize};

use crate::analysis::market_state::MarketState;
use crate::config::{OptimizationStrategy, PhPct, Price, StationId, StopPrice, TargetPrice};

#[cfg(not(target_arch = "wasm32"))]
use {
    anyhow::{Context, Result},
    sqlx::{
        Row,
        sqlite::{
            SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions,
            SqliteSynchronous,
        },
    },
    std::str::FromStr,
    std::time::Duration,
    uuid::Uuid,
};

use crate::models::{TradeDirection, TradeOutcome};

#[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
use crate::config::DF;

#[cfg(not(target_arch = "wasm32"))]
use crate::config::PriceLike;

/// A finalized trade record ready for persistent storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TradeResult {
    pub trade_id: String, // Original UUID
    pub pair: String,
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

// --- TRAIT DEFINITION ---

/// Abstract interface for results storage (Native vs WASM)
#[async_trait::async_trait]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) trait ResultsRepositoryTrait: Send + Sync {
    async fn initialize(&self) -> Result<()>;
    async fn get_installation_id(&self) -> Result<String>;
    async fn record_trade(&self, result: TradeResult) -> Result<()>;
}

// --- NATIVE IMPLEMENTATION ---

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct ResultsRepository {
    pool: SqlitePool,
}

#[cfg(not(target_arch = "wasm32"))]
impl ResultsRepository {
    pub async fn new(db_path: &str) -> Result<Self> {
        // Configure separate connection for results.db
        // We use the same robust options as the main DB
        let connection_options = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(10))
            .synchronous(SqliteSynchronous::Normal);

        let pool = SqlitePoolOptions::new()
            .max_connections(2) // Low connection count, this is low throughput
            .connect_with(connection_options)
            .await
            .context("Failed to connect to results.db")?;

        let repo = Self { pool };
        repo.initialize().await?;

        Ok(repo)
    }

    /// Generates a new random installation ID
    fn generate_new_id() -> String {
        Uuid::new_v4().to_string()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait::async_trait]
impl ResultsRepositoryTrait for ResultsRepository {
    async fn initialize(&self) -> Result<()> {
        // 1. Meta Table (Key-Value)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create meta table")?;

        // 2. History Table (UPDATED SCHEMA)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS history (
            trade_id TEXT PRIMARY KEY,
            installation_id TEXT NOT NULL,

            pair TEXT NOT NULL,
            direction TEXT NOT NULL,

            entry_price REAL NOT NULL,
            stop_price REAL NOT NULL,
            target_price REAL NOT NULL,
            exit_price REAL NOT NULL,

            exit_reason TEXT NOT NULL,

            entry_time INTEGER NOT NULL,
            planned_expiry_time INTEGER NOT NULL,
            exit_time INTEGER NOT NULL,

            strategy TEXT NOT NULL,
            station_id TEXT NOT NULL,
            market_state TEXT NOT NULL,

            ph_pct REAL NOT NULL
        );",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create history table")?;

        // 3. Ensure Identity Exists
        let _ = self.get_installation_id().await?;

        Ok(())
    }

    // async fn initialize(&self) -> Result<()> {
    //     // 1. Meta Table (Key-Value)
    //     sqlx::query(
    //         "CREATE TABLE IF NOT EXISTS meta (
    //             key TEXT PRIMARY KEY,
    //             value TEXT NOT NULL
    //         );",
    //     )
    //     .execute(&self.pool)
    //     .await
    //     .context("Failed to create meta table")?;

    //     // 2. History Table
    //     sqlx::query(
    //         "CREATE TABLE IF NOT EXISTS history (
    //             trade_id TEXT PRIMARY KEY,
    //             installation_id TEXT NOT NULL,
    //             pair TEXT NOT NULL,
    //             direction TEXT NOT NULL,
    //             entry_price REAL NOT NULL,
    //             exit_price REAL NOT NULL,
    //             outcome TEXT NOT NULL,
    //             entry_time INTEGER NOT NULL,
    //             exit_time INTEGER NOT NULL,
    //             pnl_pct REAL NOT NULL,
    //             model_json TEXT
    //         );",
    //     )
    //     .execute(&self.pool)
    //     .await
    //     .context("Failed to create history table")?;

    //     // 3. Ensure Identity Exists
    //     let _ = self.get_installation_id().await?;

    //     Ok(())
    // }

    async fn get_installation_id(&self) -> Result<String> {
        // Try fetch
        let row = sqlx::query("SELECT value FROM meta WHERE key = 'installation_id'")
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            Ok(r.get("value"))
        } else {
            // Generate and Insert
            let new_id = Self::generate_new_id();
            #[cfg(debug_assertions)]
            if DF.log_results_repo {
                log::info!("RESULTS DB: Generating new Installation ID: {}", new_id);
            }

            sqlx::query("INSERT INTO meta (key, value) VALUES ('installation_id', ?)")
                .bind(&new_id)
                .execute(&self.pool)
                .await?;

            Ok(new_id)
        }
    }

    async fn record_trade(&self, result: TradeResult) -> Result<()> {
        let install_id = self.get_installation_id().await?;
        let outcome_str = result.exit_reason.to_string();

        #[cfg(debug_assertions)]
        if DF.log_results_repo {
            log::info!(
                "RESULTS DB WRITE | id={} | pair={} | dir={} | entry={} | stop={} | target={} | exit={} | reason={} | entry_t={} | expiry_t={} | exit_t={} | strat={} | station={} | ph={}",
                result.trade_id,
                result.pair,
                result.direction,
                result.entry_price.value(),
                result.stop_price.value(),
                result.target_price.value(),
                result.exit_price.value(),
                outcome_str,
                result.entry_time,
                result.planned_expiry_time,
                result.exit_time,
                result.strategy,
                result.station_id,
                result.ph_pct.value(),
            );
        }

        sqlx::query(
            r#"
    INSERT INTO history (
        trade_id,
        installation_id,
        pair,
        direction,
        entry_price,
        stop_price,
        target_price,
        exit_price,
        exit_reason,
        entry_time,
        planned_expiry_time,
        exit_time,
        strategy,
        station_id,
        market_state,
        ph_pct
    )
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    "#,
        )
        .bind(result.trade_id)
        .bind(install_id)
        .bind(result.pair)
        .bind(result.direction.to_string())
        .bind(result.entry_price.value())
        .bind(result.stop_price.value())
        .bind(result.target_price.value())
        .bind(result.exit_price.value())
        .bind(result.exit_reason.to_string())
        .bind(result.entry_time)
        .bind(result.planned_expiry_time)
        .bind(result.exit_time)
        .bind(result.strategy.to_string())
        .bind(result.station_id.short_name())
        .bind(result.market_state.to_string())
        .bind(result.ph_pct.value())
        .execute(&self.pool)
        .await
        .context("Failed to insert trade result")?;

        Ok(())
    }
}
