// Domain types and value objects
pub mod auto_duration;
pub mod candle;
pub mod pair_interval;

// Re-export commonly used types
pub use candle::Candle;
pub use pair_interval::PairInterval;
