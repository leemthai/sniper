// Data loading, caching, and streaming
pub mod pre_main_async;
pub mod price_stream;
pub mod timeseries;

// Re-export commonly used types
pub use pre_main_async::fetch_pair_data;
pub use price_stream::PriceStreamManager;
pub use timeseries::TimeSeriesCollection;
// Only re-export this for non-WASM targets
#[cfg(not(target_arch = "wasm32"))]
pub use timeseries::serde_version::write_timeseries_data_async;
