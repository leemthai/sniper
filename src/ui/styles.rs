use eframe::egui::{Color32, RichText, Ui, Button, CursorIcon, Vec2, Stroke, CornerRadius};

use crate::config::plot::PLOT_CONFIG;
use crate::models::trading_view::TradeDirection;
use crate::ui::config::UI_CONFIG;


/// Creates a colored heading with uppercase text (not mono anymore, but can put back for stylisting reasons if requried)
pub fn colored_heading(text: impl Into<String>) -> RichText {
    let uppercase_text = text.into().to_uppercase() + ":";
    RichText::new(uppercase_text)
        .color(UI_CONFIG.colors.heading)
        // .monospace()
}

/// Creates a colored sub-section headingusing the configured label color
pub fn colored_subsection_heading(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(UI_CONFIG.colors.subsection_heading)
}

/// Creates a section heading with standard spacing
pub fn section_heading(ui: &mut Ui, text: impl Into<String>) {
    ui.add_space(10.0);
    ui.heading(colored_heading(text));
    ui.add_space(5.0);
}

/// Creates a separator with standard spacing
pub fn spaced_separator(ui: &mut Ui) {
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);
}


/// Extension trait to map Data Models to UI Colors
pub trait DirectionColor {
    fn color(&self) -> Color32;
}

impl DirectionColor for TradeDirection {
    fn color(&self) -> Color32 {
        match self {
            TradeDirection::Long => PLOT_CONFIG.color_long,
            TradeDirection::Short => PLOT_CONFIG.color_short,
        }
    }
}

/// Applies a semantic opacity factor to a color.
/// Wraps the internal egui logic to keep rendering code clean.
pub fn apply_opacity(color: Color32, factor: f32) -> Color32 {
    color.linear_multiply(factor)
}

// Helper for values
pub fn get_outcome_color(value: f64) -> Color32 {
    if value > 0.0 {
        PLOT_CONFIG.color_profit
    } else {
        PLOT_CONFIG.color_loss
    }
}


/// Extension trait to add semantic styling methods directly to `egui::Ui`.
pub trait UiStyleExt {
    /// Renders small, gray text (good for labels like "Coverage:").
    fn label_subdued(&mut self, text: impl Into<String>);

    /// Renders a "Label: Value" pair with consistent spacing and styling.
    /// The label is subdued, the value is colored.
    fn metric(&mut self, label: &str, value: &str, color: Color32);

    /// Renders a section header using the configured global color.
    fn label_header(&mut self, text: impl Into<String>);

    /// Renders a sub-section header using the configured global color.
    fn label_subheader(&mut self, text: impl Into<String>);
    
    /// Renders an error message (Red).
    fn label_error(&mut self, text: impl Into<String>);
    
    /// Renders a warning/info message (Yellow/Gold).
    fn label_warning(&mut self, text: impl Into<String>);

    /// Generates RichText styled specifically for a Primary Action Button (Green/Bold).
    fn button_text_primary(&self, text: impl Into<String>) -> RichText;
    
    /// Generates RichText styled specifically for a Secondary Action Button (White/Bold).
    fn button_text_secondary(&self, text: impl Into<String>) -> RichText;

    /// For help buttons
    fn help_button(&mut self, text: &str) -> bool; // Returns true if clicked

}

impl UiStyleExt for Ui {

fn help_button(&mut self, text: &str) -> bool {
        // 1. Use a Scope to modify styles for just this button
        self.scope(|ui| {
            let visuals = ui.visuals_mut();

            // --- A. BORDER (BG Stroke) COLOR ---
            visuals.widgets.inactive.bg_stroke = Stroke::new(2.0, PLOT_CONFIG.color_help_bg);
            visuals.widgets.hovered.bg_stroke  = Stroke::new(2.0, PLOT_CONFIG.color_help_bg_hover);
            visuals.widgets.active.bg_stroke   = Stroke::new(2.0, PLOT_CONFIG.color_help_bg_hover);

            // --- B. ICON (Text) COLOR ---
            visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, PLOT_CONFIG.color_help_bg);
            visuals.widgets.hovered.fg_stroke  = Stroke::new(1.0, PLOT_CONFIG.color_help_bg);
            visuals.widgets.active.fg_stroke   = Stroke::new(1.0, PLOT_CONFIG.color_help_bg_hover);

            // Prevent expansion animation if you want it rock solid
            visuals.widgets.hovered.expansion = 0.0;
            visuals.widgets.active.expansion = 0.0;

            // 2. Create Content WITHOUT explicit .color()
            // We rely on the 'visuals...fg_stroke' settings above to color it.
            let content = RichText::new(text).strong();

            // 3. Render
            let btn = Button::new(content)
                .fill(PLOT_CONFIG.color_help_fg)
                .corner_radius(CornerRadius::same(12))
                .min_size(Vec2::new(16.0, 16.0));

            ui.add(btn)
                .on_hover_cursor(CursorIcon::Help)
                .clicked()
        }).inner
    }

    fn label_subdued(&mut self, text: impl Into<String>) {
        self.label(RichText::new(text).small().color(Color32::GRAY));
    }

    fn metric(&mut self, label: &str, value: &str, color: Color32) {
        self.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0; // Tight spacing
            ui.label_subdued(format!("{}:", label));
            ui.label(RichText::new(value).small().color(color));
        });
    }

    fn label_header(&mut self, text: impl Into<String>) {
        let text = text.into().to_uppercase();
        self.heading(colored_heading(text));
    }

    fn label_subheader(&mut self, text: impl Into<String>) {
        self.label(colored_subsection_heading(text));
    }

    fn label_error(&mut self, text: impl Into<String>) {
        self.label(RichText::new(text).color(Color32::from_rgb(255, 100, 100)));
    }

    fn label_warning(&mut self, text: impl Into<String>) {
         self.label(RichText::new(text).small().color(Color32::from_rgb(255, 215, 0)));
    }

    fn button_text_primary(&self, text: impl Into<String>) -> RichText {
        RichText::new(text).strong().color(Color32::GREEN).small()
    }

    fn button_text_secondary(&self, text: impl Into<String>) -> RichText {
        RichText::new(text).strong().color(Color32::WHITE).small()
    }

}