use std::hash::{Hash, Hasher};
use std::sync::Arc;

use colorgrad::Gradient;
use eframe::egui::{self, Color32, Stroke};
use egui_plot::{AxisHints, Corner, HPlacement, Legend, Plot, PlotPoints, Polygon}; // Import needed for gradient.at()

use crate::config::plot::PLOT_CONFIG;
use crate::models::cva::{CVACore, ScoreType};
use crate::models::trading_view::{SuperZone, TradingModel};
use crate::ui::app::PlotVisibility;
use crate::ui::ui_text::UI_TEXT;
use crate::utils::maths_utils;

/// A lightweight representation of a background bar, drawn as a Polygon.
#[derive(Clone)]
pub struct BackgroundBar {
    pub x_max: f64,    // The length of the bar (0.0 to 1.0)
    pub y_center: f64, // The center price
    pub height: f64,   // The thickness of the bar
    pub color: Color32,
}

#[derive(Clone)]
pub struct PlotCache {
    pub cva_hash: u64,
    // CHANGED: Stores our custom struct instead of egui_plot::Bar
    pub bars: Vec<BackgroundBar>,
    pub y_min: f64,
    pub y_max: f64,
    pub x_min: f64,
    pub x_max: f64,
    pub bar_thickness: f64,
    pub time_decay_factor: f64,
    // Metadata fields
    pub score_type: ScoreType,
    pub sticky_zone_indices: Vec<usize>,
    pub zone_scores: Vec<f64>,
    pub total_width: f64,
}

#[derive(Default)]
pub struct PlotView {
    cache: Option<PlotCache>,
}

impl PlotView {
    pub fn new() -> Self {
        Self { cache: None }
    }

    pub fn cache_hits(&self) -> usize {
        0
    }
    pub fn cache_misses(&self) -> usize {
        0
    }
    pub fn cache_hit_rate(&self) -> Option<f64> {
        None
    }

    pub fn clear_cache(&mut self) {
        self.cache = None;
    }

    pub fn has_cache(&self) -> bool {
        self.cache.is_some()
    }

    pub fn show_my_plot(
        &mut self,
        ui: &mut egui::Ui,
        cva_results: &CVACore,
        current_pair_price: Option<f64>,
        background_score_type: ScoreType,
        visibility: &PlotVisibility,
    ) {
        let trading_model =
            TradingModel::from_cva(Arc::new(cva_results.clone()), current_pair_price);

        let cache = self.calculate_plot_data(cva_results, background_score_type);
        let pair_name = &cva_results.pair_name;

        let _legend = Legend::default().position(Corner::RightTop);

        Plot::new("my_plot")
            .view_aspect(PLOT_CONFIG.plot_aspect_ratio)
            .legend(_legend)
            .custom_x_axes(vec![create_x_axis(&cache)])
            .custom_y_axes(vec![create_y_axis(pair_name)])
            // // FIX ATTEMP: on-hover plot label. adding this code just renders a tiny empty box instead of default (x,y) box so not much use but at least it is small I guess
            .label_formatter(|_, _| String::new())
            .x_grid_spacer(move |_input| {
                let mut marks = Vec::new();
                let (min, max) = _input.bounds;
                let range = max - min;
                let step_size = if range < 0.1 { 0.02 } else { 0.1 };
                let start = (min / step_size).ceil() as i64;
                let end = (max / step_size).floor() as i64;
                for i in start..=end {
                    let value = i as f64 * step_size;
                    if value >= 0.0 && value <= 1.0 {
                        marks.push(egui_plot::GridMark { value, step_size });
                    }
                }
                marks
            })
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_boxed_zoom(false)
            .show(ui, |plot_ui| {
                let (y_min, y_max) = cva_results.price_range.min_max();
                let price = current_pair_price.unwrap_or(y_min);
                let y_min_adjusted = y_min.min(price);
                let y_max_adjusted = y_max.max(price);

                plot_ui.set_plot_bounds_y(y_min_adjusted..=y_max_adjusted);
                plot_ui.set_plot_bounds_x(cache.x_min..=cache.x_max);

                draw_background_plot(plot_ui, &cache, background_score_type);
                draw_classified_zones(
                    plot_ui,
                    &trading_model,
                    cache.x_min,
                    cache.x_max,
                    &visibility,
                );
                draw_current_price(plot_ui, current_pair_price);
            });
    }

