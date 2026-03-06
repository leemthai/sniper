mod debug;
mod demo;
mod persistence;

pub(crate) use debug::LOG_PERFORMANCE;

#[cfg(debug_assertions)]
pub(crate) use debug::DF;

pub use {
    demo::DEMO,
    persistence::{PERSISTENCE, kline_cache_filename},
};
