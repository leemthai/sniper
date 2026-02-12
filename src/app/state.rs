// src/app/state.rs

use crate::models::SyncStatus;

#[derive(Clone)]
pub(crate) struct RunningState;


pub(crate) enum AppState {
    Bootstrapping(BootstrapState),
    Tuning(TuningState),
    Running(RunningState),
}

impl Default for AppState {
    fn default() -> Self {
        AppState::Bootstrapping(BootstrapState::default())
    }
}

#[derive(Default, Clone)]
pub(crate) struct BootstrapState {
    pub(crate) pairs: std::collections::BTreeMap<usize, (String, SyncStatus)>,
    pub(crate) total_pairs: usize,
    pub(crate) completed: usize,
    pub(crate) failed: usize,
}

#[derive(Clone, Default)]
pub(crate) struct TuningState {
    pub(crate) todo_list: Vec<String>,
    pub(crate) total: usize,
    pub(crate) completed: usize,
}