    fn calculate_plot_data(&mut self, cva_results: &CVACore, score_type: ScoreType) -> PlotCache {
        let zone_count = cva_results.zone_count;
        let time_decay_factor = cva_results.time_decay_factor;

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        cva_results
            .price_range
            .min_max()
            .0
            .to_bits()
            .hash(&mut hasher);
        cva_results
            .price_range
            .min_max()
            .1
            .to_bits()
            .hash(&mut hasher);
        zone_count.hash(&mut hasher);
        score_type.hash(&mut hasher);
        time_decay_factor.to_bits().hash(&mut hasher);
        cva_results
            .get_scores_ref(score_type)
            .len()
            .hash(&mut hasher);
        let current_hash = hasher.finish();

        if let Some(cache) = &self.cache {
            if cache.cva_hash == current_hash {
                return cache.clone();
            }
        }

        let (y_min, y_max) = cva_results.price_range.min_max();
        let bar_width = (y_max - y_min) / zone_count as f64;

        // --- PREPARE DATA FOR DISPLAY ---
        let raw_data_vec = cva_results.get_scores_ref(score_type).clone();

        // Apply Smoothing
        let smoothing_window = ((zone_count as f64 * 0.02).ceil() as usize).max(1) | 1;
        let smoothed_data = maths_utils::smooth_data(&raw_data_vec, smoothing_window);

        // Normalize
        let data_for_display = maths_utils::normalize_max(&smoothed_data);

        let indices: Vec<usize> = (0..zone_count).collect();

        let grad = colorgrad::GradientBuilder::new()
            .html_colors(PLOT_CONFIG.zone_gradient_colors)
            .build::<colorgrad::CatmullRomGradient>()
            .expect("Failed to create color gradient");

        // --- GENERATE POLYGON DATA ---
        let bars: Vec<BackgroundBar> = indices
            .iter()
            .map(|&original_index| {
                let zone_score = data_for_display[original_index];
                let (z_min, z_max) = cva_results.price_range.chunk_bounds(original_index);
                let center_price = (z_min + z_max) / 2.0;

                let color = get_zone_color_from_zone_value(zone_score, &grad);
                let dimmed_color = color.linear_multiply(PLOT_CONFIG.background_bar_intensity_pct);

                // log::warn!("{}", format!("{:?} {}", dimmed_color, zone_score));

                BackgroundBar {
                    x_max: zone_score,
                    y_center: center_price,
                    height: bar_width * 0.9,
                    color: dimmed_color,
                }
            })
            .collect();

        let cache = PlotCache {
            cva_hash: current_hash,
            bars,
            y_min,
            y_max,
            x_min: 0.0,
            x_max: 1.0,
            bar_thickness: bar_width,
            time_decay_factor,
            score_type,
            sticky_zone_indices: indices,
            zone_scores: data_for_display,
            total_width: 1.0,
        };

        self.cache = Some(cache.clone());
        cache
    }
}

fn create_x_axis(_plot_cache: &PlotCache) -> AxisHints<'static> {
    AxisHints::new_x()
        .label(UI_TEXT.plot_x_axis)
        .formatter(move |grid_mark, _range| {
            let pct = grid_mark.value * 100.0;
            format!("{:.0}%", pct)
        })
}

fn create_y_axis(pair_name: &str) -> AxisHints<'static> {
    let label = format!("{}  {}", pair_name, UI_TEXT.plot_y_axis);
    AxisHints::new_y()
        .label(label)
        .formatter(|grid_mark, _range| format!("${:.2}", grid_mark.value))
        .placement(HPlacement::Left)
}

fn get_zone_color_from_zone_value(
    zone_value: f64,
    gradient: &colorgrad::CatmullRomGradient,
) -> Color32 {
    to_egui_color(gradient.at(zone_value as f32))
}

