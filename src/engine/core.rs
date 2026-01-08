use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, RwLock};

use crate::utils::time_utils::AppInstant;

use crate::analysis::MultiPairMonitor;
use crate::analysis::adaptive::AdaptiveParameters;
use crate::analysis::market_state::MarketState;

use crate::config::{ANALYSIS, AnalysisConfig, PriceHorizonConfig};

use crate::data::price_stream::PriceStreamManager;
use crate::data::timeseries::TimeSeriesCollection;

use crate::models::horizon_profile::HorizonProfile;
use crate::models::ledger::OpportunityLedger;
use crate::models::pair_context::PairContext;
use crate::models::timeseries::LiveCandle;
use crate::models::trading_view::{
    LiveOpportunity, TradeFinderRow, TradeOpportunity, TradingModel,
};

use crate::utils::maths_utils::calculate_percent_diff;

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

    /// Owned monitor
    pub multi_pair_monitor: MultiPairMonitor,

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

        // 5. Construct Engine
        Self {
            pairs,
            timeseries: timeseries_arc, // Pass the Arc<RwLock> we created
            price_stream,
            multi_pair_monitor: MultiPairMonitor::new(),
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
            candle_rx,
            candle_tx,
        }
    }

    /// Process incoming live candles
    pub fn process_live_data(&mut self) {
        // We accumulate updates to minimize lock contention?
        // No, try_recv is fast. Let's just lock once if we have data.

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
                        // #[cfg(debug_assertions)]
                        // log::info!(
                        //     "HEARTBEAT: Closed Candle for {} @ {} so gonna trigger force_recalc() on this pair....",
                        //     candle.symbol,
                        //     candle.close
                        // );

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
    }

    /// Generates the master list for the Trade Finder.
    pub fn get_trade_finder_rows(
        &self,
        overrides: Option<&std::collections::HashMap<String, f64>>,
    ) -> Vec<TradeFinderRow> {
        crate::trace_time!("Core: Get TradeFinder Rows", 2000, {
            let mut rows = Vec::new();

            let ph_pct = self.current_config.price_horizon.threshold_pct;
            let lookback = AdaptiveParameters::calculate_trend_lookback_candles(ph_pct);
            let now_ms = crate::utils::TimeUtils::now_timestamp_ms();
            let day_ms = 86_400_000;

            // 1. Group Ledger Opportunities by Pair for fast lookup
            let mut ops_by_pair: std::collections::HashMap<String, Vec<&TradeOpportunity>> =
                std::collections::HashMap::new();
            for op in self.ledger.get_all() {
                ops_by_pair
                    .entry(op.pair_name.clone())
                    .or_default()
                    .push(op);
            }

            let ts_guard = self.timeseries.read().unwrap();

            for (pair, _state) in &self.pairs {
                // 2. Get Context (Price)
                let current_price = if let Some(map) = overrides {
                    map.get(pair)
                        .copied()
                        .or_else(|| self.price_stream.get_price(pair))
                } else {
                    self.price_stream.get_price(pair)
                }
                .unwrap_or(0.0);

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
                let valid_ops: Vec<&crate::models::trading_view::TradeOpportunity> = raw_ops
                    .iter()
                    .filter(|&&op| op.expected_roi() > 0.0)
                    .map(|&op| op)
                    .collect();

                let total_ops = valid_ops.len();

                if total_ops > 0 {
                    // Create a ROW for EACH Opportunity
                    for op in valid_ops {
                        let live_opp = LiveOpportunity {
                            opportunity: op.clone(),
                            current_price,
                            live_roi: op.live_roi(current_price),
                            annualized_roi: op.live_annualized_roi(current_price),
                            risk_pct: crate::utils::maths_utils::calculate_percent_diff(
                                op.stop_price,
                                current_price,
                            ),
                            reward_pct: crate::utils::maths_utils::calculate_percent_diff(
                                op.target_price,
                                current_price,
                            ),
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
                .unwrap_or(std::cmp::Ordering::Equal)
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
    pub fn update(&mut self, protected_id: Option<&str>) {
        // 0. Ingest Live Data (The Heartbeat
        let t1 = AppInstant::now();
        self.process_live_data();
        let d1 = t1.elapsed().as_micros();

        // 1. WASM ONLY: Process jobs manually in the main thread
        #[cfg(target_arch = "wasm32")]
        {
            // Non-blocking check for work
            if let Ok(req) = self.job_rx.try_recv() {
                // Run sync calculation
                worker::process_request_sync(req, self.result_tx.clone());
            }
        }

        // 3. Results
        let t2 = AppInstant::now();
        while let Ok(result) = self.result_rx.try_recv() {
            self.handle_job_result(result);
        }
        let d2 = t2.elapsed().as_micros();

        // 4. Triggers
        let t3 = AppInstant::now();
        self.check_automatic_triggers();
        let d3 = t3.elapsed().as_micros();

        // 5. Queue
        let t4 = AppInstant::now();
        self.process_queue();
        let d4 = t4.elapsed().as_micros();

        // Log if total is significant (> 10ms)
        let total = d1 + d2 + d3 + d4;
        if total > 100_000 {
            log::error!(
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

    pub fn get_signals(&self) -> Vec<&PairContext> {
        self.multi_pair_monitor.get_signals()
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

    // --- TELEMETRY ---

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

    // --- CONFIG UPDATES ---

    pub fn update_config(&mut self, new_config: AnalysisConfig) {
        self.current_config = new_config;
    }

    /// Smart Global Invalidation
    pub fn trigger_global_recalc(&mut self, priority_pair: Option<String>) {
        // #[cfg(debug_assertions)]
        // log::info!("ENGINE: Global Recalc Triggered (Startup/Reset)");

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

        // log::info!("Global Invalidation: Queue Rebuilt ({} pairs).", self.queue.len());
    }

    /// Force a single recalc with optional price override
    pub fn force_recalc(&mut self, pair: &str, price_override: Option<f64>, _reason: &str) {
        // #[cfg(debug_assertions)]
        // log::info!("ENGINE: Recalc Triggered for [{}] by [{}]", pair, _reason);

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
                    // --- DIAGNOSTIC START ---
                    let pair_count_before = self
                        .ledger
                        .opportunities
                        .values()
                        .filter(|o| o.pair_name == result.pair_name)
                        .count();
                    // ------------------------

                    // --- NEW: Sync to Ledger ---
                    for op in &model.opportunities {
                        self.ledger.evolve(op.clone());
                    }

                    // --- DIAGNOSTIC END ---
                    let pair_count_after = self
                        .ledger
                        .opportunities
                        .values()
                        .filter(|o| o.pair_name == result.pair_name)
                        .count();
                    let total_ledger = self.ledger.opportunities.len();

                    // Warn level to ensure it shows in release mode
                    // log::warn!(
                    //     "LEDGER MONITOR [{}]: Ops for this pair {} -> {}. Total Ledger: {}",
                    //     result.pair_name,
                    //     pair_count_before,
                    //     pair_count_after,
                    //     total_ledger
                    // );
                    // ---------------------

                    // 3. Success: Update State
                    state.model = Some(model.clone());

                    // 3. Success: Update State
                    state.model = Some(model.clone());
                    state.is_calculating = false;
                    state.last_update_time = AppInstant::now();
                    state.last_error = None;

                    // 4. Monitor Logic: Feed the result to the signal monitor
                    let ctx = PairContext::new((*model).clone(), state.last_update_price);
                    self.multi_pair_monitor.add_pair(ctx);
                }
                Err(e) => {
                    // 5. Failure: Clear Model, Set Error
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

                        // Normal Logic
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

        // Pop the tuple
        if let Some((pair, price_opt)) = self.queue.pop_front() {
            // #[cfg(debug_assertions)]
            // log::warn!("ENGINE QUEUE: Popped job for [{}]", pair); // <--- LOG THIS

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
