//! Walk-forward backtest runner.
//!
//! Enabled via the `backtest` Cargo feature. Entry point: [`run_backtest`].
//!
//! # Approach
//! Given a full [`OhlcvTimeSeries`] for one pair, the most recent `holdout_candles`
//! are reserved as the out-of-sample hold-out set. The training window grows from
//! `[0, split)` to `[0, split + i)` as we walk forward candle-by-candle through the
//! hold-out period. For each hold-out candle `i`:
//!
//! 1. A truncated clone of the series (`[0, split + i)`) is created — the engine
//!    never sees the future.
//! 2. [`run_pathfinder_simulations`] is called on the truncated series using the
//!    close price of candle `split + i - 1` as the current price.
//! 3. Every generated [`TradeOpportunity`] is replayed forward into the real hold-out
//!    data starting at index `split + i`, using [`check_exit_condition`] logic, and
//!    the outcome is recorded as a [`TradeResult`].
//! 4. All results are enqueued into the provided [`ResultsRepositoryTrait`] (same
//!    `results.sqlite` schema used by the live engine — Phase 1b `analyze` CLI reads
//!    both together).
//!
//! The walk is intentionally simple and single-threaded per pair; the caller can
//! parallelise across pairs with rayon if desired.

#[cfg(feature = "backtest")]
use {
    crate::{
        config::{PhPct, Price, PriceLike, StationId},
        data::{ResultsRepositoryTrait, TradeResult},
        engine::run_pathfinder_simulations,
        models::{
            OhlcvTimeSeries, OptimizationStrategy, TradeDirection, TradeOpportunity, TradeOutcome,
        },
    },
    chrono::{DateTime, TimeZone, Utc},
    uuid::Uuid,
};

// ─── Public config ────────────────────────────────────────────────────────────

/// Configuration for a single walk-forward backtest run.
#[cfg(feature = "backtest")]
#[derive(Debug, Clone)]
pub struct BacktestConfig {
    /// Price-horizon percentage passed to the pathfinder.
    pub ph_pct: PhPct,
    /// Station label stored on every result row (cosmetic — use `StationId::default()` if
    /// you have not tuned the pair).
    pub station_id: StationId,
    /// Which objective the pathfinder optimises.
    pub strategy: OptimizationStrategy,
    /// How many trailing candles to reserve as the hold-out set.
    /// At 5-min resolution: 3 months ≈ 26_280 candles.
    pub holdout_candles: usize,
    /// Minimum number of training candles required before we start generating
    /// opportunities. A sensible floor is ~500 (≈ 42 h at 5 min).
    pub min_training_candles: usize,
}

#[cfg(feature = "backtest")]
impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            ph_pct: PhPct::DEFAULT,
            station_id: StationId::default(),
            strategy: OptimizationStrategy::default(),
            // ~3 months of 5-min candles
            holdout_candles: 26_280,
            // ~48 h of 5-min candles — enough for a meaningful similarity scan
            min_training_candles: 576,
        }
    }
}

// ─── Per-trade result (mirrors TradeResult but with an extra `source` tag) ────

/// Summary statistics for one completed backtest.
#[cfg(feature = "backtest")]
#[derive(Debug, Clone)]
pub struct BacktestReport {
    pub pair_name: String,
    pub config: BacktestConfig,
    /// Total number of opportunities the pathfinder generated during the walk.
    pub opportunities_generated: usize,
    /// Subset that resolved before the end of the hold-out window.
    pub trades_resolved: usize,
    pub wins: usize,
    pub losses: usize,
    pub timeouts: usize,
    pub win_rate: f64,
    /// Mean PnL across resolved trades (fractional, e.g. 0.02 = +2 %).
    pub avg_pnl: f64,
}

// ─── Main entry point ─────────────────────────────────────────────────────────

