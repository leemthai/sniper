use eframe::egui::Color32;

pub use crate::ui::ui_text::{UI_TEXT, UiText};

/// UI Colors for consistent theming
#[derive(Clone, Copy, Default)]
pub struct UiColors {
    pub label: Color32,
    pub heading: Color32,
    pub subsection_heading: Color32,
    pub central_panel: Color32,
    pub side_panel: Color32,
    pub journey_bull: Color32,
    pub journey_bear: Color32,
}

/// Main UI configuration struct that holds all UI-related settings
#[derive(Default, Clone, Copy)]
pub struct UiConfig {
    pub colors: UiColors,
    pub max_journey_zone_lines: usize,
}

/// Global UI configuration instance
pub static UI_CONFIG: UiConfig = UiConfig {
    colors: UiColors {
        label: Color32::GRAY,     // This sets every label globally to this color
        heading: Color32::YELLOW, // Sets every heading
        subsection_heading: Color32::ORANGE, // Sets every subsection heading
        central_panel: Color32::from_rgb(125, 50, 50),
        side_panel: Color32::from_rgb(25, 25, 25),
        journey_bull: Color32::from_rgb(130, 200, 140),
        journey_bear: Color32::from_rgb(180, 160, 230),
    },
    max_journey_zone_lines: 10,
};
