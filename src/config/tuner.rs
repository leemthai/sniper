use {
    crate::config::PhPct,
    serde::{Deserialize, Serialize},
    std::fmt,
};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum StationId {
    Scalp,
    Day,
    #[default]
    Swing,
    Macro,
}

impl fmt::Display for StationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.short_name())
    }
}

impl StationId {
    pub(crate) fn short_name(self) -> &'static str {
        STATIONS
            .iter()
            .find(|s| s.id == self)
            .map(|s| s.short_name)
            .unwrap_or("?")
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TunerStation {
    pub id: StationId,
    pub name: &'static str,
    pub short_name: &'static str,
    pub target_min_hours: f64,
    pub target_max_hours: f64,
    pub scan_ph_min: PhPct,
    pub scan_ph_max: PhPct,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TimeTunerConfig {
    pub stations: &'static [TunerStation],
}

pub(crate) const STATIONS: &[TunerStation] = &[
    TunerStation {
        id: StationId::Scalp,
        // name: "⚡ SCALP",
        // short_name: "⚡",
        name: "\u{f00d5} SCALP",
        short_name: "\u{f00d5}",
        target_min_hours: 1.0,
        target_max_hours: 6.0,
        scan_ph_min: PhPct::new(0.01),
        scan_ph_max: PhPct::new(0.04),
    },
    TunerStation {
        id: StationId::Day,
        name: "\u{f522} DAY",
        short_name: "\u{f522}",
        target_min_hours: 6.0,
        target_max_hours: 24.0,
        scan_ph_min: PhPct::new(0.03),
        scan_ph_max: PhPct::new(0.08),
    },
    TunerStation {
        id: StationId::Swing,
        // name: "🌊 SWING",
        // short_name: "🌊",
        name: "\u{f095b} SWING",
        short_name: "\u{f095b}",
        target_min_hours: 24.0,
        target_max_hours: 120.0,
        scan_ph_min: PhPct::new(0.05),
        scan_ph_max: PhPct::new(0.15),
    },
    TunerStation {
        id: StationId::Macro,
        name: "\u{eda7} INVEST",
        short_name: "\u{eda7}",
        target_min_hours: 336.0,
        target_max_hours: 2160.0,
        scan_ph_min: PhPct::new(0.15),
        scan_ph_max: PhPct::new(0.60),
    },
];

pub const TUNER_CONFIG: TimeTunerConfig = TimeTunerConfig { stations: STATIONS };
