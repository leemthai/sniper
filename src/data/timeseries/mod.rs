#[cfg(not(target_arch = "wasm32"))]
mod bn_kline;
mod cache_file;
#[cfg(not(target_arch = "wasm32"))]
mod rate_limiter;
mod time_series_collection;
#[cfg(target_arch = "wasm32")]
mod wasm_demo;
pub use cache_file::CacheFile;
pub use time_series_collection::TimeSeriesCollection;
#[cfg(target_arch = "wasm32")]
pub use wasm_demo::WasmDemoData;
#[cfg(not(target_arch = "wasm32"))]
pub use {bn_kline::load_klines, rate_limiter::GlobalRateLimiter};
