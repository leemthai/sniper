use eframe::egui::{Color32, Frame, Margin, Stroke};

#[derive(Clone, Copy, Default)]
pub struct UiColors {
    pub label: Color32,
    pub heading: Color32,
    pub subsection_heading: Color32,
    pub central_panel: Color32,
    pub side_panel: Color32,
}

#[derive(Default, Clone, Copy)]
pub struct UiConfig {
    pub colors: UiColors,
}

pub static UI_CONFIG: UiConfig = UiConfig {
    colors: UiColors {
        label: Color32::GRAY,
        heading: Color32::YELLOW,
        subsection_heading: Color32::ORANGE,
        central_panel: Color32::from_rgb(125, 50, 50),
        side_panel: Color32::from_rgb(25, 25, 25),
    },
};

impl UiConfig {
    pub fn side_panel_frame(&self) -> Frame {
        Frame {
            fill: self.colors.side_panel,
            stroke: Stroke::NONE,
            inner_margin: Margin::same(8),
            ..Default::default()
        }
    }

    pub fn top_panel_frame(&self) -> Frame {
        Frame {
            fill: self.colors.side_panel,
            stroke: Stroke::NONE,
            inner_margin: Margin::same(8),
            ..Default::default()
        }
    }

    pub fn bottom_panel_frame(&self) -> Frame {
        Frame {
            fill: self.colors.side_panel,
            stroke: Stroke::NONE,
            inner_margin: Margin::symmetric(8, 4),
            ..Default::default()
        }
    }

    pub fn central_panel_frame(&self) -> Frame {
        Frame {
            fill: self.colors.central_panel,
            stroke: Stroke::NONE,
            inner_margin: Margin {
                left: 0,
                right: 8,
                top: 0,
                bottom: 0,
            },
            ..Default::default()
        }
    }
}
