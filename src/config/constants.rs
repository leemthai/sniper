//! General configuration constants used across the application.

/// Default limit for number of klines returned in a single request
pub const DEFAULT_KLINES_LIMIT: i32 = 1000;

// Max number of pairs to read. If already have more than MAX_PAIRS saved to disk, all will be read.
pub const MAX_PAIRS: usize = 20;
