use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StationId {
    Scalp,
    Day,
    #[default]
    Swing,
    Macro,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunerStation {
    pub id: StationId,
    pub name: String,
    pub target_min_hours: f64,
    pub target_max_hours: f64,
    pub scan_ph_min: f64,
    pub scan_ph_max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeTunerConfig {
    pub stations: Vec<TunerStation>,
}

impl Default for TimeTunerConfig {
    fn default() -> Self {
        Self {
            stations: vec![
                TunerStation {
                    id: StationId::Scalp,
                    name: "⚡ SCALP".to_string(),
                    target_min_hours: 1.0,
                    target_max_hours: 6.0,
                    scan_ph_min: 0.01,
                    scan_ph_max: 0.04,
                },
                TunerStation {
                    id: StationId::Day,
                    name: "☀️ DAY".to_string(),
                    target_min_hours: 6.0,
                    target_max_hours: 24.0,
                    scan_ph_min: 0.03,
                    scan_ph_max: 0.08,
                },
                TunerStation {
                    id: StationId::Swing,
                    name: "🌊 SWING".to_string(),
                    target_min_hours: 24.0,
                    target_max_hours: 120.0,
                    scan_ph_min: 0.05,
                    scan_ph_max: 0.15,
                },
                TunerStation {
                    id: StationId::Macro,
                    name: "🏔️ MACRO".to_string(),
                    target_min_hours: 336.0,
                    target_max_hours: 2160.0,
                    scan_ph_min: 0.15,
                    scan_ph_max: 0.60,
                },
            ],
        }
    }
}
