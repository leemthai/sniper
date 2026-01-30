use serde::{Deserialize, Serialize};

use crate::config::PhPct;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StationId {
    Scalp,
    Day,
    #[default]
    Swing,
    Macro,
}

#[derive(Debug, Clone, Copy)]
pub struct TunerStation {
    pub id: StationId,
    pub name: &'static str,
    pub target_min_hours: f64,
    pub target_max_hours: f64,
    pub scan_ph_min: PhPct,
    pub scan_ph_max: PhPct,
}

#[derive(Debug, Clone, Copy)]
pub struct TimeTunerConfig {
    pub stations: &'static [TunerStation],
}