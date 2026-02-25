use {
    crate::{
        config::PLOT_CONFIG,
        models::TradeDirection,
        ui::{UI_CONFIG, UI_TEXT},
    },
    eframe::egui::{
        Align, Align2, Area, Color32, CornerRadius, FontId, Frame, Id, Key, Layout, Order,
        Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, WidgetInfo, WidgetType,
    },
};

pub(crate) fn colored_subsection_heading(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(UI_CONFIG.colors.subsection_heading)
}

pub trait DirectionColor {
    fn color(&self) -> Color32;
}

impl DirectionColor for TradeDirection {
    fn color(&self) -> Color32 {
        match self {
            Self::Long => PLOT_CONFIG.color_long,
            Self::Short => PLOT_CONFIG.color_short,
        }
    }
}

pub fn apply_opacity(color: Color32, factor: f32) -> Color32 {
    color.linear_multiply(factor)
}

pub fn get_outcome_color(value: f64) -> Color32 {
    if value > 0.0 {
        PLOT_CONFIG.color_profit
    } else {
        PLOT_CONFIG.color_loss
    }
}

pub fn get_momentum_color(value: f64) -> Color32 {
    if value > 0.0 {
        PLOT_CONFIG.color_long
    } else if value < 0.0 {
        PLOT_CONFIG.color_short
    } else {
        PLOT_CONFIG.color_text_subdued
    }
}

pub(crate) trait UiStyleExt {
    /// Interactive label acting as button: transparent when idle, gray bg on hover, blue bg when selected.
    fn interactive_label(
        &mut self,
        text: &str,
        is_selected: bool,
        idle_color: Color32,
        font_id: FontId,
    ) -> Response;

    fn label_subdued(&mut self, text: impl Into<String>);
    fn metric(&mut self, label: &str, value: &str, color: Color32);
    fn label_subheader(&mut self, text: impl Into<String>);
    fn button_text_primary(&self, text: impl Into<String>) -> RichText;
    fn button_text_secondary(&self, text: impl Into<String>) -> RichText;
    fn custom_dropdown(
        &mut self,
        id_salt: &str,
        label_text: &str,
        content: impl FnOnce(&mut Ui) -> bool,
    );
}

impl UiStyleExt for Ui {
    fn custom_dropdown(
        &mut self,
        id_salt: &str,
        label_text: &str,
        content: impl FnOnce(&mut Ui) -> bool,
    ) {
        let popup_id = self.make_persistent_id(id_salt);
        let global_state_id = Id::new("active_trade_variant_popup"); // Shared key to ensure only 1 opens at a time
        let btn_response = self.interactive_label(
            label_text,
            false,
            PLOT_CONFIG.color_info,
            FontId::proportional(10.0),
        );
        let is_open =
            self.data(|d| d.get_temp::<String>(global_state_id) == Some(id_salt.to_string()));

        if btn_response.clicked() {
            if is_open {
                self.data_mut(|d| d.remove_temp::<String>(global_state_id));
            } else {
                self.data_mut(|d| d.insert_temp(global_state_id, id_salt.to_string()));
            }
        }

        if is_open {
            let area = Area::new(popup_id)
                .order(Order::Tooltip)
                .pivot(Align2::RIGHT_TOP)
                .fixed_pos(btn_response.rect.right_bottom());

            let area_response = area.show(self.ctx(), |ui| {
                Frame::popup(ui.style())
                    .stroke(Stroke::new(1.0, PLOT_CONFIG.color_widget_border))
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.set_min_width(220.0);
                        ui.set_max_width(280.0);
                        ui.style_mut().interaction.selectable_labels = false; // No cursors in popup

                        // Header with Close Button
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&UI_TEXT.label_risk_select).strong().small());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .button(RichText::new(&UI_TEXT.icon_close).size(10.0))
                                    .clicked()
                                {
                                    ui.data_mut(|d| d.remove_temp::<String>(global_state_id));
                                }
                            });
                        });
                        ui.separator();
                        if content(ui) {
                            ui.data_mut(|d| d.remove_temp::<String>(global_state_id));
                        }
                    });
            });

            if self.input(|i| i.pointer.primary_clicked()) {
                let in_popup = area_response
                    .response
                    .rect
                    .contains(self.input(|i| i.pointer.interact_pos().unwrap_or_default()));
                let on_button = btn_response
                    .rect
                    .contains(self.input(|i| i.pointer.interact_pos().unwrap_or_default()));
                if !in_popup && !on_button {
                    self.data_mut(|d| d.remove_temp::<String>(global_state_id));
                }
            }
            if self.input(|i| i.key_pressed(Key::Escape)) {
                self.data_mut(|d| d.remove_temp::<String>(global_state_id));
            }
        }
    }

    fn interactive_label(
        &mut self,
        text: &str,
        is_selected: bool,
        idle_color: Color32,
        font_id: FontId,
    ) -> Response {
        let padding = Vec2::new(4.0, 4.0);
        let galley = self
            .painter()
            .layout_no_wrap(text.to_string(), font_id, idle_color);
        let desired_size = galley.size() + padding * 2.0;
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

    fn label_subheader(&mut self, text: impl Into<String>) {
        self.label(colored_subsection_heading(text));
    }

    fn button_text_primary(&self, text: impl Into<String>) -> RichText {
        RichText::new(text).strong().color(Color32::GREEN).small()
    }

    fn button_text_secondary(&self, text: impl Into<String>) -> RichText {
        RichText::new(text).strong().color(Color32::WHITE).small()
    }
}
