// src/app/mod.rs

pub(crate) mod phases;
pub(crate) mod root;
pub(crate) mod state;
pub use root::{App, ProgressEvent, SyncStatus};
