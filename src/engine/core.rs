use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::utils::time_utils::AppInstant;

use crate::analysis::MultiPairMonitor;

use crate::config::{ANALYSIS, AnalysisConfig, PriceHorizonConfig};

use crate::data::price_stream::PriceStreamManager;
use crate::data::timeseries::TimeSeriesCollection;

use crate::models::horizon_profile::HorizonProfile;
use crate::models::pair_context::PairContext;
use crate::models::trading_view::TradingModel;

use super::messages::{JobRequest, JobResult};
use super::state::PairState;
use super::worker;

pub struct SniperEngine {
    /// Registry of all pairs
    pub pairs: HashMap<String, PairState>,

    /// Shared immutable data
    pub timeseries: Arc<TimeSeriesCollection>,

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
}

impl SniperEngine {
    /// Initialize the engine, spawn workers, and start the price stream.
    pub fn new(timeseries: TimeSeriesCollection) -> Self {
        let timeseries_arc = Arc::new(timeseries);
        let price_stream = Arc::new(PriceStreamManager::new());

        let (job_tx, job_rx) = channel::<JobRequest>();
        let (result_tx, result_rx) = channel::<JobResult>();

        // NATIVE: Pass the receiver to the thread. It moves out of this scope.
        #[cfg(not(target_arch = "wasm32"))]
        worker::spawn_worker_thread(job_rx, result_tx);

        let mut pairs = HashMap::new();
        for pair in timeseries_arc.unique_pair_names() {
            pairs.insert(pair, PairState::new());
        }

        let all_names: Vec<String> = pairs.keys().cloned().collect();
        price_stream.subscribe_all(all_names);

        Self {
            pairs,
            timeseries: timeseries_arc,
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
        }
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
    pub fn update(&mut self) {
        // 1. WASM ONLY: Process jobs manually in the main thread
        #[cfg(target_arch = "wasm32")]
        {
            // Non-blocking check for work
            if let Ok(req) = self.job_rx.try_recv() {
                // Run sync calculation
                worker::process_request_sync(req, self.result_tx.clone());
            }
        }

        // 2. Process Results (Swap Buffers)
        while let Ok(result) = self.result_rx.try_recv() {
            self.handle_job_result(result);
        }

        // 2. Check Triggers (Price Movement)
        self.check_automatic_triggers();

        // 3. Dispatch Jobs
        self.process_queue();
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
        self.timeseries.unique_pair_names()
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
        #[cfg(debug_assertions)]
        log::info!("ENGINE: Global Recalc Triggered (Startup/Reset)");

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

        #[cfg(debug_assertions)]
        log::info!("ENGINE: Recalc Triggered for [{}] by [{}]", pair, _reason);


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
            // This ensures the UI displays the correct number (e.g. "99 found")
            // even if the analysis failed due to minimum requirements.
            state.last_candle_count = result.candle_count;

            match result.result {
                Ok(model) => {
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
                    log::info!("Worker failed for {}: {}", result.pair_name, e);
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
                            
                            self.dispatch_job(pair.clone(), None); 
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
            #[cfg(debug_assertions)]
            log::warn!("ENGINE QUEUE: Popped job for [{}]", pair); // <--- LOG THIS

            self.dispatch_job(pair, price_opt);
        }
    }

    fn dispatch_job(&mut self, pair: String, price_override: Option<f64>) {
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
                config, // <--- CRITICAL FIX: Use the local 'config' with overrides applied
                timeseries: self.timeseries.clone(),
                existing_profile, // <--- NEW: Pass cached profile
            };

            let _ = self.job_tx.send(req);
        }
    }
}
