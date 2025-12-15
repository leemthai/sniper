use eframe::egui::{
    Align2, Color32, ComboBox, FontId, Rect, RichText, ScrollArea, Sense, Slider, Stroke,
    StrokeKind, Ui, pos2, vec2,
};
use strum::IntoEnumIterator;

use crate::config::ANALYSIS;
use crate::config::plot::PLOT_CONFIG;
use crate::config::{AnalysisConfig, PriceHorizonConfig};

use crate::domain::pair_interval::PairInterval;

use crate::models::cva::ScoreType;
use crate::models::horizon_profile::HorizonProfile;
use crate::models::{PairContext, ZoneType};

use crate::ui::config::UI_TEXT;
use crate::ui::utils::{
    colored_subsection_heading, format_candle_count, format_duration_context, section_heading,
    spaced_separator,
};

#[cfg(debug_assertions)]
use crate::config::DEBUG_FLAGS;

/// Trait for UI panels that can be rendered
pub trait Panel {
    type Event;
    fn render(&mut self, ui: &mut Ui) -> Vec<Self::Event>;
}

/// Panel for data generation options
pub struct DataGenerationPanel<'a> {
    #[allow(dead_code)]
    zone_count: usize,
    selected_pair: Option<String>,
    available_pairs: Vec<String>,
    price_horizon_config: &'a PriceHorizonConfig,
    time_horizon_days: u64,
    profile: Option<&'a HorizonProfile>,
    actual_candle_count: usize,
}

impl<'a> DataGenerationPanel<'a> {
    pub fn new(
        zone_count: usize,
        selected_pair: Option<String>,
        available_pairs: Vec<String>,
        price_horizon_config: &'a PriceHorizonConfig,
        time_horizon_days: u64,
        profile: Option<&'a HorizonProfile>,
        actual_candle_count: usize,
    ) -> Self {
        Self {
            zone_count,
            selected_pair,
            available_pairs,
            price_horizon_config,
            time_horizon_days,
            profile,
            actual_candle_count,
        }
    }

    fn render_price_horizon_display(&mut self, ui: &mut Ui) -> Option<f64> {
        let mut changed = None;
        ui.add_space(5.0);
        ui.label(colored_subsection_heading(UI_TEXT.price_horizon_heading));

        // 1. Setup Constants & Current Value
        let min_pct = ANALYSIS.price_horizon.min_threshold_pct; // 0.01
        let max_pct = ANALYSIS.price_horizon.max_threshold_pct; // 0.80
        let range = max_pct - min_pct;
        let mut current_pct = self.price_horizon_config.threshold_pct;

        // Fetch Data-Driven Color Scale
        let quality_zones = AnalysisConfig::get_quality_zones();
        let get_color = |count: usize| -> Color32 {
            for zone in &quality_zones {
                if count <= zone.max_count {
                    return Color32::from_rgb(zone.color_rgb.0, zone.color_rgb.1, zone.color_rgb.2);
                }
            }
            Color32::from_rgb(200, 50, 255) // Fallback (Ultra)
        };

        // 2. Decide: Custom Heatmap OR Standard Slider?
        if let Some(profile) = self.profile {
            // --- A. CUSTOM HEATMAP WIDGET ---

            // Allocate 40px height for the bar
            let (rect, response) =
                ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click_and_drag());

            // Handle Input (Click or Drag)
            if response.dragged() || response.clicked() {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    // Map Mouse X -> Percentage
                    let x_frac = ((pointer_pos.x - rect.min.x) / rect.width()) as f64;
                    let new_val = min_pct + (x_frac * range);
                    current_pct = new_val.clamp(min_pct, max_pct);
                    changed = Some(current_pct);
                }
            }

            // Draw Visuals
            if ui.is_rect_visible(rect) {
                let painter = ui.painter();

                // Draw Track Background
                painter.rect_filled(rect, 2.0, Color32::from_black_alpha(40));
                // FIX 2: Add StrokeKind::Inside as 4th argument
                painter.rect_stroke(
                    rect,
                    2.0,
                    Stroke::new(1.0, Color32::from_gray(60)),
                    StrokeKind::Inside,
                );

                // Draw Data Buckets (The Colored Slices)
                // We calculate the X position for each bucket based on its threshold %
                if !profile.buckets.is_empty() {
                    let width_per_step = rect.width() / profile.buckets.len() as f32;
                    for bucket in &profile.buckets {
                        let frac = (bucket.threshold_pct - min_pct) / range;
                        let x_start = rect.min.x + (frac as f32 * rect.width());

                        let bar_rect = Rect::from_min_size(
                            pos2(x_start, rect.min.y + 2.0),
                            vec2(width_per_step + 1.0, rect.height() - 4.0),
                        );

                        let color = get_color(bucket.candle_count);
                        painter.rect_filled(bar_rect, 0.0, color);
                    }
                }

                // Draw Handle (White Line at current val)
                let handle_frac = (current_pct - min_pct) / range;
                let handle_x = rect.min.x + (handle_frac as f32 * rect.width());
                let handle_rect = Rect::from_center_size(
                    pos2(handle_x, rect.center().y),
                    vec2(4.0, rect.height()),
                );
                painter.rect_filled(handle_rect, 1.0, Color32::WHITE);
            }

