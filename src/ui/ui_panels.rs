use eframe::egui::{
    Align2, Color32, ComboBox, Context, FontId, Grid, Rect, RichText, ScrollArea, Sense, Slider,
    Stroke, StrokeKind, Ui, Window, pos2, vec2, CursorIcon,
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

// --- HELPER STRUCT FOR LOGARITHMIC SLIDER ---
struct LogMapper {
    min_log: f64,
    log_range: f64,
    min_val: f64,
    max_val: f64,
}

impl LogMapper {
    fn new(min_val: f64, max_val: f64) -> Self {
        // Ensure min_val > 0.0 for log calculation to prevent NaN
        let safe_min = min_val.max(0.0001);
        let min_log = safe_min.ln();
        let max_log = max_val.ln();
        Self {
            min_log,
            log_range: max_log - min_log,
            min_val: safe_min,
            max_val,
        }
    }

    /// Map Value (Price %) -> Screen Fraction (0.0 to 1.0)
    fn value_to_frac(&self, val: f64) -> f64 {
        let val_clamped = val.clamp(self.min_val, self.max_val);
        (val_clamped.ln() - self.min_log) / self.log_range
    }

    /// Map Screen Fraction (0.0 to 1.0) -> Value (Price %)
    fn frac_to_value(&self, frac: f64) -> f64 {
        (self.min_log + (frac * self.log_range)).exp()
    }
}

/// Trait for UI panels that can be rendered
pub trait Panel {
    type Event;
    fn render(&mut self, ui: &mut Ui, show_help: &mut bool) -> Vec<Self::Event>;
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

    pub fn render_ph_help_window(ctx: &Context, open: &mut bool) {
        Window::new(UI_TEXT.ph_help_title)
            .open(open)
            .resizable(false)
            .collapsible(true)
            .show(ctx, |ui| {
                ui.set_max_width(500.0);

                // 1. Metrics Section
                ui.label(RichText::new(UI_TEXT.ph_help_metrics_title).strong());
                for (term, def) in UI_TEXT.ph_help_definitions {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("• {}:", term)).strong());
                        ui.label(*def);
                    });
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                // 2. Signal Quality Section (Colors)
                ui.label(RichText::new(UI_TEXT.ph_help_colors_title).strong());

                Grid::new("ph_colors_grid")
                    .striped(true)
                    .spacing([20.0, 8.0])
                    .show(ui, |ui| {
                        // Header
                        let (h1, h2, h3) = UI_TEXT.ph_help_colors_headers;
                        ui.label(RichText::new(h1).underline());
                        ui.label(RichText::new(h2).underline());
                        ui.label(RichText::new(h3).underline());
                        ui.end_row();

                        // Dynamic Rows from Config
                        let zones = AnalysisConfig::get_quality_zones();
                        let mut prev_max = 0;

                        for zone in zones {
                            // Column 1: Colored Name
                            let color = Color32::from_rgb(
                                zone.color_rgb.0,
                                zone.color_rgb.1,
                                zone.color_rgb.2,
                            );
                            ui.label(RichText::new(&zone.label).strong().color(color));

                            // Column 2: Range (Prev - Max)
                            let range_text = if zone.max_count == usize::MAX {
                                format!("> {}", prev_max)
                            } else if prev_max == 0 {
                                format!("< {}", zone.max_count)
                            } else {
                                format!("{} - {}", prev_max, zone.max_count)
                            };
                            ui.label(range_text);

                            // Column 3: Description
                            ui.label(&zone.description);

                            ui.end_row();

                            // Update tracker
                            prev_max = zone.max_count;
                        }
                    });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                // 3. Tuning Guide Section
                ui.label(RichText::new(UI_TEXT.ph_help_tuning_title).strong());

                Grid::new("ph_tuning_grid")
                    .striped(true)
                    .spacing([20.0, 8.0])
                    .show(ui, |ui| {
                        // Header
                        let (h1, h2, h3) = UI_TEXT.ph_help_table_headers;
                        ui.label(RichText::new(h1).underline());
                        ui.label(RichText::new(h2).underline());
                        ui.label(RichText::new(h3).underline());
                        ui.end_row();

                        // Rows
                        for (c1, c2, c3) in UI_TEXT.ph_help_table_rows {
                            ui.label(*c1);
                            ui.label(*c2);
                            ui.label(*c3);
                            ui.end_row();
                        }
                    });
            });
    }

    fn render_price_horizon_display(&mut self, ui: &mut Ui, show_help: &mut bool) -> Option<f64> {
        let mut changed = None;
        ui.add_space(5.0);

        // Header
        ui.horizontal(|ui| {
            ui.label(colored_subsection_heading(UI_TEXT.price_horizon_heading));
             // Chain .on_hover_cursor() before checking .clicked()
            if ui.button("(?)")
                .on_hover_cursor(CursorIcon::Help) 
                .clicked() 
            {
                *show_help = !*show_help;
            }
        });

        // 1. Setup Constants
        let min_pct = ANALYSIS.price_horizon.min_threshold_pct;
        let max_pct = ANALYSIS.price_horizon.max_threshold_pct;
        let mut current_pct = self.price_horizon_config.threshold_pct;

        // NEW: Initialize Log Mapper
        let mapper = LogMapper::new(min_pct, max_pct);

        // Fetch Colors
        let quality_zones = AnalysisConfig::get_quality_zones();
        let get_color = |count: usize| -> Color32 {
            for zone in &quality_zones {
                if count <= zone.max_count {
                    return Color32::from_rgb(zone.color_rgb.0, zone.color_rgb.1, zone.color_rgb.2);
                }
            }
            Color32::from_rgb(200, 50, 255)
        };

        if let Some(profile) = self.profile {
            // --- A. CUSTOM HEATMAP WIDGET ---
            let (rect, response) =
                ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click_and_drag());

            // Handle Input (Logarithmic)
            if response.dragged() || response.clicked() {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    let x_frac = ((pointer_pos.x - rect.min.x) / rect.width()) as f64;
                    // FIX: Use Mapper to convert Fraction -> Value
                    let new_val = mapper.frac_to_value(x_frac);
                    current_pct = new_val.clamp(min_pct, max_pct);
                    changed = Some(current_pct);
                }
            }

            // Draw Visuals
            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                painter.rect_filled(rect, 2.0, Color32::from_black_alpha(40));
                painter.rect_stroke(
                    rect,
                    2.0,
                    Stroke::new(1.0, Color32::from_gray(60)),
                    StrokeKind::Inside,
                );

                // Draw Buckets (Logarithmically Scaled)
                if !profile.buckets.is_empty() {
                    let count = profile.buckets.len();
                    for (i, bucket) in profile.buckets.iter().enumerate() {
                        let val_start = bucket.threshold_pct;
                        // Determine end value (next bucket or max)
                        let val_end = if i + 1 < count {
                            profile.buckets[i + 1].threshold_pct
                        } else {
                            max_pct
                        };

                        // FIX: Use Mapper to convert Value -> Fraction
                        let x_start_frac = mapper.value_to_frac(val_start);
                        let x_end_frac = mapper.value_to_frac(val_end);

                        let x_start_px = rect.min.x + (x_start_frac as f32 * rect.width());
                        let width_px = (x_end_frac - x_start_frac) as f32 * rect.width();

                        if width_px > 0.5 {
                            let bar_rect = Rect::from_min_size(
                                pos2(x_start_px, rect.min.y + 2.0),
                                vec2(width_px + 0.5, rect.height() - 4.0),
                            );
                            let color = get_color(bucket.candle_count);
                            painter.rect_filled(bar_rect, 0.0, color);
                        }
                    }
                }

                // Draw Handle (Logarithmic Position)
                // FIX: Use Mapper
                let handle_frac = mapper.value_to_frac(current_pct) as f32;
                let handle_x = rect.min.x + (handle_frac * rect.width());
                let handle_rect = Rect::from_center_size(
                    pos2(handle_x, rect.center().y),
                    vec2(4.0, rect.height()),
                );
                painter.rect_filled(handle_rect, 1.0, Color32::WHITE);
            }

            // --- THE REPORT (Feedback) ---
            // (This logic remains identical to your existing code, just copy it back in)
            self.render_horizon_report(ui, current_pct, profile, get_color);
        } else {
            // (Loading State remains identical)
            self.render_loading_state(ui);
        }

        changed
    }

    fn render_loading_state(&self, ui: &mut Ui) {
        let desired_size = vec2(ui.available_width(), 40.0);
        let (rect, _response) = ui.allocate_exact_size(desired_size, Sense::hover());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            painter.rect_filled(rect, 2.0, Color32::from_black_alpha(40));
            painter.rect_stroke(
                rect,
                2.0,
                Stroke::new(1.0, Color32::from_gray(60)),
                StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                UI_TEXT.ph_startup,
                FontId::proportional(12.0),
                Color32::GRAY,
            );
        }
    }

    fn render_horizon_report(
        &self,
        ui: &mut Ui,
        current_pct: f64,
        profile: &HorizonProfile,
        get_color: impl Fn(usize) -> Color32,
    ) {
        ui.vertical(|ui| {
            ui.add_space(4.0);

            ui.label(
                RichText::new(format!(
                    "{} {:.2}%",
                    UI_TEXT.ph_label_horizon_prefix,
                    current_pct * 100.0
                ))
                .strong(),
            );

            if let Some(bucket) = profile.buckets.iter().min_by(|a, b| {
                (a.threshold_pct - current_pct)
                    .abs()
                    .partial_cmp(&(b.threshold_pct - current_pct).abs())
                    .unwrap()
            }) {
                let is_current_config =
                    (current_pct - self.price_horizon_config.threshold_pct).abs() < f64::EPSILON;
                let count = if is_current_config {
                    self.actual_candle_count
                } else {
                    bucket.candle_count
                };

                let span_ms = bucket.max_ts.saturating_sub(bucket.min_ts);
                let span_days = span_ms as f64 / (1000.0 * 60.0 * 60.0 * 24.0);
                let density_pct = if span_days > 0.001 {
                    (bucket.duration_days / span_days) * 100.0
                } else {
                    0.0
                };
                let color = get_color(count);

                // Row A: Evidence
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
                // Row B: History
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
                    ui.label(RichText::new(format!("{:.1}%", density_pct)).color(density_color));
                });
            }
        });
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
    fn render(&mut self, ui: &mut Ui, show_help: &mut bool) -> Vec<Self::Event> {
        let mut events = Vec::new();
        section_heading(ui, UI_TEXT.data_generation_heading);

        // Price Horizon display (always enabled)
        if let Some(threshold) = self.render_price_horizon_display(ui, show_help) {
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
    fn render(&mut self, ui: &mut Ui, _show_help: &mut bool) -> Vec<Self::Event> {
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

    fn render(&mut self, ui: &mut Ui, _show_help: &mut bool) -> Vec<Self::Event> {
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
