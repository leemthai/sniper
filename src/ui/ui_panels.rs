use colorgrad::Gradient;
use eframe::egui::{
    Align, Align2, Button, Color32, Context, FontId, Grid, Key, Layout, Rect,
    RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, Window, pos2, vec2, Order,
};

use crate::analysis::range_gap_finder::{DisplaySegment, GapReason};

use crate::config::plot::PLOT_CONFIG;
use crate::config::{ANALYSIS, PriceHorizonConfig};

use crate::domain::pair_interval::PairInterval;

use crate::models::horizon_profile::HorizonProfile;

use crate::ui::config::UI_TEXT;
use crate::ui::styles::{UiStyleExt,section_heading, spaced_separator, colored_subsection_heading};

use crate::ui::utils::{
    format_candle_count, format_duration_context,
};

use crate::utils::TimeUtils;

#[cfg(debug_assertions)]
use crate::config::DEBUG_FLAGS;

pub struct CandleRangePanel<'a> {
    segments: &'a [DisplaySegment],
    current_range_idx: Option<usize>,
}

impl<'a> CandleRangePanel<'a> {
    pub fn new(segments: &'a [DisplaySegment], current_idx: Option<usize>) -> Self {
        Self {
            segments,
            current_range_idx: current_idx,
        }
    }

    pub fn render(&mut self, ui: &mut Ui, last_viewed_idx: usize) -> Option<Option<usize>> {
        let mut action = None;

        ui.add_space(5.0);
        // ui.heading(format!("{}", UI_TEXT.cr_title));
        // ui.separator();
        ui.label_subheader(format!(
            "{} {} {}",
            self.segments.len(),
            UI_TEXT.cr_title_1,
            UI_TEXT.cr_title_2
        ));

        ui.horizontal(|ui| {
            // PREV BUTTON
            let prev_enabled = self.current_range_idx.map_or(false, |i| i > 0);
            if ui.add_enabled(prev_enabled, Button::new("⬅")).clicked() {
                if let Some(curr) = self.current_range_idx {
                    action = Some(Some(curr - 1));
                }
            }

            // TOGGLE BUTTON (Middle)
            let is_viewing_all = self.current_range_idx.is_none();
            let (btn_label, target_idx) = if is_viewing_all {
                let safe_target = last_viewed_idx.min(self.segments.len().saturating_sub(1));
                let is_live = safe_target == self.segments.len().saturating_sub(1);

                let text = if is_live {
                    UI_TEXT.cr_nav_return_live.to_string()
                } else {
                    format!("{} {}", UI_TEXT.cr_nav_return_prefix, safe_target + 1)
                };
                (ui.button_text_secondary(text), Some(safe_target))
            } else {
                (ui.button_text_primary(&UI_TEXT.cr_nav_show_all), None)
            };

            if ui.button(btn_label).clicked() {
                action = Some(target_idx);
            }

            // NEXT BUTTON
            let next_enabled = self
                .current_range_idx
                .map_or(false, |i| i < self.segments.len() - 1);
            if ui
                .add_enabled(next_enabled, Button::new("➡"))
                .clicked()
            {
                if let Some(curr) = self.current_range_idx {
                    action = Some(Some(curr + 1));
                } else {
                    action = Some(Some(self.segments.len() - 1));
                }
            }
        });

        ui.separator();

        // --- COMPACT LIST ---
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                eframe::egui::Grid::new("cr_grid")
                    .striped(true)
                    .num_columns(2) // Reduced to 2
                    .spacing([10.0, 8.0]) // Tighter spacing
                    .show(ui, |ui| {
                        // Compact Headers
                        ui.label(RichText::new("Date Range").strong().small());
                        ui.label(RichText::new("Context").strong().small());
                        ui.end_row();

                        for (i, seg) in self.segments.iter().enumerate().rev() {
                            let is_selected = self.current_range_idx == Some(i);

                            // GAP ROW
                            if i > 0 {
                                // Merged Gap Info (Duration + Reason)
                                let gap_text = format!(
                                    "-- {} Gap ({}) --",
                                    seg.gap_duration_str,
                                    match seg.gap_reason {
                                        GapReason::PriceMismatch => "Price",
                                        GapReason::MissingSourceData => "Missing",
                                        GapReason::PriceAbovePH => "High",
                                        GapReason::PriceBelowPH => "Low",
                                        _ => "Mixed",
                                    }
                                );

                                // Use Semantic Colors for Gaps
                                let gap_color = match seg.gap_reason {
                                    GapReason::MissingSourceData => PLOT_CONFIG.color_gap_missing,
                                    GapReason::PriceAbovePH => PLOT_CONFIG.color_gap_above,
                                    GapReason::PriceBelowPH => PLOT_CONFIG.color_gap_below,
                                    _ => PLOT_CONFIG.color_text_subdued,
                                };

                                ui.label(
                                    RichText::new(gap_text).italics().small().color(gap_color),
                                );
                                ui.label(""); // Empty context column for gap
                                ui.end_row();
                            }

                            // SEGMENT ROW
                            let start_date = TimeUtils::epoch_ms_to_date_string(seg.start_ts);
                            let end_date = TimeUtils::epoch_ms_to_date_string(seg.end_ts);

                            // Column 1: Date Range + Count (Clickable)
                            // Format: "2024-01-01 - 2024-02-01 (500c)"
                            let label_text =
                                format!("{} - {} ({}c)", start_date, end_date, seg.candle_count);
                            if ui
                                .selectable_label(is_selected, RichText::new(label_text).small())
                                .clicked()
                            {
                                action = Some(Some(i));
                            }

                            // Column 2: Context
                            if i == self.segments.len() - 1 {
                                ui.label(
                                    RichText::new(&UI_TEXT.cr_label_live)
                                        .color(PLOT_CONFIG.color_profit)
                                        .strong()
                                        .small(),
                                );
                            } else {
                                ui.label(
                                    RichText::new(&UI_TEXT.cr_label_historical)
                                        .small()
                                        .color(PLOT_CONFIG.color_text_subdued),
                                );
                            }
                            ui.end_row();
                        }
                    });
            });

        action
    }
}

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
    // #[allow(dead_code)]
    // zone_count: usize,
    selected_pair: Option<String>,
    available_pairs: Vec<String>,
    price_horizon_config: &'a PriceHorizonConfig,
    profile: Option<&'a HorizonProfile>,
    actual_candle_count: usize,
    interval_ms: i64,
    pub scroll_to_pair_requested: &'a mut Option<String>,
}

