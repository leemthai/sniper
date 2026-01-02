use eframe::egui::{
    Button, Color32, CornerRadius, CursorIcon, FontId, Response, RichText, Sense, Stroke,
    StrokeKind, Ui, Vec2, WidgetInfo, WidgetType, Id, Area, Order, Align2, Frame, Layout, Align, Key,
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

    // fn subtle_vertical_separator(&mut self);

    /// Renders a custom dropdown menu.
    /// - `id_salt`: Unique string for ID.
    /// - `label_text`: Text on the button.
    /// - `content`: Closure to render inside the popup. Should return `true` if we need to close the popup (e.g. selection made).
    fn custom_dropdown(&mut self, id_salt: &str, label_text: &str, content: impl FnOnce(&mut Ui) -> bool);


}

impl UiStyleExt for Ui {

        fn custom_dropdown(&mut self, id_salt: &str, label_text: &str, content: impl FnOnce(&mut Ui) -> bool) {
        let popup_id = self.make_persistent_id(id_salt);
        let global_state_id = Id::new("active_trade_variant_popup"); // Shared key to ensure only 1 opens at a time

        // 1. Draw Trigger Button
        // We use your 'interactive_label_small' style (size 10.0)
        // (Assuming you have this or use your standard interactive_label with font arg)
        let btn_response = self.interactive_label(
            label_text, 
            false, 
            PLOT_CONFIG.color_info,
            FontId::proportional(10.0)
        );

        // 2. Logic: Is this specific popup open?
        let is_open = self.data(|d| d.get_temp::<String>(global_state_id) == Some(id_salt.to_string()));

        if btn_response.clicked() {
            if is_open {
                // Close
                self.data_mut(|d| d.remove_temp::<String>(global_state_id));
            } else {
                // Open (Overwrites any other open popup)
                self.data_mut(|d| d.insert_temp(global_state_id, id_salt.to_string()));
            }
        }

        // 3. Render Popup if Open
        if is_open {
            let area = Area::new(popup_id)
                .order(Order::Tooltip)
                .pivot(Align2::RIGHT_TOP)
                .fixed_pos(btn_response.rect.right_bottom());

            let area_response = area.show(self.ctx(), |ui| {
                Frame::popup(ui.style())
                    .stroke(eframe::egui::Stroke::new(1.0, crate::config::plot::PLOT_CONFIG.color_widget_border))
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        // Formatting
                        ui.set_min_width(220.0);
                        ui.set_max_width(280.0);
                        ui.style_mut().interaction.selectable_labels = false; // No cursors in popup

                        // Header with Close Button
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&crate::ui::ui_text::UI_TEXT.label_risk_select).strong().small());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button(RichText::new(&crate::ui::ui_text::UI_TEXT.icon_close).size(10.0)).clicked() {
                                    ui.data_mut(|d| d.remove_temp::<String>(global_state_id));
                                }
                            });
                        });
                        ui.separator();

                        // Render Content
                        // If content returns true, it means "Close Me"
                        if content(ui) {
                            ui.data_mut(|d| d.remove_temp::<String>(global_state_id));
                        }
                    });
            });

            // 4. Click Outside Logic
            if self.input(|i| i.pointer.primary_clicked()) {
                // Did we click inside the popup?
                let in_popup = area_response.response.rect.contains(
                    self.input(|i| i.pointer.interact_pos().unwrap_or_default())
                );
                // Did we click the button that opened it? (If so, let button logic handle toggle)
                let on_button = btn_response.rect.contains(
                    self.input(|i| i.pointer.interact_pos().unwrap_or_default())
                );

                if !in_popup && !on_button {
                    self.data_mut(|d| d.remove_temp::<String>(global_state_id));
                }
            }

            // 5. Close on ESC
            if self.input(|i| i.key_pressed(Key::Escape)) {
                self.data_mut(|d| d.remove_temp::<String>(global_state_id));
            }
        }
    }


    // fn subtle_vertical_separator(&mut self) {
    //     let height = 12.0; // Fixed height for a clean look
    //     let width = 1.0;
    //     let color = PLOT_CONFIG.color_widget_border; // Use theme border color

    //     let (rect, _resp) = self.allocate_exact_size(Vec2::new(width + 8.0, height), Sense::hover()); // 4px padding on sides
        
    //     if self.is_rect_visible(rect) {
    //         let center_x = rect.center().x;
    //         self.painter().line_segment(
    //             [
    //                 Pos2::new(center_x, rect.top()),
    //                 Pos2::new(center_x, rect.bottom())
    //             ],
    //             Stroke::new(1.0, color)
    //         );
    //     }
    // }


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
