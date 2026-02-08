use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, RwLock};

#[cfg(not(target_arch = "wasm32"))]
use {crate::config::PERSISTENCE, std::path::Path, std::thread, tokio::runtime::Runtime};

#[cfg(any(debug_assertions, not(target_arch = "wasm32")))]
use crate::config::DF;

use crate::config::{
    BASE_INTERVAL, OptimizationStrategy, PhPct, Price, PriceLike, QuoteVol, StationId,
};

use crate::data::price_stream::PriceStreamManager;
use crate::data::results_repo::TradeResult;

#[cfg(not(target_arch = "wasm32"))]
use crate::data::results_repo::{ResultsRepository, ResultsRepositoryTrait};

use crate::data::timeseries::TimeSeriesCollection;

use crate::models::ledger::OpportunityLedger;
use crate::models::{
    DEFAULT_JOURNEY_SETTINGS, LiveCandle, PRICE_RECALC_THRESHOLD_PCT, TradeDirection,
    TradeFinderRow, TradeOpportunity, TradeOutcome, TradingModel, find_matching_ohlcv,
};

use crate::shared::SharedConfiguration;

use crate::utils::TimeUtils;
use crate::utils::time_utils::AppInstant;

use super::messages::{JobMode, JobRequest, JobResult};
use super::worker;

/// Represents the state of a single pair in the engine.
#[derive(Debug, Clone)]
pub(crate) struct PairRuntime {
    pub model: Option<Arc<TradingModel>>,

    /// Metadata for the trigger system
    pub last_update_price: Price,
    /// Is a worker currently crunching this pair?
    pub is_calculating: bool,
    /// Last error (if any) to show in UI
    pub last_error: Option<String>,
}

impl PairRuntime {
    pub(crate) fn new() -> Self {
        Self {
            model: None,
            last_update_price: Price::default(),
            is_calculating: false,
            last_error: None,
        }
    }
}

pub(crate) struct EngineJob {
    pub pair: String,
    pub price_override: Option<Price>,
    pub ph_pct: PhPct,
    pub strategy: OptimizationStrategy,
    pub station_id: StationId,
    pub mode: JobMode,
}

pub struct SniperEngine {
    // Definitive list of pairs we use in engine - initialized from app ONE TIME at startup
    pub(crate) active_engine_pairs: Vec<String>,

    /// Pair registry
    pub(crate) pairs_states: HashMap<String, PairRuntime>, // Keep track of the state of all pairs (not part of SharedConfiguration coz we don't need to save)

    pub(crate) shared_config: SharedConfiguration, // Share information between ui and engine (for variables either side can update via Arc<RwLock)
    // pub engine_strategy: OptimizationStrategy,

    // The ledger
    pub(crate) engine_ledger: OpportunityLedger,
    // Maintenance Timer (Runs in Release & Debug)
    pub(crate) last_ledger_maintenance: AppInstant,

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) results_repo: Arc<dyn ResultsRepositoryTrait>,

    /// Shared immutable data
    pub(crate) timeseries: Arc<RwLock<TimeSeriesCollection>>,

    // Live Data Channels
    candle_rx: Receiver<LiveCandle>,

    /// Live Data Feed
    pub(crate) price_stream: Arc<PriceStreamManager>,

    // Common Channels
    job_tx: Sender<JobRequest>,     // UI writes to this
    result_rx: Receiver<JobResult>, // UI reads from this

    // WASM ONLY: The Engine acts as the Worker, so it needs the "Worker Ends" of the channels
    #[cfg(target_arch = "wasm32")]
    job_rx: Receiver<JobRequest>,
    #[cfg(target_arch = "wasm32")]
    result_tx: Sender<JobResult>,

    /// This is job queue runtime variables
    pub(crate) queue: VecDeque<EngineJob>,
}

