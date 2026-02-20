mod pre_main_async;
mod price_stream;
#[cfg(not(target_arch = "wasm32"))]
mod provider;
#[cfg(not(target_arch = "wasm32"))]
mod storage;

mod timeseries;

#[cfg(not(target_arch = "wasm32"))]
mod results_repo;

#[cfg(not(target_arch = "wasm32"))]
mod ledger_io;

pub use pre_main_async::fetch_pair_data; // Must be pub not pub(crate)
pub use price_stream::PriceStreamManager; // Must be pub not pub(crate)
#[cfg(not(target_arch = "wasm32"))]
pub use storage::{MarketDataStorage, SqliteStorage};
#[cfg(target_arch = "wasm32")]
pub use timeseries::WasmDemoData;
pub use timeseries::{CacheFile, TimeSeriesCollection};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use {
    ledger_io::{load_ledger, save_ledger},
    provider::{BinanceProvider, MarketDataProvider},
    results_repo::{ResultsRepositoryTrait, SqliteResultsRepository, TradeResult},
    timeseries::{GlobalRateLimiter, load_klines},
};