fn to_egui_color(colorgrad_color: colorgrad::Color) -> Color32 {
    let rgba8 = colorgrad_color.to_rgba8();
    Color32::from_rgba_unmultiplied(rgba8[0], rgba8[1], rgba8[2], 255)
}

/// Draw background plot using dumb Polygons (no interaction)
fn draw_background_plot(plot_ui: &mut egui_plot::PlotUi, cache: &PlotCache, score_type: ScoreType) {

        // 1. Determine Label
    let type_label = match score_type {
        ScoreType::FullCandleTVW => "Trading Volume",
        ScoreType::LowWickCount => "Lower Wick Count",
        ScoreType::HighWickCount => "Upper Wick Count",
        _ => "Unknown",
    };
    // 2. Create Group Name (Appears in Legend)
    let legend_label = format!("Zone Strength: {}", type_label);

    for bar in &cache.bars {
        let half_h = bar.height / 2.0;

        let points = PlotPoints::new(vec![
            [0.0, bar.y_center - half_h],
            [bar.x_max, bar.y_center - half_h],
            [bar.x_max, bar.y_center + half_h],
            [0.0, bar.y_center + half_h],
        ]);

        let polygon = Polygon::new(&legend_label, points)
            .fill_color(bar.color)
            .allow_hover(false) // Note doesn't seem to help anything but default is aparently .allow_hover = true
            .stroke(Stroke::NONE); // Very important to have this code in i.e. set Stroke to None. If not, rendering of background bars is *very different*

        plot_ui.polygon(polygon);
    }
}

fn draw_current_price(plot_ui: &mut egui_plot::PlotUi, current_pair_price: Option<f64>) {
    if let Some(price) = current_pair_price {
        use egui_plot::HLine;

        let label = "Current Price";

        plot_ui.hline(
            HLine::new(label, price)
                .color(PLOT_CONFIG.current_price_outer_color)
                .width(PLOT_CONFIG.current_price_outer_width)
                .style(egui_plot::LineStyle::dashed_loose()),
        );

        plot_ui.hline(
            HLine::new(label, price)
                .color(PLOT_CONFIG.current_price_color)
                .width(PLOT_CONFIG.current_price_line_width),
        );
    }
}

fn draw_classified_zones(
    plot_ui: &mut egui_plot::PlotUi,
    trading_model: &TradingModel,
    x_min: f64,
    x_max: f64,
    visibility: &PlotVisibility,
) {
    let current_price = trading_model.current_price;

    // Helper: Determine Color (Red/Green/Purple) based on price position
    let get_zone_status = |zone: &SuperZone| -> Color32 {
        if let Some(price) = current_price {
            if zone.contains(price) {
                PLOT_CONFIG.sticky_zone_color // Purple (Active)
            } else if zone.price_center < price {
                PLOT_CONFIG.support_zone_color // Green
            } else {
                PLOT_CONFIG.resistance_zone_color // Red
            }
        } else {
            PLOT_CONFIG.sticky_zone_color
        }
    };

    // Helper: Determine Stroke (Gold if active)
    let get_stroke = |zone: &SuperZone, base_color: Color32| -> Stroke {
        let is_active = current_price.map(|p| zone.contains(p)).unwrap_or(false);

        if is_active {
            Stroke::new(
                PLOT_CONFIG.active_zone_stroke_width,
                PLOT_CONFIG.active_zone_stroke_color,
            )
        } else {
            Stroke::new(1.0, base_color)
        }
    };

       // 1. Sticky Zones (Wide, Faint)
    if visibility.sticky {
        for superzone in &trading_model.zones.sticky_superzones {
            let color = get_zone_status(superzone);
            let label = UI_TEXT.label_hvz;
            let stroke = get_stroke(superzone, color);

            draw_superzone(
                plot_ui, superzone, x_min, x_max, &label, color, stroke, 1.0, 1.0, ZoneShape::Rectangle,
            );
        }
    }

    // 2. Low Wicks (Narrow, Solid)
    if visibility.low_wicks {
        for superzone in &trading_model.zones.low_wicks_superzones {
            // Filter: Only draw if relevant (Below or Active)
            let is_relevant = current_price
                .map(|p| superzone.contains(p) || superzone.price_center < p)
                .unwrap_or(false);

            if is_relevant {
                let color = get_zone_status(superzone);
                let label = UI_TEXT.label_reversal_support;
                let stroke = get_stroke(superzone, color);

                draw_superzone(
                    plot_ui, superzone, x_min, x_max, &label, color, stroke, 0.5, 1.5, ZoneShape::TriangleUp
                );
            }
        }
    }

        // 3. High Wicks (Narrow, Solid)
    if visibility.high_wicks {
        for superzone in &trading_model.zones.high_wicks_superzones {
            // Filter: Only draw if relevant (Above or Active)
            let is_relevant = current_price
                .map(|p| superzone.contains(p) || superzone.price_center > p)
                .unwrap_or(false);

            if is_relevant {
                let color= get_zone_status(superzone);
                let label = UI_TEXT.label_reversal_resistance;
                let stroke = get_stroke(superzone, color);

                draw_superzone(
                    plot_ui, superzone, x_min, x_max, &label, color, stroke, 0.5, 1.5, ZoneShape::TriangleDown
                );
            }
        }
    }
}