/// Run a walk-forward backtest for one pair and persist every resolved trade to
/// `repo`.
///
/// Returns a [`BacktestReport`] with aggregate statistics.
/// Non-blocking (no async) — drive from a dedicated thread.
#[cfg(feature = "backtest")]
pub fn run_backtest(
    ohlcv: &OhlcvTimeSeries,
    config: &BacktestConfig,
    repo: &dyn ResultsRepositoryTrait,
) -> BacktestReport {
    let pair_name = ohlcv.pair_interval.name.clone();
    let total_candles = ohlcv.klines();

    // ── Validate that we have enough data ─────────────────────────────────
    let split = total_candles.saturating_sub(config.holdout_candles);
    if split < config.min_training_candles {
        log::warn!(
            "[backtest] {}: not enough training data \
             (total={}, holdout={}, split={}, min_training={}). Skipping.",
            pair_name,
            total_candles,
            config.holdout_candles,
            split,
            config.min_training_candles,
        );
        return BacktestReport {
            pair_name,
            config: config.clone(),
            opportunities_generated: 0,
            trades_resolved: 0,
            wins: 0,
            losses: 0,
            timeouts: 0,
            win_rate: 0.0,
            avg_pnl: 0.0,
        };
    }

    log::info!(
        "[backtest] {} | strategy={:?} | ph_pct={:.3} | split={} | holdout={} candles",
        pair_name,
        config.strategy,
        config.ph_pct.value(),
        split,
        config.holdout_candles,
    );

    let mut opportunities_generated: usize = 0;
    let mut wins: usize = 0;
    let mut losses: usize = 0;
    let mut timeouts: usize = 0;
    let mut total_pnl: f64 = 0.0;
    let mut trades_resolved: usize = 0;

    // ── Walk forward through the hold-out window ───────────────────────────
    //
    // We step by 1 candle at a time.  In a real production environment you
    // might step by larger strides (e.g. every `sim_duration / 4` candles) to
    // avoid generating highly correlated opportunities.  For now, every candle
    // is evaluated to maximise the number of data points.
    for i in 0..config.holdout_candles {
        let train_end = split + i; // exclusive upper bound of training window
        if train_end < config.min_training_candles {
            continue;
        }
        // The "current" candle is the last candle in the training window.
        let current_idx = train_end.saturating_sub(1);
        if current_idx >= total_candles {
            break;
        }

        // Build a truncated view — the pathfinder must not see the future.
        let training_slice = truncate_ohlcv(ohlcv, train_end);
        let current_price = Price::from(training_slice.close_prices[current_idx]);

        if !current_price.is_positive() {
            continue;
        }

        // ── Run pathfinder on training data ───────────────────────────────
        let pf_result = run_pathfinder_simulations(
            &training_slice,
            current_price,
            config.ph_pct,
            config.strategy,
            config.station_id,
            None, // CVA computed internally
        );

        if pf_result.opportunities.is_empty() {
            continue;
        }

        opportunities_generated += pf_result.opportunities.len();

        // ── Replay each opportunity forward into the real hold-out data ───
        for opp in &pf_result.opportunities {
            let entry_ts_ms = ohlcv.timestamps[current_idx];
            let entry_time: DateTime<Utc> = ts_ms_to_datetime(entry_ts_ms);
            let max_duration = opp.max_duration;
            let expiry_time = entry_time
                + chrono::Duration::from_std(std::time::Duration::from_millis(
                    max_duration.value().max(0) as u64,
                ))
                .unwrap_or(chrono::Duration::days(365));

            let outcome = replay_opportunity_forward(
                ohlcv,
                opp,
                train_end, // first hold-out candle index
                entry_time,
                expiry_time,
            );

            let exit_candle_idx = outcome.exit_candle_idx;

            // Determine exit price from the candle where the trade resolved.
            let exit_price = if exit_candle_idx < total_candles {
                let c = ohlcv.get_candle(exit_candle_idx);
                match outcome.result {
                    TradeOutcome::TargetHit => Price::from(opp.target_price),
                    TradeOutcome::StopHit => Price::from(opp.stop_price),
                    TradeOutcome::Timeout | TradeOutcome::ManualClose => Price::from(c.close_price),
                }
            } else {
                // We ran off the end of the data — treat as timeout at last close.
                Price::from(ohlcv.close_prices[total_candles - 1])
            };

            let exit_ts_ms = if exit_candle_idx < total_candles {
                ohlcv.timestamps[exit_candle_idx]
            } else {
                ohlcv.timestamps[total_candles - 1]
            };

            // ── Accumulate aggregate stats ─────────────────────────────────
            let pnl = match outcome.result {
                TradeOutcome::TargetHit => {
                    wins += 1;
                    match opp.direction {
                        TradeDirection::Long => {
                            (Price::from(opp.target_price) - current_price) / current_price
                        }
                        TradeDirection::Short => {
                            (current_price - Price::from(opp.target_price)) / current_price
                        }
                    }
                }
                TradeOutcome::StopHit => {
                    losses += 1;
                    match opp.direction {
                        TradeDirection::Long => {
                            (Price::from(opp.stop_price) - current_price) / current_price
                        }
                        TradeDirection::Short => {
                            (current_price - Price::from(opp.stop_price)) / current_price
                        }
                    }
                }
                TradeOutcome::Timeout | TradeOutcome::ManualClose => {
                    timeouts += 1;
                    match opp.direction {
                        TradeDirection::Long => (exit_price - current_price) / current_price,
                        TradeDirection::Short => (current_price - exit_price) / current_price,
                    }
                }
            };

            trades_resolved += 1;
            total_pnl += pnl;

            // ── Write to results.sqlite ────────────────────────────────────
            let trade_id = Uuid::new_v4().to_string();
            let trade_result = TradeResult {
                trade_id,
                pair_name: pair_name.clone(),
                direction: opp.direction,
                entry_price: current_price,
                exit_price,
                stop_price: opp.stop_price,
                target_price: opp.target_price,
                exit_reason: outcome.result,
                entry_time: entry_ts_ms,
                exit_time: exit_ts_ms,
                planned_expiry_time: expiry_time.timestamp_millis(),
                strategy: opp.strategy,
                station_id: opp.station_id,
                market_state: opp.market_state,
                ph_pct: opp.ph_pct,
            };

            if let Err(e) = repo.enqueue(trade_result) {
                log::error!(
                    "[backtest] DB enqueue failed for {} at candle {}: {:?}",
                    pair_name,
                    train_end,
                    e,
                );
            }
        }
    }

    let win_rate = if trades_resolved > 0 {
        wins as f64 / trades_resolved as f64
    } else {
        0.0
    };
    let avg_pnl = if trades_resolved > 0 {
        total_pnl / trades_resolved as f64
    } else {
        0.0
    };

    let report = BacktestReport {
        pair_name: pair_name.clone(),
        config: config.clone(),
        opportunities_generated,
        trades_resolved,
        wins,
        losses,
        timeouts,
        win_rate,
        avg_pnl,
    };

    log::info!(
        "[backtest] {} COMPLETE | ops_generated={} | resolved={} | \
         wins={} | losses={} | timeouts={} | win_rate={:.1}% | avg_pnl={:.3}%",
        pair_name,
        opportunities_generated,
        trades_resolved,
        wins,
        losses,
        timeouts,
        win_rate * 100.0,
        avg_pnl * 100.0,
    );

    report
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Resolved outcome of replaying one opportunity forward.
#[cfg(feature = "backtest")]
struct ReplayResult {
    result: TradeOutcome,
    /// Index of the candle at which the trade exited (or the last available candle).
    exit_candle_idx: usize,
}

/// Replay a [`TradeOpportunity`] forward into the real OHLCV data starting at
/// `start_idx` (the first hold-out candle), checking each candle's high/low
/// against target and stop prices, then expiry time.
///
/// Mirrors the pessimistic logic of [`TradeOpportunity::check_exit_condition`]:
/// stop is checked before target on each candle.
#[cfg(feature = "backtest")]
fn replay_opportunity_forward(
    ohlcv: &OhlcvTimeSeries,
    opp: &TradeOpportunity,
    start_idx: usize,
    entry_time: DateTime<Utc>,
    expiry_time: DateTime<Utc>,
) -> ReplayResult {
    let total = ohlcv.klines();
    let target_price = Price::from(opp.target_price);
    let stop_price = Price::from(opp.stop_price);

    for idx in start_idx..total {
        let c = ohlcv.get_candle(idx);
        let candle_time: DateTime<Utc> = ts_ms_to_datetime(c.timestamp_ms);

        // Timeout check first (same order as check_exit_condition)
        if candle_time > expiry_time {
            return ReplayResult {
                result: TradeOutcome::Timeout,
                exit_candle_idx: idx,
            };
        }

        let high = Price::from(c.high_price);
        let low = Price::from(c.low_price);

        match opp.direction {
            TradeDirection::Long => {
                // Pessimistic: stop before target
                if low <= stop_price {
                    return ReplayResult {
                        result: TradeOutcome::StopHit,
                        exit_candle_idx: idx,
                    };
                }
                if high >= target_price {
                    return ReplayResult {
                        result: TradeOutcome::TargetHit,
                        exit_candle_idx: idx,
                    };
                }
            }
            TradeDirection::Short => {
                if high >= stop_price {
                    return ReplayResult {
                        result: TradeOutcome::StopHit,
                        exit_candle_idx: idx,
                    };
                }
                if low <= target_price {
                    return ReplayResult {
                        result: TradeOutcome::TargetHit,
                        exit_candle_idx: idx,
                    };
                }
            }
        }
    }

    // Reached the end of available data without a resolution.
    let _ = entry_time; // suppress unused warning in case it's only used above
    ReplayResult {
        result: TradeOutcome::Timeout,
        exit_candle_idx: total.saturating_sub(1),
    }
}

/// Create a truncated clone of `ohlcv` containing only `[0, end_idx)`.
///
/// This is intentionally a full clone rather than a zero-copy view because
/// `OhlcvTimeSeries` is owned and the pathfinder takes `&OhlcvTimeSeries`.
/// The clone is bounded by `end_idx` which is at most a few thousand candles
/// larger than the training window — acceptable for an offline tool.
#[cfg(feature = "backtest")]
fn truncate_ohlcv(ohlcv: &OhlcvTimeSeries, end_idx: usize) -> OhlcvTimeSeries {
    let n = end_idx.min(ohlcv.klines());
    OhlcvTimeSeries {
        pair_interval: ohlcv.pair_interval.clone(),
        first_kline_timestamp_ms: ohlcv.first_kline_timestamp_ms,
        timestamps: ohlcv.timestamps[..n].to_vec(),
        open_prices: ohlcv.open_prices[..n].to_vec(),
        high_prices: ohlcv.high_prices[..n].to_vec(),
        low_prices: ohlcv.low_prices[..n].to_vec(),
        close_prices: ohlcv.close_prices[..n].to_vec(),
        base_asset_volumes: ohlcv.base_asset_volumes[..n].to_vec(),
        quote_asset_volumes: ohlcv.quote_asset_volumes[..n].to_vec(),
        relative_volumes: ohlcv.relative_volumes[..n].to_vec(),
    }
}

/// Convert a Unix-millisecond timestamp to a `DateTime<Utc>`.
#[cfg(feature = "backtest")]
fn ts_ms_to_datetime(ts_ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ts_ms)
        .single()
        .unwrap_or(DateTime::<Utc>::from(std::time::UNIX_EPOCH))
}
