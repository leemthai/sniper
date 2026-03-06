// Walk-forward backtester (feature = backtest).
// Reserves `holdout_candles` and iteratively expands the training window `[0..split+i)`.
// 1. Truncates history to prevent look-ahead.
// 2. Runs simulations on the snapshot.
// 3. Replays opportunities against future hold-out data to determine outcomes.
// 4. Stores results in the shared `results.sqlite` for unified analysis.

pub(crate) const BACKTEST_PAIR_COUNT: usize = 10; // # pairs to process (actual pairs processed will be random from all loaded pairs coz HashSet unordered)
pub(crate) const BACKTEST_CANDLE_STRIDE: usize = 10; // # candles we stride across when backtesting
pub(crate) const BACKTEST_HOLDOUT_CANDLES: usize = 26_280; // ~3 months of 5-min candles
pub(crate) const BACKTEST_MIN_TRAINING_CANDLES: usize = 576; // ~48 h of 5-min candles — enough for a meaningful similarity scan
pub(crate) const BACKTEST_SKIP_DB_WRITE: bool = true;
pub(crate) const BACKTEST_MODEL_VERSION: &str = "mark-ii";
pub(crate) const BACKTEST_MODEL_DESC: &str = "Walk-forward backtest run";

use {
    crate::{
        app::{Pct, PhPct, Price, PriceLike},
        data::{ResultsRepositoryTrait, TradeResult},
        engine::{StationId, run_pathfinder_simulations},
        models::{
            OhlcvTimeSeries, OptimizationStrategy, TradeDirection, TradeOpportunity, TradeOutcome,
        },
        utils::TimeUtils,
    },
    chrono::{DateTime, Utc},
    rayon::prelude::*,
    std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    uuid::Uuid,
};

#[derive(Debug, Clone)]
pub(crate) struct BacktestConfig {
    pub ph_pct: PhPct,
    pub station_id: StationId,
    pub strategy: OptimizationStrategy,
    pub holdout_candles: usize,
    pub min_training_candles: usize,
    pub stride: usize,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            ph_pct: PhPct::DEFAULT,
            station_id: StationId::default(),
            strategy: OptimizationStrategy::default(),
            holdout_candles: BACKTEST_HOLDOUT_CANDLES,
            min_training_candles: BACKTEST_MIN_TRAINING_CANDLES,
            stride: BACKTEST_CANDLE_STRIDE,
        }
    }
}

// #[derive(Debug, Clone)]
pub(crate) struct BacktestReport {
    pub pair_name: String,
    pub config: BacktestConfig,
    pub opportunities_generated: usize,
    pub trades_resolved: usize,
    pub wins: usize,
    pub losses: usize,
    pub timeouts: usize,
    pub win_rate: Pct,
    pub avg_pnl: Pct,
}

