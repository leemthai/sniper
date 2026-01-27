use serde::{Deserialize, Serialize};

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
    pub scan_ph_min: f64,
    pub scan_ph_max: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct TimeTunerConfig {
    pub stations: &'static [TunerStation],
}