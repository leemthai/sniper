use serde::{Deserialize, Serialize};

use crate::config::{PhPct, constants};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StationId {
    Scalp,
    Day,
    #[default]
    Swing,
    Macro,
}

impl StationId {
    pub fn short_name(self) -> &'static str {
        constants::tuner::STATIONS
            .iter()
            .find(|s| s.id == self)
            .map(|s| s.short_name)
            .unwrap_or("?")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TunerStation {
    pub id: StationId,
    pub name: &'static str,
    pub short_name: &'static str,
    pub target_min_hours: f64,
    pub target_max_hours: f64,
    pub scan_ph_min: PhPct,
    pub scan_ph_max: PhPct,
}

#[derive(Debug, Clone, Copy)]
pub struct TimeTunerConfig {
    pub stations: &'static [TunerStation],
}