impl SniperEngine {
    /// Initialize the engine, spawn workers, and start the price stream.
    pub(crate) fn new(
        timeseries: TimeSeriesCollection,
        shared_config: SharedConfiguration,
        active_engine_pairs: Vec<String>,
    ) -> Self {
        // 1. Create Channels
        let (_candle_tx, candle_rx) = channel();
        let (job_tx, job_rx) = channel::<JobRequest>();
        let (result_tx, result_rx) = channel::<JobResult>();

        // 2. Create the Thread-Safe Data Structure ONCE
        // Wrap the collection in RwLock (for writing) and Arc (for sharing)
        let timeseries_arc = Arc::new(RwLock::new(timeseries));

        // NATIVE: Pass the receiver to the thread.
        #[cfg(not(target_arch = "wasm32"))]
        worker::spawn_worker_thread(job_rx, result_tx);

        // Initialize pair states
        let mut pairs_states = HashMap::new();
        {
            for pair in active_engine_pairs.clone() {
                pairs_states.insert(pair, PairRuntime::new());
            }
            #[cfg(debug_assertions)]
            if DF.log_pairs {
                log::info!(
                    "SniperEngine::new() Initializing runtime PairRunTimef or the following {} pairs: {:?} ",
                    active_engine_pairs.len(),
                    active_engine_pairs
                )
            };
        }

        // 4. Initialize Price Stream
        let price_stream = {
            #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
            let mut price_manager = PriceStreamManager::new();

            #[cfg(not(target_arch = "wasm32"))]
            price_manager.set_candle_sender(_candle_tx.clone());

            let price_stream = Arc::new(price_manager);
            price_stream.subscribe_all(active_engine_pairs.clone());
            price_stream
        };

        // Initialize Results Repository (native only)
        #[cfg(not(target_arch = "wasm32"))]
        let repo = {
            let db_path = Path::new(PERSISTENCE.kline.directory)
                .parent()
                .unwrap_or(Path::new("."))
                .join("results.sqlite");

            let db_path_str = db_path.to_str().unwrap_or("results.sqlite");

            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                ResultsRepository::new(db_path_str)
                    .await
                    .unwrap_or_else(|e| {
                        log::error!("Failed to init results.sqlite: {}", e);
                        panic!("Critical Error: Results DB init failed");
                    })
            })
        };

        // Construct Engine
        Self {
            active_engine_pairs,
            pairs_states,
            shared_config,
            engine_ledger: OpportunityLedger::new(),
            timeseries: timeseries_arc,
            price_stream,
            candle_rx,
            job_tx,
            result_rx,
            // WASM: Store the handles so they don't get dropped
            #[cfg(target_arch = "wasm32")]
            job_rx,
            #[cfg(target_arch = "wasm32")]
            result_tx,
            queue: VecDeque::new(),
            #[cfg(not(target_arch = "wasm32"))]
            results_repo: Arc::new(repo),
            last_ledger_maintenance: AppInstant::now(),
        }
    }

    /// Generates the master list for the Trade Finder.
    pub(crate) fn get_trade_finder_rows(
        &self,
        overrides: Option<&HashMap<String, Price>>,
    ) -> Vec<TradeFinderRow> {
        crate::trace_time!("Core: Get TradeFinder Rows", 2000, {
            let mut rows = Vec::new();

            let now_ms = TimeUtils::now_timestamp_ms();
            let day_ms = 86_400_000;

            // 1. Group Ledger Opportunities by Pair for fast lookup
            let mut ops_by_pair: HashMap<String, Vec<&TradeOpportunity>> = HashMap::new();
            for op in self.engine_ledger.get_all() {
                ops_by_pair
                    .entry(op.pair_name.clone())
                    .or_default()
                    .push(op);
            }

            let ts_guard = self.timeseries.read().unwrap();

            for (pair, _state) in &self.pairs_states {
                // 2. Get Context (Price)
                // STRICT MODE: Do not default to 0.0. If no price, skip the pair.
                let price_opt = overrides
                    .and_then(|map| map.get(pair).copied())
                    .or_else(|| self.price_stream.get_price(pair));

                let current_price = match price_opt {
                    Some(p) if p.is_positive() => p,
                    _ => continue,
                };

                // 3. Calculate Volume & Market State (From TimeSeries)
                // We do this for every pair regardless of whether it has ops
                let mut vol_24h = QuoteVol::new(0.0);

                if let Some(ts) = ts_guard
                    .series_data
                    .iter()
                    .find(|t| t.pair_interval.name() == pair)
                {
                    let count = ts.klines();
                    if count > 0 {
                        let current_idx = count - 1;
                        // m_state = MarketState::calculate(ts, current_idx, lookback);
                        for i in (0..=current_idx).rev() {
                            let c = ts.get_candle(i);
                            if now_ms - c.timestamp_ms > day_ms {
                                break;
                            }
                            vol_24h += c.quote_asset_volume;
                        }
                    }
                }

                // 4. Retrieve & Explode Opportunities (From Ledger)
                // Default to empty slice if no ops found for this pair
                let raw_ops = ops_by_pair.get(pair).map(|v| v.as_slice()).unwrap_or(&[]);

                // Filter worthwhile trades (Static ROI check)
                let valid_ops: Vec<&TradeOpportunity> = raw_ops
                    .iter()
                    .filter(|&&op| op.expected_roi().is_positive())
                    .map(|&op| op)
                    .collect();

                let total_ops = valid_ops.len();

                if total_ops > 0 {
                    // Create a ROW for EACH Opportunity
                    for op in valid_ops {
                        rows.push(TradeFinderRow {
                            pair_name: pair.clone(),
                            quote_volume_24h: vol_24h,
                            market_state: Some(op.market_state),
                            // opportunity_count_total: total_ops,
                            opportunity: Some(op.clone()),
                            current_price: current_price,
                        });
                    }
                } else {
                    // No valid trades, push 1 placeholder row (for "All Pairs" view)
                    rows.push(TradeFinderRow {
                        pair_name: pair.clone(),
                        quote_volume_24h: vol_24h,
                        market_state: None,
                        // opportunity_count_total: 0,
                        opportunity: None,
                        current_price: current_price,
                    });
                }
            }
            rows
        })
    }

    /// THE GAME LOOP.
    pub(crate) fn update(&mut self) {
        // Ingest Live Data (The Heartbeat)
        let t1 = AppInstant::now();
        self.process_live_data();
        let d1 = t1.elapsed().as_micros();

        // WASM ONLY: Process jobs manually in the main thread
        #[cfg(target_arch = "wasm32")]
        {
            // Non-blocking check for work
            if let Ok(req) = self.job_rx.try_recv() {
                // Run sync calculation
                worker::process_request_sync(req, self.result_tx.clone());
            }
        }

        // Maintenance loop - checks for drifting trades that have overlapped and merges them.
        let journey_settings = &DEFAULT_JOURNEY_SETTINGS;

        if t1.duration_since(self.last_ledger_maintenance).as_secs()
            >= journey_settings.optimization.prune_interval_sec
        {
            // Pass the Tolerance AND the Profile (Strategy)
            self.engine_ledger
                .prune_collisions(journey_settings.optimization.fuzzy_match_tolerance);
            self.last_ledger_maintenance = t1;
        }

        // Results
        let t2 = AppInstant::now();
        while let Ok(result) = self.result_rx.try_recv() {
            self.handle_job_result(result);
        }
        let d2 = t2.elapsed().as_micros();

        // Enqueue any pairs that have changed price significantly
        let t3 = AppInstant::now();
        self.trigger_recalcs_on_price_changes();
        let d3 = t3.elapsed().as_micros();

        // Queue
        let t4 = AppInstant::now();
        self.process_queue();
        let d4 = t4.elapsed().as_micros();

        // Log if total is significant (> 10ms)
        let total = d1 + d2 + d3 + d4;
        if total > 100_000 {
            log::warn!(
                "🐢 ENGINE SLOW: Live: {}us | Results: {}us | Triggers: {}us | Queue: {}us",
                d1,
                d2,
                d3,
                d4
            );
        }
    }

    /// Accessor for UI
    pub(crate) fn get_model(&self, pair: &str) -> Option<Arc<TradingModel>> {
        self.pairs_states
            .get(pair)
            .and_then(|state| state.model.clone())
    }

    pub(crate) fn get_price(&self, pair: &str) -> Option<Price> {
        self.price_stream.get_price(pair)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn set_stream_suspended(&self, suspended: bool) {
        if suspended {
            self.price_stream.suspend();
        } else {
            self.price_stream.resume();
        }
    }

    pub(crate) fn get_all_pair_names(&self) -> Vec<String> {
        self.timeseries.read().unwrap().unique_pair_names()
    }

    pub(crate) fn get_queue_len(&self) -> usize {
        self.queue.len()
    }

    pub(crate) fn get_worker_status_msg(&self) -> Option<String> {
        let calculating_pair = self
            .pairs_states
            .iter()
            .find(|(_, state)| state.is_calculating)
            .map(|(name, _)| name.clone());

        if let Some(pair) = calculating_pair {
            Some(format!("Processing {}", pair))
        } else if !self.queue.is_empty() {
            Some(format!("Queued: {}", self.queue.len()))
        } else {
            None
        }
    }

    pub(crate) fn get_pair_status(&self, pair: &str) -> (bool, Option<String>) {
        if let Some(state) = self.pairs_states.get(pair) {
            (state.is_calculating, state.last_error.clone())
        } else {
            (false, None)
        }
    }

    /// The trigger_global_recalc function is a "Reset Button" for the engine's work queue. Its purpose is to cancel any
    /// pending jobs and immediately schedule a fresh analysis for every single pair using the current global settings.
    pub(crate) fn trigger_global_recalc(&mut self, priority_pair: Option<String>) {
        self.queue.clear();

        let mut all_pairs = self.active_engine_pairs.clone();
        #[cfg(debug_assertions)]
        if DF.log_pairs {
            log::info!(
                "calling trigger_global_recalc() with the following {} pairs: {:?}",
                all_pairs.len(),
                all_pairs
            );
        }

        // Snapshot: Takes a snapshot of self.engine_strategy so the loop uses a consistent strategy
        let strategy = self.shared_config.get_strategy();

        // We can't use a closure easily due to borrow checker rules with self,
        // so we just iterate logic directly.
        let push_pair =
            |pair: String, target_queue: &mut VecDeque<_>, config: &SharedConfiguration| {
                // 1. Lookup PH (Specific to pair)
                let ph_pct = config
                    .get_ph(&pair)
                    .expect("We must have value for ph_pct for this pair at all times");

                // 2. Lookup Station (Specific to pair)
                let station = config.get_station(&pair).expect(&format!(
                    "trigger_global_recalc must have station set for pair {}",
                    pair
                ));

                target_queue.push_back(EngineJob {
                    pair,
                    price_override: None, // Live Price
                    ph_pct,
                    strategy,            // Respects User Choice
                    station_id: station, // Respects User Choice
                    mode: JobMode::FullAnalysis,
                });
            };

        if let Some(vip) = priority_pair {
            if let Some(pos) = all_pairs.iter().position(|p| p == &vip) {
                all_pairs.remove(pos);
            }
            push_pair(vip, &mut self.queue, &self.shared_config);
        }

        for pair in all_pairs {
            push_pair(pair, &mut self.queue, &self.shared_config);
        }
    }

    /// Forces a recalculation for a single pair.
    ///
    /// Semantics:
    /// - Applies to ONE pair only.
    /// - Does NOT clear or reorder the global queue.
    /// - Any existing queued job for this pair is replaced with a fresh one.
    /// - Jobs for other pairs are never affected.
    /// - Execution still flows through the normal queue (no direct dispatch).
    ///
    /// This is used when the caller knows the current model for this pair
    /// is stale (e.g. user action, parameter change).
    pub(crate) fn invalidate_pair_and_recalc(
        &mut self,
        pair: &str,
        price_override: Option<Price>,
        ph_pct: PhPct,
        strategy: OptimizationStrategy,
        station_id: StationId,
        mode: JobMode,
        _reason: &str,
    ) {
        #[cfg(debug_assertions)]
        if DF.log_engine_core {
            log::info!(
                "ENGINE: invalidate_pair_and_recalc → scheduling fresh job for [{}]",
                pair
            );
        }

        self.enqueue_or_replace(EngineJob {
            pair: pair.to_string(),
            price_override,
            ph_pct,
            strategy,
            station_id,
            mode,
        });
    }

    fn process_live_data(&mut self) {
        // Process incoming live candles every 5 mins
        // 1. Check if we have data
        // We use a loop to drain the channel so we don't lag behind
        let mut updates = Vec::new();
        while let Ok(candle) = self.candle_rx.try_recv() {
            updates.push(candle);
        }

        if updates.is_empty() {
            return;
        }

        let ts_lock = self.timeseries.clone();

        // 2. Write Lock (on local Arc)
        if let Ok(mut ts_collection) = ts_lock.write() {
            for candle in updates {
                if let Some(series) = ts_collection
                    .series_data
                    .iter_mut()
                    .find(|s| s.pair_interval.name() == candle.symbol)
                {
                    series.update_from_live(&candle);

                    if candle.is_closed {
                        #[cfg(debug_assertions)]
                        if DF.log_candle_update {
                            log::info!(
                                "ENGINE: Candle Closed for {}. Triggering Recalc. in process_live_data()",
                                candle.symbol
                            );
                        }

                        // 3. Trigger Recalc
                        let pair_name = candle.symbol;
                        if let Some(ph_pct) = self.shared_config.get_ph(&pair_name) {
                            let station_id = self
                                .shared_config
                                .get_station(&pair_name)
                                .expect(&format!("PAIR {} with ph_pct {} UNEXPECTEDLY not found in shared_config {:?}", pair_name, ph_pct, self.shared_config)); // This should now crash if None is encountered. Better than reverting to default I think
                            #[cfg(debug_assertions)]
                            if DF.log_engine_core {
                                log::info!(
                                    "Enqueing job for {} because a candle has closed:",
                                    pair_name
                                )
                            }
                            self.enqueue_or_replace(EngineJob {
                                pair: pair_name.clone(),
                                price_override: Some(candle.close.into()),
                                ph_pct: ph_pct,
                                strategy: self.shared_config.get_strategy(),
                                station_id: station_id,
                                mode: JobMode::FullAnalysis,
                            });
                            // }
                        } else {
                            #[cfg(debug_assertions)]
                            if DF.log_ph_overrides {
                                log::info!(
                                    "FAILED to READ ph_pct value from shared_config for pair {}. Therefore not updating this pair",
                                    pair_name
                                );
                            }
                        }
                    }
                }
            }
        }
        // THE REAPER: Garbage Collect dead trades. CRITICAL: We run this on EVERY tick (not just close).
        self.prune_ledger();
    }

    fn prune_ledger(&mut self) {
        // GARBAGE COLLECTION: Removes finished trades and archives them.
        // If a wick hits our Stop Loss mid-candle, we want to kill the trade immediately.
        let time_now_utc = TimeUtils::now_utc();
        let mut dead_trades: Vec<TradeResult> = Vec::new();
        let mut ids_to_remove: Vec<String> = Vec::new();

        // Access TimeSeries mainly for High/Low checks on the latest candle
        let ts_guard = self.timeseries.read().unwrap();

        // 1. Scan Ledger
        for (id, op) in &self.engine_ledger.opportunities {
            // A. Get Data context
            let pair = &op.pair_name;
            let interval_ms = BASE_INTERVAL.as_millis() as i64;

            if let Ok(series) = find_matching_ohlcv(&ts_guard.series_data, pair, interval_ms) {
                let Some(current_price) = series.close_prices.last().copied() else {
                    continue;
                };
                let Some(current_high) = series.high_prices.last().copied() else {
                    continue;
                };
                let Some(current_low) = series.low_prices.last().copied() else {
                    continue;
                };

                let outcome = op.check_exit_condition(
                    Price::from(current_high),
                    Price::from(current_low),
                    time_now_utc,
                );
                let mut exit_price = Price::new(0.0);

                if let Some(ref reason) = outcome {
                    exit_price = match reason {
                        TradeOutcome::TargetHit => Price::from(op.target_price),
                        TradeOutcome::StopHit => Price::from(op.stop_price),
                        TradeOutcome::Timeout => Price::from(current_price),
                        TradeOutcome::ManualClose => Price::from(current_price),
                    };
                }

                // D. Process Death
                if let Some(reason) = outcome {
                    let pnl = match op.direction {
                        TradeDirection::Long => {
                            (exit_price - Price::from(op.start_price)) / op.start_price * 100.0
                        }
                        TradeDirection::Short => {
                            (op.start_price - exit_price) / op.start_price * 100.0
                        }
                    };

                    #[cfg(debug_assertions)]
                    if DF.log_ledger {
                        log::info!(
                            "LEDGER: THE REAPER: Pruning {} [{}] -> {}. PnL: {:.2}%",
                            pair,
                            id,
                            reason,
                            pnl
                        );
                    }

                    let result = TradeResult {
                        trade_id: id.clone(),
                        pair: pair.clone(),
                        direction: op.direction.clone(),
                        entry_price: op.start_price,
                        exit_price: exit_price,
                        outcome: reason,
                        entry_time: op.created_at.timestamp_millis(),
                        exit_time: time_now_utc.timestamp_millis(),
                        final_pnl_pct: pnl,
                        model_snapshot: None,
                    };

                    dead_trades.push(result);
                    ids_to_remove.push(id.clone());
                }
            }
        }

        // Drop lock before async operations (though we just fire and forget mostly)
        drop(ts_guard);

        // Archive Dead Trades
        if !dead_trades.is_empty() {
            // NATIVE: Spawn async task to save
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Only clone the repo if we are actually going to use it
                let repo = self.results_repo.clone();
                let trades = dead_trades.clone();

                thread::spawn(move || {
                    let rt = Runtime::new().unwrap();
                    rt.block_on(async {
                        for trade in trades {
                            if let Err(e) = repo.record_trade(trade).await {
                                if DF.log_results_repo {
                                    log::error!("Failed to archive trade: {}", e);
                                }
                            }
                        }
                    });
                });
            }
        }

        // 3. Update Ledger
        for id in ids_to_remove {
            self.engine_ledger.remove(&id);
        }
    }

    fn handle_job_result(&mut self, result: JobResult) {
        if let Some(state) = self.pairs_states.get_mut(&result.pair_name) {
            match result.result {
                Ok(model) => {
                    // Sync to Ledger ---
                    // let fuzzy_tolerance = ;
                    for op in &model.opportunities {
                        self.engine_ledger.evolve(
                            op.clone(),
                            DEFAULT_JOURNEY_SETTINGS.optimization.fuzzy_match_tolerance,
                        );
                    }
                    // Success: Update State
                    state.model = Some(model.clone());
                    #[cfg(debug_assertions)]
                    if DF.log_engine_core {
                        log::info!(
                            "ENGINE STATE: [{}] is_calculating = false (job complete) in OK branch of handle_job_result()",
                            result.pair_name
                        );
                    }
                    state.is_calculating = false;
                    state.last_error = None;
                }
                Err(e) => {
                    // Failure: Clear Model, Set Error
                    log::error!("Worker failed for {}: {}", result.pair_name, e);
                    state.last_error = Some(e);
                    #[cfg(debug_assertions)]
                    if DF.log_engine_core {
                        log::info!(
                            "ENGINE STATE: [{}] is_calculating = false (job complete) (in Err branch of handle_job_result()",
                            result.pair_name
                        );
                    }
                    state.is_calculating = false;

                    // Critical: Clear old model so UI shows error screen, not ghost data
                    state.model = None;
                }
            }
        }
    }

    #[cfg(debug_assertions)]
    fn _log_queue(&self, context: &str) {
        if !DF.log_engine_core {
            return;
        }

        let pairs: Vec<&str> = self.queue.iter().map(|j| j.pair.as_str()).collect();

        log::info!(
            "ENGINE QUEUE STATUS [{}]: len={} {:?}",
            context,
            pairs.len(),
            pairs
        );
    }

    fn trigger_recalcs_on_price_changes(&mut self) {
        // let pairs: Vec<String> = self.shared_config.get_all_pairs();
        let threshold = PRICE_RECALC_THRESHOLD_PCT;

        let pairs: Vec<String> = self.active_engine_pairs.iter().cloned().collect();
        #[cfg(debug_assertions)]
        if DF.log_pairs {
            log::info!(
                "calling trigger_recalcs_on_price_changes() with the following {} pairs: {:?}",
                pairs.len(),
                pairs
            );
        }
        for pair_name in pairs {
            let Some(current_price) = self.price_stream.get_price(&pair_name) else {
                continue;
            };

            // ---- read-only phase (NO mutable borrows) ----
            let should_trigger = {
                let Some(state) = self.pairs_states.get_mut(&pair_name) else {
                    continue;
                };

                if state.last_update_price.value() == 0.0 {
                    #[cfg(debug_assertions)]
                    if DF.log_engine_core {
                        log::info!(
                            "ENGINE PRICE BOOTSTRAP: [{}] initializing last_update_price = {} in trigger_recalcs_on_price_change()",
                            pair_name,
                            current_price,
                        );
                    }
                    state.last_update_price = current_price;
                    continue;
                } else {
                    let pct_diff =
                        PhPct::new(current_price.percent_diff_from_0_1(&state.last_update_price));
                    let triggered = pct_diff > threshold;

                    #[cfg(debug_assertions)]
                    if triggered && DF.log_engine_core {
                        log::info!(
                            "ENGINE AUTO (PRICE TRIGGER): [{}] last={} current={} diff={} threshold={}",
                            pair_name,
                            state.last_update_price,
                            current_price,
                            pct_diff,
                            threshold,
                        );
                    }

                    triggered
                }
            };

            if !should_trigger {
                // even though we aren't triggering, we still need to bootstrap last_update_price
                if let Some(state) = self.pairs_states.get_mut(&pair_name) {
                    if state.last_update_price.value() == 0.0 {
                        state.last_update_price = current_price;
                    }
                }
                continue;
            }

            // We are doing a new job for this pair so get its details out.........
            let ph_pct = match self.shared_config.get_ph(&pair_name) {
                Some(v) => v,
                None => {
                    #[cfg(debug_assertions)]
                    if DF.log_engine_core {
                        log::error!(
                            "Was intending to enqueue a job for {} but ph value not available",
                            { pair_name }
                        );
                    };
                    continue;
                }
            };

            let station_id = self.shared_config.get_station(&pair_name).expect(&format!(
                "PAIR {} with ph_pct {} unexpectedly missing station_id",
                pair_name, ph_pct
            ));

            self.enqueue_or_replace(EngineJob {
                pair: pair_name.clone(),
                price_override: None,
                ph_pct,
                strategy: self.shared_config.get_strategy(),
                station_id,
                mode: JobMode::FullAnalysis,
            });

            // ---- mutation phase ----
            if let Some(state) = self.pairs_states.get_mut(&pair_name) {
                state.last_update_price = current_price;
            }
        }
    }

    fn process_queue(&mut self) {
        if self.queue.is_empty() {
            return;
        }

        // Pop and dispatch
        if let Some(job) = self.queue.pop_front() {
            #[cfg(debug_assertions)]
            if DF.log_engine_core {
                log::info!("ENGINE QUEUE: dispatching job for [{}]", job.pair);
            }

            self.dispatch_job(job);
        }
    }

    fn dispatch_job(&mut self, job: EngineJob) {
        if let Some(state) = self.pairs_states.get_mut(&job.pair) {
            // Resolve Price
            let live_price = self.price_stream.get_price(&job.pair).map(Price::from);
            let final_price_opt = job.price_override.or(live_price);

            // 🔴 THIS IS THE FIX for the engine violation - refuse to queue a new job for the same pair if already being calculated
            // Why do we do that? Because any job enqueued for a pair that is already calculating is guaranteed to be stale by the time it runs,
            // so it wastes work and risks overwriting newer state with older assumptions.

            if state.is_calculating {
                #[cfg(debug_assertions)]
                if DF.log_engine_core {
                    log::info!(
                        "ENGINE SKIP: [{}] already calculating, dropping dispatched job",
                        job.pair
                    );
                }
                return;
            }
            // 🔴 END FIX

            #[cfg(debug_assertions)]
            if DF.log_engine_core {
                log::info!(
                    "ENGINE STATE: [{}] is_calculating = true (dispatch)",
                    job.pair
                );
            }
            #[cfg(debug_assertions)]
            if DF.log_engine_core && state.is_calculating {
                log::error!(
                    "ENGINE VIOLATION: [{}] dispatched while already calculating - this should literally never happen",
                    job.pair
                );
            }

            state.is_calculating = true;

            if let Some(p) = final_price_opt {
                #[cfg(debug_assertions)]
                if DF.log_engine_core {
                    log::info!(
                        "ENGINE DISPATCH: [{}] committing last_update_price = {}",
                        job.pair,
                        p,
                    );
                }
                state.last_update_price = p;
            }

            // 4. Send Request
            let req = JobRequest {
                pair_name: job.pair,
                current_price: final_price_opt,
                // config: self.app_constants.clone(),
                timeseries: self.timeseries.clone(),
                // horizon_profile,
                ph_pct: job.ph_pct,
                strategy: job.strategy,
                station_id: job.station_id,
                mode: job.mode,
            };

            let _ = self.job_tx.send(req);
        }
    }

    fn enqueue_or_replace(&mut self, job: EngineJob) {
        // Remove any existing queued job for the same pair
        if let Some(pos) = self.queue.iter().position(|j| j.pair == job.pair) {
            #[cfg(debug_assertions)]
            if DF.log_engine_core {
                log::info!("ENGINE QUEUE: Replacing queued job for pair [{}]", job.pair);
            }
            self.queue.remove(pos);
        } else {
            #[cfg(debug_assertions)]
            if DF.log_engine_core {
                log::info!("ENGINE QUEUE: Enqueuing new job for pair [{}]", job.pair);
            }
        }

        self.queue.push_back(job);
    }
}
