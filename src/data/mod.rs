// Data loading, caching, and streaming
pub mod pre_main_async;
pub mod price_stream;
pub mod timeseries;
pub mod storage;
pub mod provider;

#[cfg(not(target_arch = "wasm32"))]
pub mod rate_limiter;

// Re-export commonly used types
pub use pre_main_async::fetch_pair_data;
pub use price_stream::PriceStreamManager;
pub use timeseries::TimeSeriesCollection;