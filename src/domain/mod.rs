// Domain types and value objects
mod candle;
mod pair_interval;
mod price_horizon;

// Re-export commonly used types
pub(crate) use candle::Candle;
pub(crate) use price_horizon::{auto_select_ranges, calculate_price_range};

// Re-export commonly used types to the world
pub use pair_interval::PairInterval; // make_demo_cache.rs uses it
