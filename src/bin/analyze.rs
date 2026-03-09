// `cargo run --bin analyze -- [OPTIONS]`
//
// Reads `results.sqlite` and prints formatted analysis tables, then persists a
// summary row into the `run_summaries` table so that aggregate stats are
// queryable without re-scanning all trades.
//
// Usage examples:
//   cargo run --bin analyze                        # latest run
//   cargo run --bin analyze -- --run-id latest     # same
//   cargo run --bin analyze -- --run-id all        # every run side-by-side
//   cargo run --bin analyze -- --run-id 3          # specific run
//   cargo run --bin analyze -- --db path/to/other.sqlite

#[cfg(not(target_arch = "wasm32"))]
mod inner {
    use {
        anyhow::{Context, Result, anyhow},
        chrono::{DateTime, Utc},
        clap::Parser,
        sqlx::{
            Row,
            sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions},
        },
        std::{collections::HashMap, str::FromStr, time::Duration},
        tabled::{
            Table, Tabled,
            settings::{Alignment, Style, object::Columns},
        },
        zone_sniper::{RunSummary, SqliteResultsRepository},
    };

    // ─── CLI ──────────────────────────────────────────────────────────────────

    #[derive(Parser, Debug)]
    #[command(
        name = "analyze",
        about = "Analyse results.sqlite: win-rates, PnL, calibration, and cross-run comparison"
    )]
    struct Cli {
        /// Run ID to analyse: a positive integer, "latest", or "all"
        #[arg(long, default_value = "latest")]
        run_id: String,

        /// Path to the SQLite results database
        #[arg(long, default_value = "results.sqlite")]
        db: String,

        /// Skip persisting summary rows back into run_summaries
        #[arg(long, default_value_t = false)]
        no_persist: bool,
    }

    // ─── Raw row types returned from SQL ─────────────────────────────────────

    #[derive(Debug)]
    struct RunMeta {
        id: i64,
        model_version: String,
        run_type: String,
        description: String,
        created_at: i64,
        trade_count: i64,
    }

    #[derive(Debug)]
    struct TradeSummaryRow {
        pair_name: String,
        station_id: String,
        strategy: String,
        exit_reason: String,
        /// Raw PnL percentage (fractional, e.g. 0.05 = 5%)
        pnl_pct: f64,
        predicted_win_rate: Option<f64>,
    }

    // ─── Table display structs (one per printed table) ─────────────────────

    #[derive(Tabled)]
    struct OverallRow {
        #[tabled(rename = "Metric")]
        metric: String,
        #[tabled(rename = "Value")]
        value: String,
    }

    #[derive(Tabled)]
    struct SegmentRow {
        #[tabled(rename = "Segment")]
        segment: String,
        #[tabled(rename = "Trades")]
        trades: usize,
        #[tabled(rename = "Wins")]
        wins: usize,
        #[tabled(rename = "Losses")]
        losses: usize,
        #[tabled(rename = "Timeouts")]
        timeouts: usize,
        #[tabled(rename = "Win Rate")]
        win_rate: String,
        #[tabled(rename = "Avg PnL")]
        avg_pnl: String,
    }

    #[derive(Tabled)]
    struct CalibrationRow {
        #[tabled(rename = "Predicted Band")]
        band: String,
        #[tabled(rename = "Trades")]
        trades: usize,
        #[tabled(rename = "Actual Win Rate")]
        actual_win_rate: String,
        #[tabled(rename = "Predicted Mid")]
        predicted_mid: String,
        #[tabled(rename = "Abs Error")]
        abs_error: String,
    }

    #[derive(Tabled)]
    struct ComparisonRow {
        #[tabled(rename = "Run ID")]
        run_id: String,
        #[tabled(rename = "Model")]
        model_version: String,
        #[tabled(rename = "Type")]
        run_type: String,
        #[tabled(rename = "Trades")]
        trade_count: String,
        #[tabled(rename = "Win Rate")]
        win_rate: String,
        #[tabled(rename = "Avg PnL")]
        avg_pnl: String,
        #[tabled(rename = "Cal. MAE")]
        calibration_mae: String,
        #[tabled(rename = "Created At")]
        created_at: String,
        #[tabled(rename = "Description")]
        description: String,
    }

    // ─── Entry point ─────────────────────────────────────────────────────────

    pub async fn run() -> Result<()> {
        let cli = Cli::parse();

        // Open a read-capable pool (separate from the write pool in the main app).
        let pool = open_pool(&cli.db).await?;

        // Resolve which run IDs we're working with.
        let run_ids = resolve_run_ids(&pool, &cli.run_id).await?;

        if run_ids.is_empty() {
            println!("No matching runs found in {}.", cli.db);
            return Ok(());
        }

        // Open the write-capable SqliteResultsRepository for persisting summaries.
        // We only need it when --no-persist is not set.
        let repo_opt: Option<SqliteResultsRepository> = if cli.no_persist {
            None
        } else {
            Some(SqliteResultsRepository::new(&cli.db).await?)
        };

        if run_ids.len() == 1 {
            // ── Single-run detailed view ──────────────────────────────────
            let run_id = run_ids[0];
            let meta = fetch_run_meta(&pool, run_id).await?;
            let trades = fetch_trades(&pool, run_id).await?;

            print_run_header(&meta);
            let (win_rate, avg_pnl) = print_overall_stats(&trades);
            print_segment_table("By Pair", &trades, |t| t.pair_name.clone());
            print_segment_table("By Station", &trades, |t| t.station_id.clone());
            print_segment_table("By Strategy", &trades, |t| t.strategy.clone());
            let mae = print_calibration_table(&trades);

            if let Some(repo) = &repo_opt {
                let wins = trades
                    .iter()
                    .filter(|t| t.exit_reason == "TargetHit")
                    .count() as i64;
                let losses = trades.iter().filter(|t| t.exit_reason == "StopHit").count() as i64;
                let timeouts = trades
                    .iter()
                    .filter(|t| t.exit_reason == "Timeout" || t.exit_reason == "ManualClose")
                    .count() as i64;
                let summary = RunSummary {
                    run_id,
                    trade_count: trades.len() as i64,
                    win_count: wins,
                    loss_count: losses,
                    timeout_count: timeouts,
                    win_rate,
                    avg_pnl,
                    calibration_mae: mae,
                    computed_at: Utc::now().timestamp_millis(),
                };
                repo.persist_summary(&summary)
                    .await
                    .context("Failed to persist run summary")?;
                println!("\n✅  Summary persisted to run_summaries (run_id = {run_id}).");
            }
        } else {
            // ── Multi-run comparison view ─────────────────────────────────
            println!("═══════════════════════════════════════════════════════════");
            println!("  Cross-run comparison ({} runs)", run_ids.len());
            println!("═══════════════════════════════════════════════════════════\n");

            let mut comparison_rows: Vec<ComparisonRow> = Vec::new();

            for &run_id in &run_ids {
                let meta = fetch_run_meta(&pool, run_id).await?;
                let trades = fetch_trades(&pool, run_id).await?;

                // Quick stats (no detailed printing in multi-run mode)
                let (win_rate, avg_pnl) = compute_overall_stats(&trades);
                let mae = compute_calibration_mae(&trades);

                let dt: DateTime<Utc> = DateTime::from_timestamp_millis(meta.created_at)
                    .unwrap_or(DateTime::UNIX_EPOCH);
                let created_str = dt.format("%Y-%m-%d %H:%M").to_string();

                comparison_rows.push(ComparisonRow {
                    run_id: run_id.to_string(),
                    model_version: meta.model_version.clone(),
                    run_type: meta.run_type.clone(),
                    trade_count: meta.trade_count.to_string(),
                    win_rate: format!("{:.1}%", win_rate * 100.0),
                    avg_pnl: format!("{:+.3}%", avg_pnl * 100.0),
                    calibration_mae: mae
                        .map(|m| format!("{:.3}%", m * 100.0))
                        .unwrap_or_else(|| "N/A".to_string()),
                    created_at: created_str,
                    description: truncate(&meta.description, 40),
                });

                // Persist each run's summary if requested
                if let Some(repo) = &repo_opt {
                    let wins = trades
                        .iter()
                        .filter(|t| t.exit_reason == "TargetHit")
                        .count() as i64;
                    let losses =
                        trades.iter().filter(|t| t.exit_reason == "StopHit").count() as i64;
                    let timeouts = trades
                        .iter()
                        .filter(|t| t.exit_reason == "Timeout" || t.exit_reason == "ManualClose")
                        .count() as i64;
                    let summary = RunSummary {
                        run_id,
                        trade_count: trades.len() as i64,
                        win_count: wins,
                        loss_count: losses,
                        timeout_count: timeouts,
                        win_rate,
                        avg_pnl,
                        calibration_mae: mae,
                        computed_at: Utc::now().timestamp_millis(),
                    };
                    repo.persist_summary(&summary)
                        .await
                        .context("Failed to persist run summary")?;
                }
            }

            let mut table = Table::new(comparison_rows);
            table.with(Style::rounded());
            table.modify(Columns::single(3), Alignment::right()); // Trades
            println!("{table}");

            if repo_opt.is_some() {
                println!("\n✅  Summaries persisted to run_summaries.");
            }
        }

        Ok(())
    }

    // ─── DB helpers ──────────────────────────────────────────────────────────

    async fn open_pool(db_path: &str) -> Result<SqlitePool> {
        let opts = SqliteConnectOptions::from_str(&format!("sqlite://{db_path}"))?
            .create_if_missing(false)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(10))
            .read_only(true);

        SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await
            .with_context(|| format!("Cannot open database at '{db_path}'"))
    }

    async fn resolve_run_ids(pool: &SqlitePool, spec: &str) -> Result<Vec<i64>> {
        match spec.trim().to_lowercase().as_str() {
            "all" => {
                let rows = sqlx::query("SELECT id FROM runs ORDER BY id")
                    .fetch_all(pool)
                    .await?;
                Ok(rows.iter().map(|r| r.get::<i64, _>("id")).collect())
            }
            "latest" => {
                let row = sqlx::query(
                    "SELECT id FROM runs WHERE run_type != 'live' ORDER BY id DESC LIMIT 1",
                )
                .fetch_optional(pool)
                .await?;
                match row {
                    Some(r) => Ok(vec![r.get::<i64, _>("id")]),
                    None => {
                        // Fall back to the very latest row including live sentinel
                        let row2 = sqlx::query("SELECT id FROM runs ORDER BY id DESC LIMIT 1")
                            .fetch_optional(pool)
                            .await?;
                        Ok(row2
                            .map(|r| vec![r.get::<i64, _>("id")])
                            .unwrap_or_default())
                    }
                }
            }
            other => {
                let id: i64 = other.parse().map_err(|_| {
                    anyhow!(
                        "--run-id must be a positive integer, 'latest', or 'all' (got '{other}')"
                    )
                })?;
                // Verify it exists
                let exists: bool =
                    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs WHERE id = ?1")
                        .bind(id)
                        .fetch_one(pool)
                        .await
                        .map(|n| n > 0)?;

                if !exists {
                    return Err(anyhow!("run_id {id} not found in the runs table"));
                }
                Ok(vec![id])
            }
        }
    }

    async fn fetch_run_meta(pool: &SqlitePool, run_id: i64) -> Result<RunMeta> {
        let row = sqlx::query(
            r#"
            SELECT r.id, r.model_version, r.run_type, r.description, r.created_at,
                   COUNT(t.id) AS trade_count
            FROM runs r
            LEFT JOIN trades t ON t.run_id = r.id
            WHERE r.id = ?1
            GROUP BY r.id
            "#,
        )
        .bind(run_id)
        .fetch_one(pool)
        .await
        .with_context(|| format!("Could not fetch meta for run_id {run_id}"))?;

        Ok(RunMeta {
            id: row.get("id"),
            model_version: row.get("model_version"),
            run_type: row.get("run_type"),
            description: row.get("description"),
            created_at: row.get("created_at"),
            trade_count: row.get("trade_count"),
        })
    }

    async fn fetch_trades(pool: &SqlitePool, run_id: i64) -> Result<Vec<TradeSummaryRow>> {
        let rows = sqlx::query(
            r#"
            SELECT pair_name, station_id, strategy, exit_reason,
                   entry_price, exit_price, direction, predicted_win_rate
            FROM trades
            WHERE run_id = ?1
            "#,
        )
        .bind(run_id)
        .fetch_all(pool)
        .await
        .with_context(|| format!("Could not fetch trades for run_id {run_id}"))?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let entry_price: f64 = r.get("entry_price");
            let exit_price: f64 = r.get("exit_price");
            let exit_reason: String = r.get("exit_reason");
            let direction: String = r.get("direction");

            // Re-derive PnL the same way backtest.rs does it.
            let pnl_pct = compute_pnl(entry_price, exit_price, &direction, &exit_reason);

            out.push(TradeSummaryRow {
                pair_name: r.get("pair_name"),
                station_id: r.get("station_id"),
                strategy: r.get("strategy"),
                exit_reason,
                pnl_pct,
                predicted_win_rate: r.get("predicted_win_rate"),
            });
        }
        Ok(out)
    }

    // ─── PnL reconstruction ───────────────────────────────────────────────────

    /// Reconstruct PnL% from stored prices the same way backtest.rs does it.
    fn compute_pnl(entry: f64, exit: f64, direction: &str, _exit_reason: &str) -> f64 {
        if entry <= 0.0 {
            return 0.0;
        }
        match direction {
            "Long" => (exit - entry) / entry,
            "Short" => (entry - exit) / entry,
            _ => 0.0,
        }
    }

    // ─── Stats computations ───────────────────────────────────────────────────

    fn compute_overall_stats(trades: &[TradeSummaryRow]) -> (f64, f64) {
        if trades.is_empty() {
            return (0.0, 0.0);
        }
        let wins = trades
            .iter()
            .filter(|t| t.exit_reason == "TargetHit")
            .count();
        let avg_pnl = trades.iter().map(|t| t.pnl_pct).sum::<f64>() / trades.len() as f64;
        (wins as f64 / trades.len() as f64, avg_pnl)
    }

    fn compute_calibration_mae(trades: &[TradeSummaryRow]) -> Option<f64> {
        // Bucket predicted_win_rate into 10% bands [0,10%), [10,20%), ... [90,100%].
        // For each band: actual win rate = wins / trades_in_band.
        // MAE = mean over bands (weighted by trade count) of |predicted_mid - actual|.
        let with_pred: Vec<_> = trades
            .iter()
            .filter(|t| t.predicted_win_rate.is_some())
            .collect();

        if with_pred.is_empty() {
            return None;
        }

        // band index 0..=9
        let mut band_wins = [0usize; 10];
        let mut band_totals = [0usize; 10];

        for t in &with_pred {
            let p = t.predicted_win_rate.unwrap();
            let band = bucket_index(p);
            band_totals[band] += 1;
            if t.exit_reason == "TargetHit" {
                band_wins[band] += 1;
            }
        }

        let total_weighted: f64 = with_pred.len() as f64;
        let mut mae_acc = 0.0_f64;
        for b in 0..10 {
            if band_totals[b] == 0 {
                continue;
            }
            let predicted_mid = (b as f64 * 0.1) + 0.05; // mid-point of band
            let actual = band_wins[b] as f64 / band_totals[b] as f64;
            let weight = band_totals[b] as f64 / total_weighted;
            mae_acc += weight * (predicted_mid - actual).abs();
        }

        Some(mae_acc)
    }

    fn bucket_index(p: f64) -> usize {
        let p = p.clamp(0.0, 1.0);
        let b = (p * 10.0).floor() as usize;
        b.min(9)
    }

    // ─── Printing ─────────────────────────────────────────────────────────────

    fn print_run_header(meta: &RunMeta) {
        let dt: DateTime<Utc> =
            DateTime::from_timestamp_millis(meta.created_at).unwrap_or(DateTime::UNIX_EPOCH);
        println!("\n═══════════════════════════════════════════════════════════");
        println!(
            "  Run #{} — {} [{}]  |  {}",
            meta.id,
            meta.model_version,
            meta.run_type,
            dt.format("%Y-%m-%d %H:%M UTC")
        );
        println!("  {} trades  |  {}", meta.trade_count, meta.description);
        println!("═══════════════════════════════════════════════════════════\n");
    }

    /// Returns (win_rate, avg_pnl) for persistence use.
    fn print_overall_stats(trades: &[TradeSummaryRow]) -> (f64, f64) {
        if trades.is_empty() {
            println!("  (no trades for this run)\n");
            return (0.0, 0.0);
        }

        let total = trades.len();
        let wins = trades
            .iter()
            .filter(|t| t.exit_reason == "TargetHit")
            .count();
        let losses = trades.iter().filter(|t| t.exit_reason == "StopHit").count();
        let timeouts = trades
            .iter()
            .filter(|t| t.exit_reason == "Timeout" || t.exit_reason == "ManualClose")
            .count();
        let win_rate = wins as f64 / total as f64;
        let avg_pnl = trades.iter().map(|t| t.pnl_pct).sum::<f64>() / total as f64;

        let rows = vec![
            OverallRow {
                metric: "Total trades".to_string(),
                value: total.to_string(),
            },
            OverallRow {
                metric: "Wins".to_string(),
                value: format!("{wins}"),
            },
            OverallRow {
                metric: "Losses".to_string(),
                value: format!("{losses}"),
            },
            OverallRow {
                metric: "Timeouts / Manual".to_string(),
                value: format!("{timeouts}"),
            },
            OverallRow {
                metric: "Win rate".to_string(),
                value: format!("{:.1}%", win_rate * 100.0),
            },
            OverallRow {
                metric: "Avg PnL".to_string(),
                value: format!("{:+.3}%", avg_pnl * 100.0),
            },
        ];

        println!("── Overall ─────────────────────────────────────────────────\n");
        let mut table = Table::new(rows);
        table.with(Style::rounded());
        println!("{table}\n");

        (win_rate, avg_pnl)
    }

    fn print_segment_table(
        label: &str,
        trades: &[TradeSummaryRow],
        key_fn: impl Fn(&TradeSummaryRow) -> String,
    ) {
        if trades.is_empty() {
            return;
        }

        // Aggregate by key
        let mut map: HashMap<String, Vec<&TradeSummaryRow>> = HashMap::new();
        for t in trades {
            map.entry(key_fn(t)).or_default().push(t);
        }

        let mut keys: Vec<String> = map.keys().cloned().collect();
        keys.sort();

        let mut rows: Vec<SegmentRow> = keys
            .iter()
            .map(|k| {
                let group = &map[k];
                let total = group.len();
                let wins = group
                    .iter()
                    .filter(|t| t.exit_reason == "TargetHit")
                    .count();
                let losses = group.iter().filter(|t| t.exit_reason == "StopHit").count();
                let timeouts = total - wins - losses;
                let win_rate = wins as f64 / total as f64;
                let avg_pnl = group.iter().map(|t| t.pnl_pct).sum::<f64>() / total as f64;
                SegmentRow {
                    segment: k.clone(),
                    trades: total,
                    wins,
                    losses,
                    timeouts,
                    win_rate: format!("{:.1}%", win_rate * 100.0),
                    avg_pnl: format!("{:+.3}%", avg_pnl * 100.0),
                }
            })
            .collect();

        // Append a totals row
        let total = trades.len();
        let wins_all = trades
            .iter()
            .filter(|t| t.exit_reason == "TargetHit")
            .count();
        let losses_all = trades.iter().filter(|t| t.exit_reason == "StopHit").count();
        let timeouts_all = total - wins_all - losses_all;
        let win_rate_all = wins_all as f64 / total as f64;
        let avg_pnl_all = trades.iter().map(|t| t.pnl_pct).sum::<f64>() / total as f64;
        rows.push(SegmentRow {
            segment: "TOTAL".to_string(),
            trades: total,
            wins: wins_all,
            losses: losses_all,
            timeouts: timeouts_all,
            win_rate: format!("{:.1}%", win_rate_all * 100.0),
            avg_pnl: format!("{:+.3}%", avg_pnl_all * 100.0),
        });

        println!("── {label} ──────────────────────────────────────────────────\n");
        let mut table = Table::new(rows);
        table.with(Style::rounded());
        table.modify(Columns::single(1), Alignment::right()); // Trades
        table.modify(Columns::single(2), Alignment::right()); // Wins
        table.modify(Columns::single(3), Alignment::right()); // Losses
        table.modify(Columns::single(4), Alignment::right()); // Timeouts
        println!("{table}\n");
    }

    /// Prints the calibration table and returns the weighted MAE (if computable).
    fn print_calibration_table(trades: &[TradeSummaryRow]) -> Option<f64> {
        println!("── Calibration (predicted vs actual win rate) ───────────────\n");

        let with_pred: Vec<_> = trades
            .iter()
            .filter(|t| t.predicted_win_rate.is_some())
            .collect();

        if with_pred.is_empty() {
            println!(
                "  (No predicted_win_rate data available — this column is populated\n   \
                 from Phase 1d onwards once the backtest runs with the wired success_rate.)\n"
            );
            return None;
        }

        let mut band_wins = [0usize; 10];
        let mut band_totals = [0usize; 10];
        let mut band_pred_sum = [0.0_f64; 10];

        for t in &with_pred {
            let p = t.predicted_win_rate.unwrap();
            let band = bucket_index(p);
            band_totals[band] += 1;
            band_pred_sum[band] += p;
            if t.exit_reason == "TargetHit" {
                band_wins[band] += 1;
            }
        }

        let total_with_pred = with_pred.len() as f64;
        let mut calibration_rows: Vec<CalibrationRow> = Vec::new();
        let mut mae_acc = 0.0_f64;

        for b in 0..10 {
            let lo = b * 10;
            let hi = lo + 10;
            let band_label = format!("{lo}%–{hi}%");

            if band_totals[b] == 0 {
                calibration_rows.push(CalibrationRow {
                    band: band_label,
                    trades: 0,
                    actual_win_rate: "—".to_string(),
                    predicted_mid: format!("{:.0}%", (lo as f64 + 5.0)),
                    abs_error: "—".to_string(),
                });
                continue;
            }

            let actual = band_wins[b] as f64 / band_totals[b] as f64;
            let pred_avg = band_pred_sum[b] / band_totals[b] as f64;
            let abs_err = (pred_avg - actual).abs();
            let weight = band_totals[b] as f64 / total_with_pred;
            mae_acc += weight * abs_err;

            calibration_rows.push(CalibrationRow {
                band: band_label,
                trades: band_totals[b],
                actual_win_rate: format!("{:.1}%", actual * 100.0),
                predicted_mid: format!("{:.1}%", pred_avg * 100.0),
                abs_error: format!("{:.2}%", abs_err * 100.0),
            });
        }

        let mut table = Table::new(calibration_rows);
        table.with(Style::rounded());
        table.modify(Columns::single(1), Alignment::right());
        println!("{table}");
        println!(
            "\n  Weighted calibration MAE: {:.3}%  (over {} trades with predicted_win_rate)\n",
            mae_acc * 100.0,
            with_pred.len()
        );

        Some(mae_acc)
    }

    // ─── Utility ─────────────────────────────────────────────────────────────

    fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            s.to_string()
        } else {
            format!(
                "{}…",
                &s[..s
                    .char_indices()
                    .nth(max_chars - 1)
                    .map_or(s.len(), |(i, _)| i)]
            )
        }
    }
}

// ─── main ────────────────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    inner::run().await
}

#[cfg(target_arch = "wasm32")]
fn main() {}
