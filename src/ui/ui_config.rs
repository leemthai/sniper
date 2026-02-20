use eframe::egui::{Color32, Frame, Margin, Stroke};

pub use crate::ui::ui_text::UI_TEXT;

/// UI Colors for consistent theming
#[derive(Clone, Copy, Default)]
pub struct UiColors {
    pub label: Color32,
    pub heading: Color32,
    pub subsection_heading: Color32,
    pub central_panel: Color32,
    pub side_panel: Color32,
}

/// Main UI configuration struct that holds all UI-related settings
#[derive(Default, Clone, Copy)]
pub struct UiConfig {
    pub colors: UiColors,
    // pub max_journey_zone_lines: usize,
}

/// Global UI configuration instance
pub static UI_CONFIG: UiConfig = UiConfig {
    colors: UiColors {
        label: Color32::GRAY,     // This sets every label globally to this color
        heading: Color32::YELLOW, // Sets every heading
        subsection_heading: Color32::ORANGE, // Sets every subsection heading
        central_panel: Color32::from_rgb(125, 50, 50),
        side_panel: Color32::from_rgb(25, 25, 25),
    },
};

impl UiConfig {
    /// Frame for Left/Right panels (Standard padding)
    pub fn side_panel_frame(&self) -> Frame {
        Frame {
            fill: self.colors.side_panel,
            stroke: Stroke::NONE,
            inner_margin: Margin::same(8),
            ..Default::default()
        }
    }

    /// Frame for the Top Toolbar (Standard padding)
    pub fn top_panel_frame(&self) -> Frame {
        Frame {
            fill: self.colors.side_panel,
            stroke: Stroke::NONE,
            inner_margin: Margin::same(8),
            ..Default::default()
        }
    }

    /// Frame for Bottom Status bar (Tighter vertical padding)
    pub fn bottom_panel_frame(&self) -> Frame {
        Frame {
            fill: self.colors.side_panel,
            stroke: Stroke::NONE,
            inner_margin: Margin::symmetric(8, 4), // Tighter vertically
            ..Default::default()
        }
    }

    // Frame for the Plot area
    pub fn central_panel_frame(&self) -> Frame {
        Frame {
            fill: self.colors.central_panel,
            stroke: Stroke::NONE,
            inner_margin: Margin {
                left: 0,
                right: 8, // <--- THE GAP allows "PAIRNAME Price" to be fully viewable not smashed against the right border
                top: 0,
                bottom: 0,
            },
            ..Default::default()
        }
    }
}
