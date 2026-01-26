use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, RwLock};

#[cfg(not(target_arch = "wasm32"))]
use {crate::config::PERSISTENCE, std::path::Path, std::thread, tokio::runtime::Runtime};

use crate::config::{OptimizationGoal, StationId, constants};
#[cfg(debug_assertions)]
use crate::config::DF;

use crate::data::price_stream::PriceStreamManager;
use crate::data::results_repo::{ResultsRepository, ResultsRepositoryTrait, TradeResult};
use crate::data::timeseries::TimeSeriesCollection;

use crate::models::ledger::OpportunityLedger;
use crate::models::timeseries::{LiveCandle, find_matching_ohlcv};
use crate::models::trading_view::{
    TradeDirection, TradeFinderRow, TradeOpportunity, TradeOutcome, TradingModel,
};

use crate::utils::TimeUtils;
use crate::utils::time_utils::AppInstant;

use super::messages::{JobMode, JobRequest, JobResult};
use super::state::PairState;
use super::worker;

pub struct SniperEngine {
    /// Registry of all pairs
    pub pairs: HashMap<String, PairState>,

    /// Shared immutable data
    pub timeseries: Arc<RwLock<TimeSeriesCollection>>,

    // Live Data Channels
    candle_rx: Receiver<LiveCandle>,
    pub candle_tx: Sender<LiveCandle>, // Public so App can grab it easily

    /// Live Data Feed
    pub price_stream: Arc<PriceStreamManager>,

    // Common Channels
    job_tx: Sender<JobRequest>,     // UI writes to this
    result_rx: Receiver<JobResult>, // UI reads from this

    // WASM ONLY: The Engine acts as the Worker, so it needs the "Worker Ends" of the channels
    #[cfg(target_arch = "wasm32")]
    job_rx: Receiver<JobRequest>,
    #[cfg(target_arch = "wasm32")]
    result_tx: Sender<JobResult>,

    /// Queue Logic: (Pair, Price, PH, Strategy, StationId, Mode)
    // Update the VecDeque type to hold the runtime variables
    pub queue: VecDeque<(
        String,
        Option<f64>,
        f64,
        OptimizationGoal,
        StationId,
        JobMode,
    )>,
    /// The Live Configuration State
    pub price_horizon_overrides: HashMap<String, f64>,
    pub current_strategy: OptimizationGoal,
    pub station_overrides: HashMap<String, StationId>,
    pub ledger: OpportunityLedger,
    pub results_repo: Arc<dyn ResultsRepositoryTrait>,

    // Maintenance Timer (Runs in Release & Debug)
    pub last_ledger_maintenance: AppInstant,
}

