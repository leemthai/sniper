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

    /// Owned monitor
    pub multi_pair_monitor: MultiPairMonitor,

    /// Worker Communication
    job_tx: Sender<JobRequest>,
    result_rx: Receiver<JobResult>,

    /// Queue Logic
    pub queue: VecDeque<String>,

    /// The Live Configuration State
    pub current_config: AnalysisConfig,
}

impl SniperEngine {
    /// Initialize the engine, spawn workers, and start the price stream.
    pub fn new(timeseries: TimeSeriesCollection) -> Self {
        let timeseries_arc = Arc::new(timeseries);
        let price_stream = Arc::new(PriceStreamManager::new());

        let (job_tx, job_rx) = channel::<JobRequest>();
        let (result_tx, result_rx) = channel::<JobResult>();

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
            queue: VecDeque::new(),
            current_config: ANALYSIS.clone(),
        }
    }

    /// THE GAME LOOP.
    /// Returns TRUE if the engine is busy (Queue not empty OR Workers calculating).
    /// This tells the UI to keep waking up (request_repaint).
    pub fn update(&mut self) -> bool {
        // 1. Process Results (Swap Buffers)
        while let Ok(result) = self.result_rx.try_recv() {
            self.handle_job_result(result);
        }

        // 2. Check Triggers (Price Movement)
        self.check_automatic_triggers();

        // 3. Dispatch Jobs
        self.process_queue();

        // 4. Report Busy Status
        // If we have items in queue OR any pair is currently calculating, we are busy.
        !self.queue.is_empty() || self.has_active_workers()
    }

    /// Accessor for UI
    pub fn get_model(&self, pair: &str) -> Option<Arc<TradingModel>> {
        self.pairs.get(pair).and_then(|state| state.model.clone())
    }

    pub fn get_price(&self, pair: &str) -> Option<f64> {
        self.price_stream.get_price(pair)
    }

    pub fn get_signals(&self) -> Vec<&crate::models::pair_context::PairContext> {
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

    // --- CONFIG UPDATES ---

    pub fn update_config(&mut self, new_config: AnalysisConfig) {
        self.current_config = new_config;
    }

    /// Smart Global Invalidation
    /// Clears queue, adds all pairs, prioritizes the selected pair.
    pub fn trigger_global_recalc(&mut self, priority_pair: Option<String>) {
        // 1. Clear existing queue (Don't process stale jobs)
        self.queue.clear();

        // 2. Identify pairs
        let mut all_pairs = self.get_all_pair_names();

        // 3. Handle Priority
        if let Some(vip) = priority_pair {
            // Remove VIP from general list if present
            if let Some(pos) = all_pairs.iter().position(|p| p == &vip) {
                all_pairs.remove(pos);
            }
            // Push VIP first
            self.queue.push_back(vip);
        }

        // 4. Push the rest
        for pair in all_pairs {
            self.queue.push_back(pair);
        }

        // 1. REQUESTED LOG: Print the new queue
        log::info!(
            "Global Invalidation: Queue Rebuilt ({} pairs). Head: {:?}",
            self.queue.len(),
            self.queue.front()
        );
    }

    /// Force a single recalc (e.g. user click)
    /// Checks for duplicates before adding.
    pub fn force_recalc(&mut self, pair: &str) {
        // Only push if not already in queue AND not currently calculating
        let is_calculating = self
            .pairs
            .get(pair)
            .map(|s| s.is_calculating)
            .unwrap_or(false);
        let in_queue = self.queue.contains(&pair.to_string());

        if !is_calculating && !in_queue {
            // Priority: Front of queue
            self.queue.push_front(pair.to_string());
        }
    }

    // --- INTERNAL LOGIC ---

    fn has_active_workers(&self) -> bool {
        self.pairs.values().any(|s| s.is_calculating)
    }

    fn handle_job_result(&mut self, result: JobResult) {
        if let Some(state) = self.pairs.get_mut(&result.pair_name) {
            match result.result {
                Ok(model) => {
                    state.update_buffer(model.clone());

                    let ctx = crate::models::pair_context::PairContext::new(
                        (*model).clone(),
                        state.last_update_price,
                    );
                    self.multi_pair_monitor.add_pair(ctx);
                }
                Err(e) => {
                    log::error!("Worker failed for {}: {}", result.pair_name, e);
                    state.last_error = Some(e);
                    state.is_calculating = false;
                }
            }
        }
    }

    fn check_automatic_triggers(&mut self) {
        let pairs: Vec<String> = self.pairs.keys().cloned().collect();

        for pair in pairs {
            if let Some(current_price) = self.price_stream.get_price(&pair) {
                if let Some(state) = self.pairs.get_mut(&pair) {
                    // Don't queue if already busy or already queued
                    if state.is_calculating || self.queue.contains(&pair) {
                        continue;
                    }

                    // Handle startup (0.0)
                    if state.last_update_price == 0.0 {
                        self.queue.push_back(pair);
                    } else {
                        let threshold = ANALYSIS.cva.price_recalc_threshold_pct;
                        let pct_diff = (current_price - state.last_update_price).abs()
                            / state.last_update_price;

                        if pct_diff >= threshold {
                            log::info!("[{}] Trigger: Price moved {:.4}%", pair, pct_diff * 100.0);
                            self.queue.push_back(pair);
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
        if let Some(pair) = self.queue.front() {
            // Double check: is it calculating now? (Race condition check)
            if let Some(state) = self.pairs.get(pair) {
                if state.is_calculating {
                    // It's busy. Rotate it to the back? Or just wait?
                    // Let's wait. Single threaded worker for now.
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
            // STRICT LOGIC: Only proceed if we actually have a price.
            if let Some(price) = self.price_stream.get_price(&pair) {
                state.is_calculating = true;
                state.last_update_price = price;

                let req = JobRequest {
                    pair_name: pair,
                    current_price: price,
                    config: self.current_config.clone(),
                    timeseries: self.timeseries.clone(),
                };

                // Send to worker. If receiver is dead, we ignore the error (engine shutting down).
                let _ = self.job_tx.send(req);
            } else {
                // No price available yet (e.g. WebSocket connecting).
                // We do nothing. The 'check_automatic_triggers' loop will pick this up
                // automatically once a valid price arrives.
            }
        }
    }
    // 3. NEW HELPER: Expose Status for UI
    pub fn get_pair_status(&self, pair: &str) -> (bool, Option<String>) {
        if let Some(state) = self.pairs.get(pair) {
            (state.is_calculating, state.last_error.clone())
        } else {
            (false, None)
        }
    }
}