// Run walk-forward backtest for one pair and persist every resolved trade to `repo`.
pub(crate) fn run_backtest(
    ohlcv: &OhlcvTimeSeries,
    config: &BacktestConfig,
    repo: &dyn ResultsRepositoryTrait,
    run_id: i64,
) -> Option<BacktestReport> {
    let pair_name = ohlcv.pair_interval.name.clone();
    let total_candles = ohlcv.klines();

    let split = total_candles.saturating_sub(config.holdout_candles);
    if split < config.min_training_candles {
        println!(
            "[backtest] {}: not enough training data \
             (total={}, holdout={}, split={}, min_training={}). Skipping.",
            pair_name, total_candles, config.holdout_candles, split, config.min_training_candles,
        );
        return None;
    }

    println!(
        "[backtest] {} with {} Rayon threads made available | strategy={:?} | ph_pct={} | split={} | holdout={} candles",
        pair_name,
        rayon::current_num_threads(),
        config.strategy,
        config.ph_pct,
        split,
        config.holdout_candles,
    );

    let opportunities_generated = AtomicUsize::new(0);
    let wins = AtomicUsize::new(0);
    let losses = AtomicUsize::new(0);
    let timeouts = AtomicUsize::new(0);
    let trades_resolved = AtomicUsize::new(0);
    let total_pnl_pct = Mutex::new(0.0_f64);

    (0..config.holdout_candles)
        .step_by(config.stride)
        .collect::<Vec<_>>()
        .par_iter()
        .for_each(|&i| {
            let train_end = split + i;
            if train_end < config.min_training_candles {
                return;
            }
            let current_idx = train_end.saturating_sub(1);
            if current_idx >= total_candles {
                return;
            }

            let training_slice = truncate_ohlcv(ohlcv, train_end);
            let current_price = Price::from(training_slice.close_prices[current_idx]);

            if !current_price.is_positive() {
                return;
            }

            let pf_result = run_pathfinder_simulations(
                &training_slice,
                current_price,
                config.ph_pct,
                config.strategy,
                config.station_id,
                None,
            );

            if pf_result.opportunities.is_empty() {
                return;
            }

            opportunities_generated.fetch_add(pf_result.opportunities.len(), Ordering::Relaxed);

            for opp in &pf_result.opportunities {
                let entry_ts_ms = ohlcv.timestamps[current_idx];
                let entry_time: DateTime<Utc> = TimeUtils::ms_to_datetime(entry_ts_ms);
                let max_duration = opp.max_duration;
                let expiry_time = entry_time
                    + chrono::Duration::from_std(std::time::Duration::from_millis(
                        max_duration.value().max(0) as u64,
                    ))
                    .unwrap_or(chrono::Duration::days(365));

                let outcome = replay_opportunity_forward(ohlcv, opp, train_end, expiry_time);

                let exit_candle_idx = outcome.exit_candle_idx;
                let exit_price = if exit_candle_idx < total_candles {
                    let c = ohlcv.get_candle(exit_candle_idx);
                    match outcome.result {
                        TradeOutcome::TargetHit => Price::from(opp.target_price),
                        TradeOutcome::StopHit => Price::from(opp.stop_price),
                        TradeOutcome::Timeout | TradeOutcome::ManualClose => {
                            Price::from(c.close_price)
                        }
                    }
                } else {
                    Price::from(ohlcv.close_prices[total_candles - 1])
                };

                let exit_ts_ms = if exit_candle_idx < total_candles {
                    ohlcv.timestamps[exit_candle_idx]
                } else {
                    ohlcv.timestamps[total_candles - 1]
                };

                let pnl_pct = match outcome.result {
                    TradeOutcome::TargetHit => {
                        wins.fetch_add(1, Ordering::Relaxed);
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
                        losses.fetch_add(1, Ordering::Relaxed);
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
                        timeouts.fetch_add(1, Ordering::Relaxed);
                        match opp.direction {
                            TradeDirection::Long => (exit_price - current_price) / current_price,
                            TradeDirection::Short => (current_price - exit_price) / current_price,
                        }
                    }
                };

                trades_resolved.fetch_add(1, Ordering::Relaxed);
                *total_pnl_pct.lock().unwrap() += pnl_pct;

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
                    run_id,
                    predicted_win_rate: None,
                };

                if !BACKTEST_SKIP_DB_WRITE {
                    if let Err(e) = repo.enqueue(trade_result) {
                        log::error!(
                            "[backtest] DB enqueue failed for {} at candle {}: {:?}",
                            pair_name,
                            train_end,
                            e
                        );
                    }
                }
            }
        });

    // Convert atomics back to regular values
    let opportunities_generated = opportunities_generated.load(Ordering::Relaxed);
    let wins = wins.load(Ordering::Relaxed);
    let losses = losses.load(Ordering::Relaxed);
    let timeouts = timeouts.load(Ordering::Relaxed);
    let trades_resolved = trades_resolved.load(Ordering::Relaxed);
    let total_pnl_pct = *total_pnl_pct.lock().unwrap();

    let (win_rate, avg_pnl) = if trades_resolved > 0 {
        let tr = trades_resolved as f64;
        (Pct::new(wins as f64 / tr), Pct::new(total_pnl_pct / tr))
    } else {
        (Pct::new(0.0), Pct::new(0.0))
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

    println!(
        "[backtest] {} COMPLETE | ops_generated={} | resolved={} | \
         wins={} | losses={} | timeouts={} | win_rate={} | avg_pnl={}",
        pair_name,
        opportunities_generated,
        trades_resolved,
        wins,
        losses,
        timeouts,
        win_rate,
        avg_pnl,
    );

    Some(report)
}

// Resolved outcome of replaying one opportunity forward.
struct ReplayResult {
    result: TradeOutcome,
    exit_candle_idx: usize, // Candle index where trade exited (or last available candle)
}

// Replay a [`TradeOpportunity`] forward into real OHLCV data start at `start_idx` (first hold-out candle), checking each candle's high/low against target and stop prices, then expiry time.
// Mirrors pessimistic logic of [`TradeOpportunity::check_exit_condition`]: stop is checked before target on each candle.
fn replay_opportunity_forward(
    ohlcv: &OhlcvTimeSeries,
    opp: &TradeOpportunity,
    start_idx: usize,
    expiry_time: DateTime<Utc>,
) -> ReplayResult {
    let total = ohlcv.klines();
    let target_price = Price::from(opp.target_price);
    let stop_price = Price::from(opp.stop_price);

    for idx in start_idx..total {
        let c = ohlcv.get_candle(idx);
        let candle_time: DateTime<Utc> = TimeUtils::ms_to_datetime(c.timestamp_ms);

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
    // let _ = entry_time; // suppress unused warning in case it's only used above
    ReplayResult {
        result: TradeOutcome::Timeout,
        exit_candle_idx: total.saturating_sub(1),
    }
}

// Create a truncated clone of `ohlcv` containing only `[0, end_idx)`.
// The clone is bounded by `end_idx` which is at most a few thousand candles larger than training window — acceptable for offline tool.
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
