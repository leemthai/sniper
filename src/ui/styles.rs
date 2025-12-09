use eframe::egui::{Color32, RichText, Ui};
use crate::ui::config::UI_CONFIG;

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
}

impl UiStyleExt for Ui {
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
        self.heading(RichText::new(text).color(UI_CONFIG.colors.heading).monospace());
    }

    fn label_subheader(&mut self, text: impl Into<String>) {
        self.label(RichText::new(text).color(UI_CONFIG.colors.subsection_heading));
    }

    fn label_error(&mut self, text: impl Into<String>) {
        self.label(RichText::new(text).color(Color32::from_rgb(255, 100, 100)));
    }

    fn label_warning(&mut self, text: impl Into<String>) {
         self.label(RichText::new(text).small().color(Color32::from_rgb(255, 215, 0)));
    }
}