impl SniperEngine {
    /// Initialize the engine, spawn workers, and start the price stream.
    pub fn new(timeseries: TimeSeriesCollection) -> Self {
        // 1. Create Channels
        let (candle_tx, candle_rx) = channel();
        let (job_tx, job_rx) = channel::<JobRequest>();
        let (result_tx, result_rx) = channel::<JobResult>();

        // 2. Create the Thread-Safe Data Structure ONCE
        // Wrap the collection in RwLock (for writing) and Arc (for sharing)
        let timeseries_arc = Arc::new(RwLock::new(timeseries));

        // NATIVE: Pass the receiver to the thread.
        #[cfg(not(target_arch = "wasm32"))]
        worker::spawn_worker_thread(job_rx, result_tx);

        // 3. Initialize Pairs
        // We must Read-Lock the data temporarily to get the names
        let mut pairs = HashMap::new();
        {
            let ts_guard = timeseries_arc.read().unwrap();
            for pair in ts_guard.unique_pair_names() {
                pairs.insert(pair, PairState::new());
            }
        } // Lock is dropped here

        // 4. Initialize Price Stream
        // "If compiling for WASM, ignore the fact that this is mutable but not mutated."
        #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
        let mut price_manager = PriceStreamManager::new();

        #[cfg(not(target_arch = "wasm32"))]
        price_manager.set_candle_sender(candle_tx.clone());

        let price_stream = Arc::new(price_manager);
        let all_names: Vec<String> = pairs.keys().cloned().collect();
        price_stream.subscribe_all(all_names);

        // 5. Initialize Results Repository
        // Construct path: "data_dir/results.db"
        #[cfg(not(target_arch = "wasm32"))]
        let db_path = Path::new(PERSISTENCE.kline.directory)
            .parent()
            .unwrap_or(Path::new("."))
            .join("results.sqlite");

        #[cfg(not(target_arch = "wasm32"))]
        let db_path_str = db_path.to_str().unwrap_or("results.sqlite");

        // We use block_on in native to init the DB async
        #[cfg(not(target_arch = "wasm32"))]
        let repo = {
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

        #[cfg(target_arch = "wasm32")]
        let repo = ResultsRepository::new("").unwrap();
        // 5. Construct Engine
        Self {
            pairs,
            timeseries: timeseries_arc, // Pass the Arc<RwLock> we created
            price_stream,
            candle_rx,
            candle_tx,
            job_tx,
            result_rx,
            // WASM: Store the handles so they don't get dropped
            #[cfg(target_arch = "wasm32")]
            job_rx,
            #[cfg(target_arch = "wasm32")]
            result_tx,
            queue: VecDeque::new(),
            price_horizon_overrides: HashMap::new(),
            ledger: OpportunityLedger::new(),
            results_repo: Arc::new(repo),
            last_ledger_maintenance: AppInstant::now(),
            current_strategy: OptimizationGoal::default(),
            station_overrides: HashMap::new(),
        }
    }

    // INTENT: The User clicked a Pair or Station.
    // pub fn request_market_scan(&mut self, pair: String) {
    //     // Just trigger the work.
    //     self.force_recalc(&pair, None, JobMode::FullAnalysis, "MARKET SCAN");
    // }

    /// INTENT: The User clicked a specific Trade Opportunity.
    /// Action: Calculate CVA Zones for this specific PH so the chart looks right,
    /// but DO NOT re-run simulations (preserve the trade list).
    pub fn request_trade_context(&mut self, pair: String, target_ph: f64) {
        // 2. Set Override (so the worker picks it up)
        self.set_price_horizon_override(pair.clone(), target_ph);

        // 3. Dispatch Context-Only Job
        // Lee's only code to grab the station id out of station_overrides lol
        if let Some(station_id) = self.station_overrides.get(&pair).copied() {
            self.force_recalc(
                &pair,
                None,
                target_ph,
                self.current_strategy,
                station_id,
                JobMode::ContextOnly,
                "TRADE CONTEXT",
            );
        }
    }

    /// Process incoming live candles
    pub fn process_live_data(&mut self) {
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
                        if DF.log_engine {
                            log::info!(
                                "ENGINE: Candle Closed for {}. Triggering Recalc.",
                                candle.symbol
                            );
                        }

                        // 3. Trigger Recalc
                        let pair_name = candle.symbol;
                        // This is now allowed because 'ts_collection' borrows 'ts_lock', not 'self'.
                        if let Some(ph_pct) = self.price_horizon_overrides.get(&pair_name) {
                            if let Some(station_id) = self.station_overrides.get(&pair_name) {
                                self.force_recalc(
                                    &pair_name,
                                    Some(candle.close),
                                    *ph_pct,
                                    self.current_strategy,
                                    *station_id,
                                    JobMode::FullAnalysis,
                                    "CANDLE CLOSE TRIGGER",
                                );
                            }
                        }
                    }
                }
            }
        }
        // THE REAPER: Garbage Collect dead trades
        // CRITICAL: We run this on EVERY tick (not just close).
        // If a wick hits our Stop Loss mid-candle, we want to kill the trade immediately.
        self.prune_ledger();
    }

    /// GARBAGE COLLECTION: Removes finished trades and archives them.
    pub fn prune_ledger(&mut self) {
        let now_ms = TimeUtils::now_timestamp_ms();
        let mut dead_trades: Vec<TradeResult> = Vec::new();
        let mut ids_to_remove: Vec<String> = Vec::new();

        // Access TimeSeries mainly for High/Low checks on the latest candle
        let ts_guard = self.timeseries.read().unwrap();

        // 1. Scan Ledger
        for (id, op) in &self.ledger.opportunities {
            // A. Get Data context
            let pair = &op.pair_name;
            let interval_ms = constants::INTERVAL_WIDTH_MS;

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

                let outcome = op.check_exit_condition(current_high, current_low, now_ms);
                let mut exit_price = 0.0;

                if let Some(ref reason) = outcome {
                    exit_price = match reason {
                        TradeOutcome::TargetHit => op.target_price,
                        TradeOutcome::StopHit => op.stop_price,
                        TradeOutcome::Timeout => current_price,
                        TradeOutcome::ManualClose => current_price,
                    };
                }

                // D. Process Death
                if let Some(reason) = outcome {
                    let pnl = match op.direction {
                        TradeDirection::Long => {
                            (exit_price - op.start_price) / op.start_price * 100.0
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
                        exit_price,
                        outcome: reason,
                        entry_time: op.created_at,
                        exit_time: now_ms,
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
                                log::error!("Failed to archive trade: {}", e);
                            }
                        }
                    });
                });
            }
        }

        // 3. Update Ledger
        for id in ids_to_remove {
            self.ledger.remove(&id);
        }
    }

    /// Generates the master list for the Trade Finder.
    pub fn get_trade_finder_rows(
        &self,
        overrides: Option<&HashMap<String, f64>>,
    ) -> Vec<TradeFinderRow> {
        crate::trace_time!("Core: Get TradeFinder Rows", 2000, {
            let mut rows = Vec::new();

            let now_ms = TimeUtils::now_timestamp_ms();
            let day_ms = 86_400_000;

            // 1. Group Ledger Opportunities by Pair for fast lookup
            let mut ops_by_pair: HashMap<String, Vec<&TradeOpportunity>> = HashMap::new();
            for op in self.ledger.get_all() {
                ops_by_pair
                    .entry(op.pair_name.clone())
                    .or_default()
                    .push(op);
            }

            let ts_guard = self.timeseries.read().unwrap();

            for (pair, _state) in &self.pairs {
                // 2. Get Context (Price)
                // STRICT MODE: Do not default to 0.0. If no price, skip the pair.
                let price_opt = if let Some(map) = overrides {
                    map.get(pair)
                        .copied()
                        .or_else(|| self.price_stream.get_price(pair))
                } else {
                    self.price_stream.get_price(pair)
                };

                let current_price = match price_opt {
                    Some(p) if p > f64::EPSILON => p,
                    _ => continue, // Skip this pair completely until we have data
                };

                // 3. Calculate Volume & Market State (From TimeSeries)
                // We do this for every pair regardless of whether it has ops
                let mut vol_24h = 0.0;

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
                    .filter(|&&op| op.expected_roi() > 0.0)
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
                            opportunity_count_total: total_ops,
                            opportunity: Some(op.clone()),
                            current_price,
                        });
                    }
                } else {
                    // No valid trades, push 1 placeholder row (for "All Pairs" view)
                    rows.push(TradeFinderRow {
                        pair_name: pair.clone(),
                        quote_volume_24h: vol_24h,
                        market_state: None,
                        opportunity_count_total: 0,
                        opportunity: None,
                        current_price,
                    });
                }
            }
            rows
        })
    }

    // Helper functions to ensure the UI can update the Engine's status
    pub fn set_price_horizon_override(&mut self, pair: String, ph_pct: f64) {
        self.price_horizon_overrides.insert(pair, ph_pct);
    }

    pub fn set_strategy(&mut self, strategy: OptimizationGoal) {
        self.current_strategy = strategy;
    }

    pub fn set_station_override(&mut self, pair: String, station: StationId) {
        self.station_overrides.insert(pair, station);
    }

    /// THE GAME LOOP.
    pub fn update(&mut self) {
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
        let journey_settings = &constants::journey::DEFAULT;

        if t1.duration_since(self.last_ledger_maintenance).as_secs()
            >= journey_settings.optimization.prune_interval_sec
        {
            // Pass the Tolerance AND the Profile (Strategy)
            self.ledger
                .prune_collisions(journey_settings.optimization.fuzzy_match_tolerance);
            self.last_ledger_maintenance = t1;
        }

        // Results
        let t2 = AppInstant::now();
        while let Ok(result) = self.result_rx.try_recv() {
            self.handle_job_result(result);
        }
        let d2 = t2.elapsed().as_micros();

        // Triggers
        let t3 = AppInstant::now();
        self.check_automatic_triggers();
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
    pub fn get_model(&self, pair: &str) -> Option<Arc<TradingModel>> {
        self.pairs.get(pair).and_then(|state| state.model.clone())
    }

    pub fn get_price(&self, pair: &str) -> Option<f64> {
        self.price_stream.get_price(pair)
    }

    pub fn set_stream_suspended(&self, suspended: bool) {
        if suspended {
            self.price_stream.suspend();
        } else {
            self.price_stream.resume();
        }
    }

    pub fn get_all_pair_names(&self) -> Vec<String> {
        self.timeseries.read().unwrap().unique_pair_names()
    }

    pub fn get_queue_len(&self) -> usize {
        self.queue.len()
    }

    pub fn get_worker_status_msg(&self) -> Option<String> {
        let calculating_pair = self
            .pairs
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

    pub fn get_active_pair_count(&self) -> usize {
        self.pairs.len()
    }

    pub fn get_pair_status(&self, pair: &str) -> (bool, Option<String>) {
        if let Some(state) = self.pairs.get(pair) {
            (state.is_calculating, state.last_error.clone())
        } else {
            (false, None)
        }
    }

    pub fn trigger_global_recalc(&mut self, priority_pair: Option<String>) {
        self.queue.clear();

        let mut all_pairs = self.get_all_pair_names();

        // Helper to push with CORRECT context
        // We capture 'self' features (maps/strategy) to use inside the loop
        let strat = self.current_strategy;

        // We can't use a closure easily due to borrow checker rules with self,
        // so we just iterate logic directly.
        let push_pair = |pair: String,
                         target_queue: &mut VecDeque<_>,
                         ph_map: &HashMap<String, f64>,
                         st_map: &HashMap<String, StationId>| {
            // 1. Lookup PH (Specific to pair)
            let ph = *ph_map.get(&pair).unwrap_or(&0.15);

            // 2. Lookup Station (Specific to pair)
            let station = *st_map.get(&pair).unwrap_or(&StationId::default());

            target_queue.push_back((
                pair,
                None, // Live Price
                ph,
                strat,   // Respects User Choice
                station, // Respects User Choice
                JobMode::FullAnalysis,
            ));
        };

        if let Some(vip) = priority_pair {
            if let Some(pos) = all_pairs.iter().position(|p| p == &vip) {
                all_pairs.remove(pos);
            }
            push_pair(
                vip,
                &mut self.queue,
                &self.price_horizon_overrides,
                &self.station_overrides,
            );
        }

        for pair in all_pairs {
            push_pair(
                pair,
                &mut self.queue,
                &self.price_horizon_overrides,
                &self.station_overrides,
            );
        }
    }

    /// Force a single recalc with optional price override
    pub fn force_recalc(
        &mut self,
        pair: &str,
        price_override: Option<f64>,
        ph_pct: f64,
        strategy: OptimizationGoal,
        station_id: StationId,
        mode: JobMode,
        _reason: &str,
    ) {
        // Check if calculating
        let is_calculating = self
            .pairs
            .get(pair)
            .map(|s| s.is_calculating)
            .unwrap_or(false);

        // Check if already in queue (by Name)
        // We use a manual iterator check because contains() fails on tuples
        let in_queue = self.queue.iter().any(|(p, _, _, _, _, _)| p == pair);

        if !is_calculating && !in_queue {
            self.queue.push_front((
                pair.to_string(),
                price_override,
                ph_pct,
                strategy,
                station_id,
                mode,
            ));
        }
    }

    fn handle_job_result(&mut self, result: JobResult) {
        if let Some(state) = self.pairs.get_mut(&result.pair_name) {
            // 1. Always update profile if present (Success OR Failure)
            // if let Some(p) = result.profile {
            //     state.profile = Some(p);
            // }

            // 2. Always update the authoritative candle count
            state.last_candle_count = result.candle_count;

            match result.result {
                Ok(model) => {
                    // Sync to Ledger ---
                    // let fuzzy_tolerance = ;
                    for op in &model.opportunities {
                        self.ledger.evolve(
                            op.clone(),
                            constants::journey::optimization::FUZZY_MATCH_TOLERANCE,
                        );
                    }

                    // Success: Update State
                    state.model = Some(model.clone());

                    // Success: Update State
                    state.model = Some(model.clone());
                    state.is_calculating = false;
                    state.last_update_time = AppInstant::now();
                    state.last_error = None;
                }
                Err(e) => {
                    // Failure: Clear Model, Set Error
                    log::error!("Worker failed for {}: {}", result.pair_name, e);
                    state.last_error = Some(e);
                    state.is_calculating = false;

                    // Critical: Clear old model so UI shows error screen, not ghost data
                    state.model = None;
                }
            }
        }
    }

    // Add Accessor
    pub fn get_candle_count(&self, pair: &str) -> usize {
        self.pairs
            .get(pair)
            .map(|s| s.last_candle_count)
            .unwrap_or(0)
    }

    fn check_automatic_triggers(&mut self) {
        let pairs: Vec<String> = self.pairs.keys().cloned().collect();
        let threshold = constants::cva::PRICE_RECALC_THRESHOLD_PCT;

        for pair in pairs {
            if let Some(current_price) = self.price_stream.get_price(&pair) {
                if let Some(state) = self.pairs.get_mut(&pair) {
                    let in_queue = self.queue.iter().any(|(p, _, _, _, _, _)| p == &pair);

                    if !state.is_calculating && !in_queue {
                        // FIX: Handle Startup Case (0.0)
                        // If we have no previous price, just sync state and DO NOT trigger.
                        // The startup job (trigger_global_recalc) is already handling the calc.
                        if state.last_update_price.abs() < f64::EPSILON {
                            state.last_update_price = current_price;
                            continue;
                        }

                        let diff = (current_price - state.last_update_price).abs();
                        let pct_diff = diff / state.last_update_price;

                        if pct_diff > threshold {
                            #[cfg(debug_assertions)]
                            if DF.log_engine {
                                log::info!(
                                    "ENGINE AUTO: [{}] moved {:.4}% with threshold {}. Triggering Recalc.",
                                    pair,
                                    pct_diff * 100.0,
                                    threshold,
                                );
                            }

                            if let Some(ph_pct) = self.price_horizon_overrides.get(&pair) {
                                if let Some(station_id) = self.station_overrides.get(&pair) {
                                    self.dispatch_job(
                                        pair.clone(),
                                        None,
                                        *ph_pct,
                                        self.current_strategy,
                                        *station_id,
                                        JobMode::FullAnalysis,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn process_queue(&mut self) {
        if self.queue.is_empty() {
            return;
        }

        // Peek at front (tuple index 0 is pair name)
        if let Some((pair, _, _, _, _, _)) = self.queue.front() {
            if let Some(state) = self.pairs.get(pair) {
                if state.is_calculating {
                    return; // Busy
                }
            }
        }

        if let Some((pair, price_opt, ph_pct, strategy, station_id, mode)) = self.queue.pop_front()
        {
            self.dispatch_job(pair, price_opt, ph_pct, strategy, station_id, mode);
        }
    }

    fn dispatch_job(
        &mut self,
        pair: String,
        price_override: Option<f64>,
        ph_pct: f64,
        strategy: OptimizationGoal,
        station_id: StationId,
        mode: JobMode,
    ) {
        if let Some(state) = self.pairs.get_mut(&pair) {
            // 1. Resolve Price
            // Priority: Override -> Live Stream -> None (Worker will fetch from DB)
            let live_price = self.price_stream.get_price(&pair);
            let final_price_opt = price_override.or(live_price);

            // Update State Metadata
            state.is_calculating = true;
            if let Some(p) = final_price_opt {
                state.last_update_price = p;
            }

            // 2. Capture Cache (Smart Profiling Optimization)
            let existing_profile = state.profile.clone();

            // 4. Send Request
            let req = JobRequest {
                pair_name: pair,
                current_price: final_price_opt,
                // config: self.app_constants.clone(),
                timeseries: self.timeseries.clone(),
                existing_profile,
                ph_pct,
                strategy,
                station_id,
                mode: mode,
            };

            let _ = self.job_tx.send(req);
        }
    }
}
