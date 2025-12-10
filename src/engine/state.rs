use crate::models::trading_view::TradingModel;
use std::sync::Arc;
use std::time::Instant;

/// Represents the state of a single pair in the engine.
#[derive(Debug, Clone)]
pub struct PairState {
    /// THE FRONT BUFFER.
    /// The UI reads this every frame. It is never locked for writing.
    /// When a new model is ready, we simply replace this Arc pointer.
    pub model: Option<Arc<TradingModel>>,

    /// Metadata for the trigger system
    pub last_update_price: f64,
    pub last_update_time: Instant,

    /// Is a worker currently crunching this pair?
    pub is_calculating: bool,

    /// Last error (if any) to show in UI
    pub last_error: Option<String>,
}

impl PairState {
    pub fn new() -> Self {
        Self {
            model: None,
            last_update_price: 0.0,
            last_update_time: Instant::now(),
            is_calculating: false,
            last_error: None,
        }
    }

    /// The "Swap" operation.
    /// Promotes the Back Buffer (Result) to the Front Buffer (UI).
    pub fn update_buffer(&mut self, new_model: Arc<TradingModel>) {
        // THIS IS THE SWAP.
        // Overwriting 'self.model' drops the old pointer and sets the new one.
        // It takes nanoseconds.
        self.model = Some(new_model);
        self.is_calculating = false;
        self.last_update_time = Instant::now();
        self.last_error = None;
    }
}
