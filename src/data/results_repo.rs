use anyhow::Result;

use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use {
    anyhow::Context,
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

use crate::models::trading_view::{TradeDirection, TradeOutcome};

#[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
use crate::config::DF;

/// A finalized trade record ready for persistent storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeResult {
    pub trade_id: String, // Original UUID
    pub pair: String,
    pub direction: TradeDirection,
    pub entry_price: f64,
    pub exit_price: f64,
    pub outcome: TradeOutcome,
    pub entry_time: i64,
    pub exit_time: i64,
    pub final_pnl_pct: f64,
    pub model_snapshot: Option<String>, // JSON blob for debugging
}

// --- TRAIT DEFINITION ---

/// Abstract interface for results storage (Native vs WASM)
#[async_trait::async_trait]
pub trait ResultsRepositoryTrait: Send + Sync {
    async fn initialize(&self) -> Result<()>;
    async fn get_installation_id(&self) -> Result<String>;
    async fn record_trade(&self, result: TradeResult) -> Result<()>;
}

// --- NATIVE IMPLEMENTATION ---

#[cfg(not(target_arch = "wasm32"))]
pub struct ResultsRepository {
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

        // 2. History Table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS history (
                trade_id TEXT PRIMARY KEY,
                installation_id TEXT NOT NULL,
                pair TEXT NOT NULL,
                direction TEXT NOT NULL,
                entry_price REAL NOT NULL,
                exit_price REAL NOT NULL,
                outcome TEXT NOT NULL,
                entry_time INTEGER NOT NULL,
                exit_time INTEGER NOT NULL,
                pnl_pct REAL NOT NULL,
                model_json TEXT
            );",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create history table")?;

        // 3. Ensure Identity Exists
        let _ = self.get_installation_id().await?;

        Ok(())
    }

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
        let direction_str = result.direction.to_string();
        let outcome_str = result.outcome.to_string();

        #[cfg(debug_assertions)]
        if DF.log_results_repo {
            log::info!(
                "RESULTS DB: Archiving trade [{}] {} PnL: {:.2}% ({})",
                result.pair,
                result.trade_id,
                result.final_pnl_pct,
                outcome_str
            );
        }

        sqlx::query(
            r#"
            INSERT INTO history 
            (trade_id, installation_id, pair, direction, entry_price, exit_price, outcome, entry_time, exit_time, pnl_pct, model_json)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(result.trade_id)
        .bind(install_id)
        .bind(result.pair)
        .bind(direction_str)
        .bind(result.entry_price)
        .bind(result.exit_price)
        .bind(outcome_str)
        .bind(result.entry_time)
        .bind(result.exit_time)
        .bind(result.final_pnl_pct)
        .bind(result.model_snapshot)
        .execute(&self.pool)
        .await
        .context("Failed to insert trade result")?;

        Ok(())
    }
}

// --- WASM IMPLEMENTATION (No-Op) ---

#[cfg(target_arch = "wasm32")]
pub struct ResultsRepository;

#[cfg(target_arch = "wasm32")]
impl ResultsRepository {
    pub fn new(_path: &str) -> Result<Self> {
        Ok(Self)
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait::async_trait]
impl ResultsRepositoryTrait for ResultsRepository {
    async fn initialize(&self) -> Result<()> {
        Ok(())
    }
    async fn get_installation_id(&self) -> Result<String> {
        Ok("wasm-demo-user".to_string())
    }
    async fn record_trade(&self, _result: TradeResult) -> Result<()> {
        // In WASM, we just discard the result as per instructions
        Ok(())
    }
}
