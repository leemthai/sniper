use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::analysis::MultiPairMonitor;
use crate::config::{ANALYSIS, AnalysisConfig};
use crate::data::price_stream::PriceStreamManager;
use crate::data::timeseries::TimeSeriesCollection;
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

    /// Worker Communication
    job_tx: Sender<JobRequest>,
    result_rx: Receiver<JobResult>,

    /// Queue Logic
    pub queue: VecDeque<String>,

    pub multi_pair_monitor: MultiPairMonitor,

    /// Live configuration state
    pub current_config: AnalysisConfig,
}

impl SniperEngine {
    /// Initialize the engine, spawn workers, and start the price stream.
    pub fn new(timeseries: TimeSeriesCollection) -> Self {
        let timeseries_arc = Arc::new(timeseries);
        let price_stream = Arc::new(PriceStreamManager::new());

        // 1. Setup Channels
        let (job_tx, job_rx) = channel::<JobRequest>();
        let (result_tx, result_rx) = channel::<JobResult>();

        // 2. Spawn Background Workers
        // We spawn a dedicated thread that listens to job_rx and sends to result_tx.
        worker::spawn_worker_thread(job_rx, result_tx);

        // 3. Initialize Pair States
        let mut pairs = HashMap::new();
        for pair in timeseries_arc.unique_pair_names() {
            pairs.insert(pair, PairState::new());
        }

        // 4. Start Price Stream
        let all_names = pairs.keys().cloned().collect();
        price_stream.subscribe_all(all_names);

        Self {
            pairs,
            timeseries: timeseries_arc,
            price_stream,
            job_tx,
            result_rx,
            queue: VecDeque::new(),
            multi_pair_monitor: MultiPairMonitor::new(),
            current_config: ANALYSIS.clone(),
        }
    }

    /// THE GAME LOOP.
    /// Call this once per frame in `update()`.
    pub fn update(&mut self) {
        // 1. Process Results (Swap Buffers)
        // Non-blocking drain of the result channel
        while let Ok(result) = self.result_rx.try_recv() {
            self.handle_job_result(result);
        }

        // 2. Check Triggers (Price Movement)
        self.check_automatic_triggers();

        // 3. Dispatch Jobs
        self.process_queue();
    }

    // --- TELEMETRY GETTERS (For Status Bar) ---
    pub fn get_queue_len(&self) -> usize {
        self.queue.len()
    }

    pub fn get_worker_status_msg(&self) -> Option<String> {
        // Find if any pair is calculating
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

    // --- CONFIG UPDATES ---
    pub fn update_config(&mut self, new_config: AnalysisConfig) {
        self.current_config = new_config;
        // Optimization: We could auto-trigger a recalc here if we wanted,
        // but explicit invalidation from UI is safer.
    }

    /// Accessor for UI: Get the current Front Buffer
    pub fn get_model(&self, pair: &str) -> Option<Arc<TradingModel>> {
        self.pairs.get(pair).and_then(|state| state.model.clone())
    }

    pub fn get_price(&self, pair: &str) -> Option<f64> {
        self.price_stream.get_price(pair)
    }

    // --- INTERNAL LOGIC ---
    fn handle_job_result(&mut self, result: JobResult) {
        if let Some(state) = self.pairs.get_mut(&result.pair_name) {
            match result.result {
                Ok(model) => {
                    // Update Buffer
                    state.update_buffer(model.clone()); // <--- Passing the Arc to the State

                    // Update Monitor
                    // Note: We need a PairContext.
                    // Ideally TradingModel converts to PairContext, or we adapt Monitor.
                    // For now, let's assume we can create PairContext from Model.
                    let ctx = crate::models::pair_context::PairContext::new(
                        (*model).clone(),
                        state.last_update_price, // or fetch current price
                    );
                    self.multi_pair_monitor.add_pair(ctx);
                }
                Err(e) => {
                    // Handle Failure
                    log::error!("Worker failed for {}: {}", result.pair_name, e);
                    state.last_error = Some(e);
                    state.is_calculating = false;
                }
            }
        }
    }

    fn process_queue(&mut self) {
        if self.queue.is_empty() {
            return;
        }

        // Simple FIFO for now.
        // We verify the pair isn't ALREADY calculating to avoid stacking jobs.
        if let Some(pair) = self.queue.front() {
            if let Some(state) = self.pairs.get(pair) {
                if state.is_calculating {
                    // Skip, it's busy. Maybe rotate queue?
                    // For now, just return. Worker is busy.
                    return;
                }
            }
        }

        if let Some(pair) = self.queue.pop_front() {
            self.dispatch_job(pair);
        }
    }

    fn dispatch_job(&mut self, pair: String) {
        if let Some(state) = self.pairs.get_mut(&pair) {
            let price = self.price_stream.get_price(&pair).unwrap_or(0.0);

            state.is_calculating = true;
            state.last_update_price = price;

            let req = JobRequest {
                pair_name: pair,
                current_price: price,
                // CRITICAL FIX: Use the Engine's current_config, NOT the static ANALYSIS constant
                config: self.current_config.clone(),
                timeseries: self.timeseries.clone(),
            };

            let _ = self.job_tx.send(req);
        }
    }

    /// Public method to force a recalc (e.g. user clicked pair)
    pub fn force_recalc(&mut self, pair: &str) {
        // Push to front of queue
        self.queue.push_front(pair.to_string());
    }

    fn check_automatic_triggers(&mut self) {
        // Iterate over all tracked pairs
        let pairs: Vec<String> = self.pairs.keys().cloned().collect();

        for pair in pairs {
            // 1. Get Live Price
            if let Some(current_price) = self.price_stream.get_price(&pair) {
                // 2. Get State
                if let Some(state) = self.pairs.get_mut(&pair) {
                    // Don't queue if already busy
                    if state.is_calculating {
                        continue;
                    }

                    // 3. Logic: Has price moved enough?
                    // Use the configured threshold (e.g., 0.5% or 1.0%)
                    let threshold = ANALYSIS.cva.price_recalc_threshold_pct;

                    // Handle startup case (last_update_price is 0.0)
                    if state.last_update_price == 0.0 {
                        // Initial load
                        self.queue.push_back(pair);
                    } else {
                        let pct_diff = (current_price - state.last_update_price).abs()
                            / state.last_update_price;

                        if pct_diff >= threshold {
                            log::info!(
                                "[{}] Trigger: Price moved {:.4}% (Threshold {:.4}%)",
                                pair,
                                pct_diff * 100.0,
                                threshold * 100.0
                            );
                            self.queue.push_back(pair);
                        }
                    }
                }
            }
        }
    }

    pub fn get_signals(&self) -> Vec<&crate::models::pair_context::PairContext> {
        // Direct access, because SniperEngine owns the monitor
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
}