enum ZoneShape {
    Rectangle,
    TriangleUp,   // Flat bottom, points up
    TriangleDown, // Flat top, points down
}

fn draw_superzone(
    plot_ui: &mut egui_plot::PlotUi,
    superzone: &SuperZone,
    x_min: f64,
    x_max: f64,
    label: &str,
    fill_color: Color32,
    stroke: Stroke,
    width_factor: f64,
    opacity_factor: f32,
    shape: ZoneShape,
) {
    // Calculate Centered Width
    let total_width = x_max - x_min;
    let actual_width = total_width * width_factor;
    let margin = (total_width - actual_width) / 2.0;

    let z_x_min = x_min + margin;
    let z_x_max = x_max - margin;
    let z_x_center = z_x_min + (actual_width / 2.0);

    let points_vec = match shape {
        ZoneShape::Rectangle => vec![
            [z_x_min, superzone.price_bottom],
            [z_x_max, superzone.price_bottom],
            [z_x_max, superzone.price_top],
            [z_x_min, superzone.price_top],
        ],
        ZoneShape::TriangleUp => vec![
            [z_x_min, superzone.price_bottom], // Bottom Left
            [z_x_max, superzone.price_bottom], // Bottom Right
            [z_x_center, superzone.price_top], // Top Point
        ],
        ZoneShape::TriangleDown => vec![
            [z_x_min, superzone.price_top],    // Top Left
            [z_x_max, superzone.price_top],    // Top Right
            [z_x_center, superzone.price_bottom], // Bottom Point
        ],
    };

    let points = PlotPoints::new(points_vec);
    let final_color = fill_color.linear_multiply(PLOT_CONFIG.zone_fill_opacity_pct * opacity_factor);

    let polygon = Polygon::new(label, points)
        .fill_color(final_color)
        .stroke(stroke)
        .highlight(true); // Highlight superzones

    plot_ui.polygon(polygon);

    // Manual Hit Test
    if let Some(pointer) = plot_ui.pointer_coordinate() {
        if pointer.y >= superzone.price_bottom
            && pointer.y <= superzone.price_top
            && pointer.x >= x_min
            && pointer.x <= x_max
        {
            let tooltip_layer =
                egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("zone_tooltips"));

            #[allow(deprecated)]
            egui::show_tooltip_at_pointer(
                plot_ui.ctx(),
                tooltip_layer,
                egui::Id::new(format!("tooltip_{}", superzone.id)),
                |ui: &mut egui::Ui| {
                    ui.label(egui::RichText::new(label).strong().color(fill_color));
                    ui.separator();
                    ui.label(format!("ID: #{}", superzone.id));
                    ui.label(format!(
                        "Range: ${:.2} - ${:.2}",
                        superzone.price_bottom, superzone.price_top
                    ));
                    let height = superzone.price_top - superzone.price_bottom;
                    ui.label(format!("Height: ${:.2}", height));
                },
            );
        }
    }
}