            // --- THE REPORT (Feedback beneath slider) ---
            ui.vertical(|ui| {
                ui.add_space(4.0);

                // 1. Horizon Setting
                ui.label(RichText::new(format!("Horizon: {:.2}%", current_pct * 100.0)).strong());

                // Match Bucket
                if let Some(bucket) = profile.buckets.iter().min_by(|a, b| {
                    (a.threshold_pct - current_pct)
                        .abs()
                        .partial_cmp(&(b.threshold_pct - current_pct).abs())
                        .unwrap()
                }) {
                    let is_current_config = (current_pct - self.price_horizon_config.threshold_pct)
                        .abs()
                        < f64::EPSILON;

                    // Display Logic: Use Actual count if static, Bucket if dragging
                    let count = if is_current_config {
                        self.actual_candle_count
                    } else {
                        bucket.candle_count
                    };

                    // Calc History & Density
                    let span_ms = bucket.max_ts.saturating_sub(bucket.min_ts);
                    let span_days = span_ms as f64 / (1000.0 * 60.0 * 60.0 * 24.0);

                    let density_pct = if span_days > 0.001 {
                        (bucket.duration_days / span_days) * 100.0
                    } else {
                        0.0
                    };

                    let color = get_color(count);

                    // Row A: Evidence (Active)
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{}:", UI_TEXT.ph_label_evidence))
                                .small()
                                .color(Color32::GRAY),
                        );
                        ui.label(
                            RichText::new(format!(
                                "{} ({})",
                                format_duration_context(bucket.duration_days),
                                format_candle_count(count)
                            ))
                            .color(color),
                        );
                    });

                    // Row B: History (Span)
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{}:", UI_TEXT.ph_label_history))
                                .small()
                                .color(Color32::GRAY),
                        );
                        ui.label(
                            RichText::new(format_duration_context(span_days))
                                .color(Color32::LIGHT_BLUE),
                        );
                    });

                    // Row C: Density
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{}:", UI_TEXT.ph_label_density))
                                .small()
                                .color(Color32::GRAY),
                        );

                        let density_color = if density_pct > 50.0 {
                            Color32::GREEN
                        } else if density_pct > 10.0 {
                            Color32::YELLOW
                        } else {
                            Color32::LIGHT_RED
                        };

                        ui.label(
                            RichText::new(format!("{:.1}%", density_pct)).color(density_color),
                        );
                    });
                }
            });
        } else {
            // --- B. FALLBACK: LOADING STATE ---
            // Draw the same shape as the custom widget, but grayed out.
            // This prevents the UI popping/shifting during initialization.

            let desired_size = vec2(ui.available_width(), 40.0);
            let (rect, _response) = ui.allocate_exact_size(desired_size, Sense::hover());

            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                // Draw Empty Track
                painter.rect_filled(rect, 2.0, Color32::from_black_alpha(40));
                painter.rect_stroke(
                    rect,
                    2.0,
                    Stroke::new(1.0, Color32::from_gray(60)),
                    StrokeKind::Inside,
                );

                // Draw Loading Text
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    UI_TEXT.ph_startup,
                    FontId::proportional(12.0),
                    Color32::GRAY,
                );
            }
        }

        changed
    }

    fn render_time_horizon_slider(&mut self, ui: &mut Ui) -> Option<u64> {
        let mut changed = None;

        ui.add_space(5.0);
        ui.label(colored_subsection_heading(UI_TEXT.time_horizon_heading));

        let mut horizon_days = self.time_horizon_days as f64;
        let response = ui.add(
            Slider::new(
                &mut horizon_days,
                ANALYSIS.time_horizon.min_days as f64..=ANALYSIS.time_horizon.max_days as f64,
            )
            .integer()
            .suffix(" days"),
        );

        let new_value = horizon_days.round() as u64;
        self.time_horizon_days = new_value;

        if response.changed() {
            changed = Some(new_value);
        }

        let helper_text = format!(
            "{}{}{}",
            UI_TEXT.time_horizon_helper_prefix, new_value, UI_TEXT.time_horizon_helper_suffix
        );
        ui.label(RichText::new(helper_text).small().color(Color32::GRAY));

        changed
    }

    fn render_pair_selector(&mut self, ui: &mut Ui) -> Option<String> {
        let mut changed = None;
        let previously_selected_pair = self.selected_pair.clone();

        ui.label(colored_subsection_heading(UI_TEXT.pair_selector_heading));
        ScrollArea::vertical()
            .max_height(160.)
            .id_salt("pair_selector")
            .show(ui, |ui| {
                for item in &self.available_pairs {
                    let is_selected = self.selected_pair.as_ref() == Some(item);
                    if ui.selectable_label(is_selected, item).clicked() {
                        self.selected_pair = Some(item.clone());
                        changed = Some(item.clone());
                    }
                }
            });

        // Defensive check: catch changes even if .clicked() didn't fire
        if self.selected_pair != previously_selected_pair {
            changed = self.selected_pair.clone();
            #[cfg(debug_assertions)]
            if DEBUG_FLAGS.print_ui_interactions {
                log::info!("A new pair was selected: {:?}", self.selected_pair);
            }
        }

        changed
    }
}

