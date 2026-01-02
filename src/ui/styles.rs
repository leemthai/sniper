use eframe::egui::{
    Button, Color32, CornerRadius, CursorIcon, FontId, Response, RichText, Sense, Stroke,
    StrokeKind, Ui, Vec2, WidgetInfo, WidgetType, Pos2,
};

use crate::config::plot::PLOT_CONFIG;
use crate::models::trading_view::TradeDirection;
use crate::ui::config::UI_CONFIG;

/// Creates a colored heading with uppercase text (not mono anymore, but can put back for stylisting reasons if requried)
pub fn colored_heading(text: impl Into<String>) -> RichText {
    let uppercase_text = text.into().to_uppercase() + ":";
    RichText::new(uppercase_text).color(UI_CONFIG.colors.heading)
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
    /// A custom label that acts like a button:
    /// - Idle: Transparent BG, 'idle_color' text.
    /// - Hover: Gray BG, YELLOW text.
    /// - Selected: Blue BG, WHITE text.
    fn interactive_label(
        &mut self,
        text: &str,
        is_selected: bool,
        idle_color: Color32,
        font_id: FontId,
    ) -> Response;

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

    fn subtle_vertical_separator(&mut self);
}

impl UiStyleExt for Ui {

    fn subtle_vertical_separator(&mut self) {
        let height = 12.0; // Fixed height for a clean look
        let width = 1.0;
        let color = PLOT_CONFIG.color_widget_border; // Use theme border color

        let (rect, _resp) = self.allocate_exact_size(Vec2::new(width + 8.0, height), Sense::hover()); // 4px padding on sides
        
        if self.is_rect_visible(rect) {
            let center_x = rect.center().x;
            self.painter().line_segment(
                [
                    Pos2::new(center_x, rect.top()),
                    Pos2::new(center_x, rect.bottom())
                ],
                Stroke::new(1.0, color)
            );
        }
    }


    fn interactive_label(
        &mut self,
        text: &str,
        is_selected: bool,
        idle_color: Color32,
        font_id: FontId,
    ) -> Response {
        // REMOVED hardcoded 14.0
        let padding = eframe::egui::Vec2::new(4.0, 2.0);

        // 1. Calculate Size
        let galley = self
            .painter()
            .layout_no_wrap(text.to_string(), font_id, idle_color);
        let desired_size = galley.size() + padding * 2.0;

        // 2. Allocate
        let (rect, response) = self.allocate_exact_size(desired_size, Sense::click());
        response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, is_selected, text));

        if self.is_rect_visible(rect) {
            let visuals = self.style().visuals.clone();

            let (bg_fill, text_color) = if is_selected {
                (visuals.selection.bg_fill, Color32::WHITE)
            } else if response.hovered() || response.has_focus() {
                (visuals.widgets.hovered.bg_fill, Color32::YELLOW)
            } else {
                (Color32::TRANSPARENT, idle_color)
            };

            if is_selected || response.hovered() {
                self.painter().rect(
                    rect,
                    CornerRadius::same(4),
                    bg_fill,
                    Stroke::NONE,
                    StrokeKind::Inside,
                );
            }

            let text_pos = rect.left_top() + padding;
            self.painter().galley(text_pos, galley, text_color);
        }

        response
    }

    fn help_button(&mut self, text: &str) -> bool {
        // 1. Use a Scope to modify styles for just this button
        self.scope(|ui| {
            let visuals = ui.visuals_mut();

            // --- A. BORDER (BG Stroke) COLOR ---
            visuals.widgets.inactive.bg_stroke = Stroke::new(2.0, PLOT_CONFIG.color_help_bg);
            visuals.widgets.hovered.bg_stroke = Stroke::new(2.0, PLOT_CONFIG.color_help_bg_hover);
            visuals.widgets.active.bg_stroke = Stroke::new(2.0, PLOT_CONFIG.color_help_bg_hover);

            // --- B. ICON (Text) COLOR ---
            visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, PLOT_CONFIG.color_help_bg);
            visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, PLOT_CONFIG.color_help_bg);
            visuals.widgets.active.fg_stroke = Stroke::new(1.0, PLOT_CONFIG.color_help_bg_hover);

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

            ui.add(btn).on_hover_cursor(CursorIcon::Help).clicked()
        })
        .inner
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
        self.label(
            RichText::new(text)
                .small()
                .color(Color32::from_rgb(255, 215, 0)),
        );
    }

    fn button_text_primary(&self, text: impl Into<String>) -> RichText {
        RichText::new(text).strong().color(Color32::GREEN).small()
    }

    fn button_text_secondary(&self, text: impl Into<String>) -> RichText {
        RichText::new(text).strong().color(Color32::WHITE).small()
    }
}
