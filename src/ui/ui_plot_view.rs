use colorgrad::Gradient;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use eframe::egui::{self, Color32};
use egui_plot::{AxisHints, Corner, HPlacement, Legend, Plot};

use crate::config::plot::PLOT_CONFIG;
use crate::models::cva::{CVACore, ScoreType};
use crate::models::trading_view::TradingModel;
use crate::ui::ui_text::UI_TEXT;
use crate::utils::maths_utils;

// Import the new Layer System
use crate::ui::plot_layers::{
    BackgroundLayer, LayerContext, PlotLayer, PriceLineLayer, ReversalZoneLayer, StickyZoneLayer,
};

/// A lightweight representation of a background bar.
#[derive(Clone)]
pub struct BackgroundBar {
    pub x_max: f64,
    pub y_center: f64,
    pub height: f64,
    pub color: Color32,
}

#[derive(Clone)]
pub struct PlotCache {
    pub cva_hash: u64,
    pub bars: Vec<BackgroundBar>,
    pub y_min: f64,
    pub y_max: f64,
    pub x_min: f64,
    pub x_max: f64,
    pub bar_thickness: f64,
    pub time_decay_factor: f64,
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
        visibility: &crate::ui::app::PlotVisibility,
    ) {
        let trading_model =
            TradingModel::from_cva(Arc::new(cva_results.clone()), current_pair_price);

        let cache = self.calculate_plot_data(cva_results, background_score_type);
        let pair_name = &cva_results.pair_name;
        let (y_min, y_max) = cva_results.price_range.min_max();
        let total_y_range = y_max - y_min;

        let _legend = Legend::default().position(Corner::RightTop);

        Plot::new("my_plot")
            // .view_aspect(PLOT_CONFIG.plot_aspect_ratio)
            .legend(_legend)
            .custom_x_axes(vec![create_x_axis(&cache)])
            .custom_y_axes(vec![create_y_axis(pair_name)])
            // Suppress Defaults
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
            // NEW: Force Y-Axis Labels at Start/End
            .y_grid_spacer(move |_input| {
                let mut marks = Vec::new();

                // 1. Mandatory Start (Min Price)
                marks.push(egui_plot::GridMark {
                    value: y_min,
                    step_size: total_y_range,
                });

                // 2. Mandatory End (Max Price)
                marks.push(egui_plot::GridMark {
                    value: y_max,
                    step_size: total_y_range,
                });

                // 3. Fill in the middle (e.g. 5 even steps) to keep it readable
                // We use a slightly different step_size so egui knows they are secondary
                let divisions = 5;
                let step = total_y_range / divisions as f64;
                for i in 1..divisions {
                    let value = y_min + (step * i as f64);
                    marks.push(egui_plot::GridMark {
                        value,
                        step_size: step,
                    });
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

                // --- LAYER RENDERING SYSTEM ---

                // 1. Create Context
                let ctx = LayerContext {
                    trading_model: &trading_model,
                    cache: &cache,
                    visibility,
                    background_score_type,
                    x_min: cache.x_min,
                    x_max: cache.x_max,
                };

                // 2. Define Layer Stack (Back to Front)
                let layers: Vec<Box<dyn PlotLayer>> = vec![
                    Box::new(BackgroundLayer),
                    Box::new(StickyZoneLayer),
                    Box::new(ReversalZoneLayer),
                    Box::new(PriceLineLayer),
                ];

                // 3. Render Loop
                for layer in layers {
                    layer.render(plot_ui, &ctx);
                }
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

        // Raw Data (Raw Counts)
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

        // Generate BackgroundBars
        let bars: Vec<BackgroundBar> = indices
            .iter()
            .map(|&original_index| {
                let zone_score = data_for_display[original_index];
                let (z_min, z_max) = cva_results.price_range.chunk_bounds(original_index);
                let center_price = (z_min + z_max) / 2.0;

                let color = to_egui_color(grad.at(zone_score as f32));
                let dimmed_color = color.linear_multiply(PLOT_CONFIG.background_bar_intensity_pct);

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

// Helpers retained locally for calculate_plot_data
fn to_egui_color(colorgrad_color: colorgrad::Color) -> Color32 {
    let rgba8 = colorgrad_color.to_rgba8();
    Color32::from_rgba_unmultiplied(rgba8[0], rgba8[1], rgba8[2], 255)
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
