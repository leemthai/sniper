use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize, Default)]
pub(crate) enum SimDirection {
    #[default]
    Up,
    Down,
}

impl fmt::Display for SimDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimDirection::Up => write!(f, "▲ PRICE UP"),
            SimDirection::Down => write!(f, "▼ PRICE DOWN"),
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize, Default)]
pub(crate) enum SimStepSize {
    #[default]
    Point1, // 0.1%
    Point5, // 0.5%
    One,    // 1%
    Five,   // 5%
    Ten,    // 10%
}

impl SimStepSize {
    pub(crate) fn as_percentage(&self) -> f64 {
        match self {
            SimStepSize::Point1 => 0.001,
            SimStepSize::Point5 => 0.005,
            SimStepSize::One => 0.01,
            SimStepSize::Five => 0.05,
            SimStepSize::Ten => 0.10,
        }
    }

    pub(crate) fn cycle(&mut self) {
        *self = match self {
            SimStepSize::Point1 => SimStepSize::Point5,
            SimStepSize::Point5 => SimStepSize::One,
            SimStepSize::One => SimStepSize::Five,
            SimStepSize::Five => SimStepSize::Ten,
            SimStepSize::Ten => SimStepSize::Point1,
        };
    }
}

// impl Default for SimStepSize {
//     fn default() -> Self {
//         Self::Point1
//     }
// }

impl fmt::Display for SimStepSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1}%", self.as_percentage() * 100.0)
    }
}