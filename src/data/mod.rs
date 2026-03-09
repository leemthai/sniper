mod pre_main_async;
mod price_stream;
mod timeseries;

#[cfg(not(target_arch = "wasm32"))]
mod binance;
#[cfg(not(target_arch = "wasm32"))]
mod ledger_io;
#[cfg(not(target_arch = "wasm32"))]
mod provider;
#[cfg(not(target_arch = "wasm32"))]
mod results_repo;
#[cfg(not(target_arch = "wasm32"))]
mod storage;

pub use {
    pre_main_async::fetch_pair_data,
    price_stream::PriceStreamManager,
    timeseries::{CacheFile, TimeSeriesCollection},
};

#[cfg(not(target_arch = "wasm32"))]
pub use storage::{MarketDataStorage, SqliteStorage};

#[cfg(target_arch = "wasm32")]
pub use timeseries::WasmDemoData;

#[cfg(not(target_arch = "wasm32"))]
pub use results_repo::{RunSummary, SqliteResultsRepository};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use {
    binance::{BINANCE_API, BINANCE_MAX_PAIRS, BinanceApiConfig},
    ledger_io::{load_ledger, save_ledger},
    provider::{BinanceProvider, MarketDataProvider},
    results_repo::{ResultsRepositoryTrait, TradeResult},
    timeseries::{GlobalRateLimiter, load_klines},
};