#[derive(Debug)]
pub enum DataGenerationEventChanged {
    // ZoneCount(usize),
    Pair(String),
    PriceHorizonThreshold(f64),
    TimeHorizonDays(u64),
}

impl<'a> Panel for DataGenerationPanel<'a> {
    type Event = DataGenerationEventChanged;
    fn render(&mut self, ui: &mut Ui) -> Vec<Self::Event> {
        let mut events = Vec::new();
        section_heading(ui, UI_TEXT.data_generation_heading);

        // Price Horizon display (always enabled)
        if let Some(threshold) = self.render_price_horizon_display(ui) {
            events.push(DataGenerationEventChanged::PriceHorizonThreshold(threshold));
        }
        spaced_separator(ui);

        if let Some(days) = self.render_time_horizon_slider(ui) {
            events.push(DataGenerationEventChanged::TimeHorizonDays(days));
        }
        spaced_separator(ui);

        if let Some(pair) = self.render_pair_selector(ui) {
            events.push(DataGenerationEventChanged::Pair(pair));
        }
        if let Some(pair) = &self.selected_pair {
            ui.label(format!(
                "Selected: {:?}",
                PairInterval::split_pair_name(pair)
            ));
        }
        ui.add_space(20.0);
        events
    }
}

/// Panel for view options
pub struct ViewPanel {
    selected_score_type: ScoreType,
}

impl ViewPanel {
    pub fn new(score_type: ScoreType) -> Self {
        Self {
            selected_score_type: score_type,
        }
    }
}

impl Panel for ViewPanel {
    type Event = ScoreType;
    fn render(&mut self, ui: &mut Ui) -> Vec<Self::Event> {
        let mut events = Vec::new();
        section_heading(ui, UI_TEXT.view_options_heading);

        ui.label(colored_subsection_heading(UI_TEXT.view_data_source_heading));
        ComboBox::from_id_salt("Score Type")
            .selected_text(self.selected_score_type.to_string())
            .show_ui(ui, |ui| {
                for score_type_variant in ScoreType::iter() {
                    if ui
                        .selectable_value(
                            &mut self.selected_score_type,
                            score_type_variant,
                            score_type_variant.to_string(),
                        )
                        .clicked()
                    {
                        events.push(self.selected_score_type);
                    }
                }
            });

        ui.add_space(20.0);
        events
    }
}

/// Panel showing trading opportunities across all monitored pairs
pub struct SignalsPanel<'a> {
    signals: Vec<&'a PairContext>,
}

impl<'a> SignalsPanel<'a> {
    pub fn new(signals: Vec<&'a PairContext>) -> Self {
        Self { signals }
    }
}

impl<'a> Panel for SignalsPanel<'a> {
    type Event = String; // Returns pair name if clicked

    fn render(&mut self, ui: &mut Ui) -> Vec<Self::Event> {
        let mut events = Vec::new();
        section_heading(ui, UI_TEXT.signals_heading);

        if self.signals.is_empty() {
            ui.label(
                RichText::new("No high-interest signals")
                    .small()
                    .color(Color32::GRAY),
            );
        } else {
            ui.label(
                RichText::new(format!("{} active", self.signals.len()))
                    .small()
                    .color(Color32::from_rgb(100, 200, 255)),
            );
            ui.add_space(5.0);

            for opp in &self.signals {
                ui.group(|ui| {
                    // Pair name as clickable button
                    let pair_label = format!("📌 {}", opp.pair_name);
                    if ui.button(pair_label).clicked() {
                        events.push(opp.pair_name.clone());
                    }

                    // Current zone types (as lng as it is sticky)
                    for (zone_index, zone_type) in &opp.current_zones {
                        let zone_label = match zone_type {
                            ZoneType::Sticky => Some((
                                format!("🔑 Sticky superzone {}", zone_index),
                                PLOT_CONFIG.sticky_zone_color,
                            )),
                            _ => None,
                        };

                        if let Some((text, color)) = zone_label {
                            ui.label(RichText::new(text).small().color(color));
                        }
                    }
                });
                ui.add_space(3.0);
            }
        }
        ui.add_space(10.0);
        events
    }
}