impl<'a> DataGenerationPanel<'a> {
    pub fn new(
        selected_pair: Option<String>,
        available_pairs: Vec<String>,
        price_horizon_config: &'a PriceHorizonConfig,
        profile: Option<&'a HorizonProfile>,
        actual_candle_count: usize,
        interval_ms: i64,
        scroll_to_pair_requested: &'a mut Option<String>,
    ) -> Self {
        Self {
            selected_pair,
            available_pairs,
            price_horizon_config,
            profile,
            actual_candle_count,
            interval_ms,
            scroll_to_pair_requested,
        }
    }

    pub fn render_ph_help_window(ctx: &Context, open: &mut bool) {
        Window::new(&UI_TEXT.ph_help_title)
            .open(open)
            .resizable(false)
            .order(Order::Tooltip)
            .collapsible(true)
            .show(ctx, |ui| {
                ui.set_max_width(600.0);

                // 1. METRICS DEFINITIONS
                ui.label(RichText::new("Definitions").strong());
                for (term, def) in UI_TEXT.ph_help_definitions {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("• {}:", term)).strong());
                        ui.label(*def);
                    });
                }
                ui.add_space(10.0);
                ui.separator();

                // 2. HEATMAP LEGEND (Colors)
                ui.label(RichText::new("1. Reading the Heatmap (Data Density)").strong());
                Grid::new("ph_help_density")
                    .striped(true)
                    .spacing([20.0, 8.0])
                    .show(ui, |ui| {
                        let (h1, h2, h3) = UI_TEXT.ph_help_density_header;
                        ui.label(RichText::new(h1).underline());
                        ui.label(RichText::new(h2).underline());
                        ui.label(RichText::new(h3).underline());
                        ui.end_row();

                        for (col_name, density, sig) in UI_TEXT.ph_help_density_rows {
                            // Manual coloring for the help text to match the gradient approx
                            let color = match *col_name {
                                "Deep Purple" => PLOT_CONFIG.color_heatmap_low,
                                "Orange/Red" => PLOT_CONFIG.color_heatmap_med,
                                "Bright Yellow" => PLOT_CONFIG.color_heatmap_high,
                                _ => PLOT_CONFIG.color_text_subdued, // Gray fallback
                            };

                            ui.label(RichText::new(*col_name).color(color));
                            ui.label(*density);
                            ui.label(*sig);
                            ui.end_row();
                        }
                    });

                ui.add_space(10.0);
                ui.separator();
                // 3. SCOPE LEGEND (Trade Styles)
                ui.label(RichText::new("2. Selecting your Scope (Trade Style)").strong());
                Grid::new("ph_help_scope")
                    .striped(true)
                    .spacing([20.0, 8.0])
                    .show(ui, |ui| {
                        let (h1, h2, h3) = UI_TEXT.ph_help_scope_header;
                        ui.label(RichText::new(h1).underline());
                        ui.label(RichText::new(h2).underline());
                        ui.label(RichText::new(h3).underline());
                        ui.end_row();

                        for (range, style, focus) in UI_TEXT.ph_help_scope_rows {
                            ui.label(*range); // <--- Added *
                            ui.label(RichText::new(*style).strong().color(PLOT_CONFIG.color_info));
                            ui.label(*focus);
                            ui.end_row();
                        }
                    });
            });
    }

    fn render_price_horizon_display(&mut self, ui: &mut Ui, show_help: &mut bool) -> Option<f64> {
        // 1. Setup Constants
        let min_pct = ANALYSIS.price_horizon.min_threshold_pct;
        let max_pct = ANALYSIS.price_horizon.max_threshold_pct;
        let mut current_pct = self.price_horizon_config.threshold_pct;
        let mut changed = None;

        ui.add_space(5.0);

        ui.horizontal(|ui| {
            ui.label(colored_subsection_heading(&UI_TEXT.price_horizon_heading));

            if ui.help_button(&UI_TEXT.icon_help) {
                *show_help = !*show_help;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label("%");

                let id = ui.make_persistent_id("ph_input_box");
                let has_focus = ui.memory(|m| m.has_focus(id));

                // LOGIC:
                // 1. If Focused: Read from Temp (User is typing)
                // 2. If Not Focused: Read from Variable (Source of Truth) AND update Temp (Sync)
                let mut text_buf = if has_focus {
                    ui.data(|d| d.get_temp(id))
                        .unwrap_or_else(|| format!("{:.3}", current_pct * 100.0))
                } else {
                    let s = format!("{:.3}", current_pct * 100.0);
                    // Keep temp synchronized so it's ready the moment we click
                    ui.data_mut(|d| d.insert_temp(id, s.clone()));
                    s
                };

                let response = ui.add(
                    TextEdit::singleline(&mut text_buf)
                        .id(id)
                        .desired_width(50.0)
                        .horizontal_align(Align::RIGHT),
                );

                // Save changes while typing
                if response.changed() {
                    ui.data_mut(|d| d.insert_temp(id, text_buf.clone()));
                }

                // Commit changes
                if response.lost_focus()
                    || (response.changed() && ui.input(|i| i.key_pressed(Key::Enter)))
                {
                    if let Ok(val) = text_buf.parse::<f64>() {
                        let val_clamped = val.clamp(min_pct * 100.0, max_pct * 100.0);
                        let new_val = val_clamped / 100.0;

                        current_pct = new_val;
                        changed = Some(new_val);

                        // Update buffer to show the clamped/committed value
                        let clean_text = format!("{:.3}", val_clamped);
                        ui.data_mut(|d| d.insert_temp(id, clean_text));
                    }
                }
            });
        });

        // NEW: Initialize Log Mapper
        let mapper = LogMapper::new(min_pct, max_pct);

        // --- VISUALIZATION STRATEGY: INFERNO HEATMAP ---
        // Instead of using discrete 'QualityZone' buckets (Red/Green/Blue),
        // we use a continuous gradient. This distinguishes "Data Density" (PH Bar)
        // from "Trade Signals" (Cyan/Magenta Wicks) on the main chart.

        let max_count = if let Some(p) = self.profile {
            p.max_candle_count
        } else {
            0
        };

        // Define Gradient (Deep Purple -> Orange -> Yellow)
        // FIXED: Use GradientBuilder and preset module
        let gradient = colorgrad::GradientBuilder::new()
            .colors(&[
                colorgrad::Color::from_html("#2d0b59").unwrap(), // Deep Purple
                colorgrad::Color::from_html("#781c6d").unwrap(), // Purple
                colorgrad::Color::from_html("#bc3754").unwrap(), // Red-Pink
                colorgrad::Color::from_html("#ed6925").unwrap(), // Orange
                colorgrad::Color::from_html("#fbb41a").unwrap(), // Gold/Yellow
                colorgrad::Color::from_html("#fcffa4").unwrap(), // Pale Yellow
            ])
            .build::<colorgrad::LinearGradient>() // Explicit linear interpolation
            .expect("Failed to build Price Horizon Gradient");

        // Define Color Function
        let get_color = move |count: usize| -> Color32 {
            if max_count == 0 || count == 0 {
                return Color32::TRANSPARENT; // Or Color32::from_rgb(45, 11, 89) for solid background
            }

            // Normalize current bucket against the profile's max
            let val = count as f64;
            let max = max_count as f64;

            // Use sqrt curve so lower-mid values don't disappear into the dark
            let t = (val / max).sqrt();

            let rgba = gradient.at(t as f32).to_rgba8();
            Color32::from_rgb(rgba[0], rgba[1], rgba[2])
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

                        // Calculate Pixel Coordinates
                        let x_start_frac = mapper.value_to_frac(val_start);
                        let x_end_frac = mapper.value_to_frac(val_end);

                        let x_start_px = rect.min.x + (x_start_frac as f32 * rect.width());
                        let x_end_px = rect.min.x + (x_end_frac as f32 * rect.width());

                        // FIX: Calculate width contiguously + overlap for AA
                        // 1. Calculate pure geometric width
                        // 2. Add 1.0 to overlap the next bar slightly (fixes sub-pixel black lines)
                        // 3. Ensure min width 1.0 so tiny buckets are visible
                        let width_px = (x_end_px - x_start_px).max(0.0) + 1.0;

                        // FIX Y: Pixel Snap vertical coordinates to prevent bottom-edge artifacts
                        // We round the start and the height to ensure we land on integer pixel boundaries.
                        let y_start = (rect.min.y + 2.0).round();
                        let bar_height = (rect.height() - 4.0).round();

                        let bar_rect = Rect::from_min_size(
                            pos2(x_start_px, y_start),
                            vec2(width_px, bar_height),
                        );

                        let color = get_color(bucket.candle_count);
                        painter.rect_filled(bar_rect, 0.0, color);
                    }
                }

                // Draw Handle (Logarithmic Position)
                let handle_frac = mapper.value_to_frac(current_pct) as f32;
                let handle_x = rect.min.x + (handle_frac * rect.width());
                let handle_rect = Rect::from_center_size(
                    pos2(handle_x, rect.center().y),
                    vec2(4.0, rect.height()),
                );
                painter.rect_filled(handle_rect, 1.0, Color32::WHITE);
            }

            self.render_horizon_report(ui, current_pct, profile);
        } else {
            self.render_loading_state(ui);
        }

        changed
    }

    fn render_loading_state(&self, ui: &mut Ui) {
        let desired_size = vec2(ui.available_width(), 40.0);
        let (rect, _response) = ui.allocate_exact_size(desired_size, Sense::hover());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            painter.rect_filled(rect, 2.0, PLOT_CONFIG.color_widget_background);
            painter.rect_stroke(
                rect,
                2.0,
                Stroke::new(1.0, PLOT_CONFIG.color_widget_border),
                StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                &UI_TEXT.ph_startup,
                FontId::proportional(12.0),
                PLOT_CONFIG.color_text_subdued,
            );
        }
    }

    fn render_horizon_report(&self, ui: &mut Ui, current_pct: f64, profile: &HorizonProfile) {
        ui.vertical(|ui| {
            ui.add_space(4.0);

            ui.label(
                RichText::new(format!(
                    "{} {:.2}%",
                    UI_TEXT.ph_label_horizon_prefix,
                    current_pct * 100.0
                ))
                .strong()
                .color(PLOT_CONFIG.color_text_primary),
            );

            if let Some(bucket) = profile.buckets.iter().min_by(|a, b| {
                (a.threshold_pct - current_pct)
                    .abs()
                    .partial_cmp(&(b.threshold_pct - current_pct).abs())
                    .unwrap()
            }) {
                let is_current_config =
                    (current_pct - self.price_horizon_config.threshold_pct).abs() < f64::EPSILON;

                // 1. Get Authoritative Count
                let count = if is_current_config {
                    self.actual_candle_count
                } else {
                    bucket.candle_count
                };

                // 2. Calculate History (Span of Time)
                // "How long is the calendar period covered by this range?"
                let span_ms = bucket.max_ts.saturating_sub(bucket.min_ts);
                let history_days = span_ms as f64 / (1000.0 * 60.0 * 60.0 * 24.0);

                // 3. Calculate Evidence (Mass of Data)
                // "How much actual data do we have?" (Count * 5mins)
                let interval_ms = self.interval_ms as f64;
                let evidence_ms = count as f64 * interval_ms;
                let evidence_days = evidence_ms / (1000.0 * 60.0 * 60.0 * 24.0);

                // 4. Calculate Density
                let density_pct = if history_days > 0.001 {
                    (evidence_days / history_days) * 100.0
                } else {
                    0.0
                };

                // Row A: Evidence (Actual Data Duration)
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{}:", UI_TEXT.ph_label_evidence))
                            .small()
                            .color(PLOT_CONFIG.color_text_subdued),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} ({})",
                            format_duration_context(evidence_days), // Use calculated evidence
                            format_candle_count(count)
                        ))
                        .strong()
                        .color(PLOT_CONFIG.color_text_neutral),
                    );
                });

                // Row B: History (Calendar Span)
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{}:", UI_TEXT.ph_label_history))
                            .small()
                            .color(PLOT_CONFIG.color_text_subdued),
                    );
                    ui.label(
                        RichText::new(format_duration_context(history_days))
                            .color(PLOT_CONFIG.color_info),
                    );
                });

                // Row C: Density (Quality)
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{}:", UI_TEXT.ph_label_density))
                            .small()
                            .color(PLOT_CONFIG.color_text_subdued),
                    );
                    // Semantic Density Coloring
                    let density_color = if density_pct > 90.0 {
                        PLOT_CONFIG.color_profit // Green
                    } else if density_pct > 50.0 {
                        PLOT_CONFIG.color_warning // Yellow
                    } else {
                        PLOT_CONFIG.color_loss // Red
                    };
                    ui.label(RichText::new(format!("{:.1}%", density_pct)).color(density_color));
                });
            }
        });
    }

    fn render_pair_selector(&mut self, ui: &mut Ui) -> Option<String> {
        let mut changed = None;
        let previously_selected_pair = self.selected_pair.clone();

        ui.label(colored_subsection_heading(&UI_TEXT.pair_selector_heading));
        ScrollArea::vertical()
            .max_height(ui.available_height() - 50.0)
            .id_salt("pair_selector")
            .show(ui, |ui| {
                for item in &self.available_pairs {
                    let is_selected = self.selected_pair.as_ref() == Some(item);
                    let response = ui.selectable_label(is_selected, item);

                    if response.clicked() {
                        self.selected_pair = Some(item.clone());
                        changed = Some(item.clone());
                    }

                    // NEW LOGIC: Only scroll if there is an explicit external request for THIS item
                    if let Some(target) = self.scroll_to_pair_requested {
                        if target == item {
                            // Use None to "Make Visible" (minimal scrolling) instead of Center
                            response.scroll_to_me(None);

                            // Clear the request immediately so it doesn't happen again
                            *self.scroll_to_pair_requested = None;
                        }
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
}

impl<'a> Panel for DataGenerationPanel<'a> {
    type Event = DataGenerationEventChanged;
    fn render(&mut self, ui: &mut Ui, show_help: &mut bool) -> Vec<Self::Event> {
        let mut events = Vec::new();
        section_heading(ui, &UI_TEXT.data_generation_heading);

        // Price Horizon display (always enabled)
        if let Some(threshold) = self.render_price_horizon_display(ui, show_help) {
            events.push(DataGenerationEventChanged::PriceHorizonThreshold(threshold));
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