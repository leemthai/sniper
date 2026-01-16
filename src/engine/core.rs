use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, RwLock};

#[cfg(not(target_arch = "wasm32"))]
use {crate::config::PERSISTENCE, std::path::Path, std::thread, tokio::runtime::Runtime};

use crate::analysis::adaptive::AdaptiveParameters;
use crate::analysis::market_state::MarketState;

use crate::config::{ANALYSIS, AnalysisConfig, PriceHorizonConfig};

use crate::data::price_stream::PriceStreamManager;
use crate::data::results_repo::{ResultsRepository, ResultsRepositoryTrait, TradeResult};
use crate::data::timeseries::TimeSeriesCollection;

use crate::models::horizon_profile::HorizonProfile;
use crate::models::ledger::OpportunityLedger;
use crate::models::timeseries::{LiveCandle, find_matching_ohlcv};
use crate::models::trading_view::{
    LiveOpportunity, TradeDirection, TradeFinderRow, TradeOpportunity, TradeOutcome, TradingModel,
};

use crate::utils::TimeUtils;
use crate::utils::maths_utils::calculate_percent_diff;
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

    /// Queue Logic: (PairName, OptionalPriceOverride)
    pub queue: VecDeque<(String, Option<f64>)>,
    /// The Live Configuration State
    pub current_config: AnalysisConfig,
    pub config_overrides: HashMap<String, PriceHorizonConfig>,
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
            current_config: ANALYSIS.clone(),
            config_overrides: HashMap::new(),
            ledger: OpportunityLedger::new(),
            results_repo: Arc::new(repo),
            last_ledger_maintenance: AppInstant::now(),
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
                        log::info!(
                            "ENGINE: Candle Closed for {}. Triggering Recalc.",
                            candle.symbol
                        );

                        // 3. Trigger Recalc
                        // This is now allowed because 'ts_collection' borrows 'ts_lock', not 'self'.
                        self.force_recalc(
                            &candle.symbol,
                            Some(candle.close),
                            "CANDLE CLOSE TRIGGER",
                        );
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
            let interval_ms = self.current_config.interval_width_ms;

            if let Ok(series) = find_matching_ohlcv(&ts_guard.series_data, pair, interval_ms) {
                let current_price = series.close_prices.last().copied().unwrap_or(0.0);
                let current_high = series.high_prices.last().copied().unwrap_or(0.0);
                let current_low = series.low_prices.last().copied().unwrap_or(0.0);
                if current_price <= f64::EPSILON {
                    continue;
                }

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
                    log::info!(
                        "THE REAPER: Pruning {} [{}] -> {}. PnL: {:.2}%",
                        pair,
                        id,
                        reason,
                        pnl
                    );

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

            let ph_pct = self.current_config.price_horizon.threshold_pct;
            let lookback = AdaptiveParameters::calculate_trend_lookback_candles(ph_pct);
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
                let mut m_state = None;
                let mut vol_24h = 0.0;

                if let Some(ts) = ts_guard
                    .series_data
                    .iter()
                    .find(|t| t.pair_interval.name() == pair)
                {
                    let count = ts.klines();
                    if count > 0 {
                        let current_idx = count - 1;
                        m_state = MarketState::calculate(ts, current_idx, lookback);
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
                        #[cfg(debug_assertions)]
                        if pair == "PAXGUSDT" {
                            let live_val = op.live_roi(current_price);
                            let static_val = op.expected_roi();
                            if (live_val - static_val).abs() > 1. {
                                log::warn!(
                                    "🕵️ ROI MISMATCH AUDIT [{}]: Static: {:.2}% | Live: {:.2}% | Diff: {:.2}%",
                                    op.id,
                                    static_val,
                                    live_val,
                                    static_val - live_val
                                );
                            }
                        }
                        let live_opp = LiveOpportunity {
                            opportunity: op.clone(),
                            current_price,
                            live_roi: op.live_roi(current_price),
                            annualized_roi: op.live_annualized_roi(current_price),
                            risk_pct: calculate_percent_diff(op.stop_price, current_price),
                            reward_pct: calculate_percent_diff(op.target_price, current_price),
                            max_duration_ms: op.max_duration_ms,
                        };

                        rows.push(TradeFinderRow {
                            pair_name: pair.clone(),
                            quote_volume_24h: vol_24h,
                            market_state: m_state, // Copy state
                            opportunity_count_total: total_ops,
                            opportunity: Some(live_opp),
                        });
                    }
                } else {
                    // No valid trades, push 1 placeholder row (for "All Pairs" view)
                    rows.push(TradeFinderRow {
                        pair_name: pair.clone(),
                        quote_volume_24h: vol_24h,
                        market_state: m_state,
                        opportunity_count_total: 0,
                        opportunity: None,
                    });
                }
            }
            rows
        })
    }

    /// Aggregates opportunities.
    /// overrides: If provided, uses these prices instead of live stream (For Simulation).
    pub fn get_all_live_opportunities(
        &self,
        overrides: Option<&HashMap<String, f64>>,
    ) -> Vec<LiveOpportunity> {
        let mut results = Vec::new();

        for (pair, state) in &self.pairs {
            let model_opt = &state.model;

            // PRIORITY: Check Override -> Then check Stream
            let current_price_opt = if let Some(map) = overrides {
                map.get(pair)
                    .copied()
                    .or_else(|| self.price_stream.get_price(pair))
            } else {
                self.price_stream.get_price(pair)
            };

            if let (Some(model), Some(price)) = (model_opt, current_price_opt) {
                for opp in &model.opportunities {
                    // ... (Calculate Live Stats using 'price') ...
                    let live_roi = opp.live_roi(price);
                    let annualized_roi = opp.live_annualized_roi(price);

                    let risk_pct = calculate_percent_diff(opp.stop_price, price);
                    let reward_pct = calculate_percent_diff(opp.target_price, price);

                    results.push(LiveOpportunity {
                        opportunity: opp.clone(),
                        current_price: price,
                        live_roi,
                        annualized_roi,
                        risk_pct,
                        reward_pct,
                        max_duration_ms: opp.max_duration_ms,
                    });
                }
            }
        }

        // Sort by ROI descending (Standard)
        results.sort_by(|a, b| {
            b.live_roi
                .partial_cmp(&a.live_roi)
                .unwrap_or(Ordering::Equal)
        });

        results
    }

    // NEW: Helper to set an override
    pub fn set_price_horizon_override(&mut self, pair: String, config: PriceHorizonConfig) {
        self.config_overrides.insert(pair, config);
    }

    // NEW: Helper to bulk update (for startup sync)
    pub fn set_all_overrides(&mut self, overrides: HashMap<String, PriceHorizonConfig>) {
        self.config_overrides = overrides;
    }

    /// THE GAME LOOP.
    pub fn update(&mut self, _protected_id: Option<&str>) {
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
        let journey_settings = &self.current_config.journey; // Access journey settings
        
        if t1.duration_since(self.last_ledger_maintenance).as_secs() >= journey_settings.optimization.prune_interval_sec {
            // Pass the Tolerance AND the Profile (Strategy)
            self.ledger.prune_collisions(
                journey_settings.optimization.fuzzy_match_tolerance,
                &journey_settings.profile
            );
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

    pub fn update_config(&mut self, new_config: AnalysisConfig) {
        self.current_config = new_config;
    }

    /// Smart Global Invalidation
    pub fn trigger_global_recalc(&mut self, priority_pair: Option<String>) {
        self.queue.clear();

        let mut all_pairs = self.get_all_pair_names();

        if let Some(vip) = priority_pair {
            if let Some(pos) = all_pairs.iter().position(|p| p == &vip) {
                all_pairs.remove(pos);
            }
            // Use None for price override (Global recalc uses live price)
            self.queue.push_back((vip, None));
        }

        for pair in all_pairs {
            self.queue.push_back((pair, None));
        }
    }

    /// Force a single recalc with optional price override
    pub fn force_recalc(&mut self, pair: &str, price_override: Option<f64>, _reason: &str) {
        // Check if calculating
        let is_calculating = self
            .pairs
            .get(pair)
            .map(|s| s.is_calculating)
            .unwrap_or(false);

        // Check if already in queue (by Name)
        // We use a manual iterator check because contains() fails on tuples
        let in_queue = self.queue.iter().any(|(p, _)| p == pair);

        if !is_calculating && !in_queue {
            self.queue.push_front((pair.to_string(), price_override));
        }
    }

    // --- INTERNAL LOGIC ---

    fn handle_job_result(&mut self, result: JobResult) {
        if let Some(state) = self.pairs.get_mut(&result.pair_name) {
            // 1. Always update profile if present (Success OR Failure)
            if let Some(p) = result.profile {
                state.profile = Some(p);
            }

            // 2. Always update the authoritative candle count
            state.last_candle_count = result.candle_count;

            match result.result {
                Ok(model) => {
                    // Sync to Ledger ---
                    let fuzzy_tolerance = self
                        .current_config
                        .journey
                        .optimization
                        .fuzzy_match_tolerance;
                    for op in &model.opportunities {
                        self.ledger.evolve(op.clone(), fuzzy_tolerance);
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

    pub fn get_profile(&self, pair: &str) -> Option<HorizonProfile> {
        self.pairs.get(pair).and_then(|state| state.profile.clone())
    }

    fn check_automatic_triggers(&mut self) {
        let pairs: Vec<String> = self.pairs.keys().cloned().collect();
        // Use cva settings for threshold
        let threshold = self.current_config.cva.price_recalc_threshold_pct;

        for pair in pairs {
            if let Some(current_price) = self.price_stream.get_price(&pair) {
                if let Some(state) = self.pairs.get_mut(&pair) {
                    let in_queue = self.queue.iter().any(|(p, _)| p == &pair);

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
                            log::info!(
                                "ENGINE AUTO: [{}] moved {:.4}% with threshold {}. Triggering Recalc.",
                                pair,
                                pct_diff * 100.0,
                                threshold,
                            );

                            self.dispatch_job(pair.clone(), None, JobMode::Standard);
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

        // Peek at front
        if let Some((pair, _)) = self.queue.front() {
            // Race check: is it calculating now?
            if let Some(state) = self.pairs.get(pair) {
                if state.is_calculating {
                    // It's busy. Wait.
                    return;
                }
            }
        }

        if let Some((pair, price_opt)) = self.queue.pop_front() {
            self.dispatch_job(pair, price_opt, JobMode::Standard);
        }
    }

    fn dispatch_job(&mut self, pair: String, price_override: Option<f64>, mode: JobMode) {
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

            // 3. Prepare Config
            let mut config = self.current_config.clone();

            // APPLY OVERRIDE i.e. use per-pair setting, not global default
            let pair_upper = pair.to_uppercase();
            if let Some(horizon) = self.config_overrides.get(&pair_upper) {
                config.price_horizon = horizon.clone();
            } else {
                if let Some(horizon) = self.config_overrides.get(&pair) {
                    config.price_horizon = horizon.clone();
                }
            }

            // 4. Send Request
            let req = JobRequest {
                pair_name: pair,
                current_price: final_price_opt, // Pass Option<f64>
                config,                         // Use the local 'config' with overrides applied
                timeseries: self.timeseries.clone(),
                existing_profile, // Pass cached profile
                mode,             // Auto or manual job
            };

            let _ = self.job_tx.send(req);
        }
    }
}
