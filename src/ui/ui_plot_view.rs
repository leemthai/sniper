use colorgrad::{CatmullRomGradient, Color, Gradient};
use eframe::egui::{self, Color32, Stroke};
use egui_plot::{
    AxisHints, Bar, BarChart, Corner, HLine, HPlacement, Legend, Plot, PlotPoints, Polygon,
};
use std::sync::Arc;

use crate::analysis::selection_criteria::{FilterChain, ZoneSelectionCriteria};
use crate::config::plot::PLOT_CONFIG;
use crate::models::cva::{CVACore, ScoreType};
use crate::models::{SuperZone, TradingModel};
use crate::ui::ui_text::UI_TEXT;
use crate::utils::maths_utils;

#[cfg(debug_assertions)]
use crate::config::DEBUG_FLAGS;

#[derive(Clone, Debug, PartialEq)]
pub struct PlotCache {
    bars: Vec<Bar>,
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
    bar_thickness: f64,
    total_width: f64,
    zone_count: usize,
    score_type: ScoreType,
    cva_hash: u64,
    time_decay_factor: f64,
    sticky_zone_indices: Vec<usize>, // Zones passing the filter (for support/resistance)
    zone_scores: Vec<f64>, // Full normalized scores for all zones (for gradient calculation)
}

#[derive(Default)]
pub struct PlotView {
    cache: Option<PlotCache>,
    cache_hits: usize,
    cache_misses: usize,
}

impl PlotView {
    pub fn new() -> Self {
        Self {
            cache: None,
            cache_hits: 0,
            cache_misses: 0,
        }
    }

    /// Returns the number of cache hits
    pub fn cache_hits(&self) -> usize {
        self.cache_hits
    }

    /// Returns the number of cache misses
    pub fn cache_misses(&self) -> usize {
        self.cache_misses
    }

    /// Returns the cache hit rate as a percentage
    pub fn cache_hit_rate(&self) -> Option<f64> {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            None
        } else {
            Some((self.cache_hits as f64 / total as f64) * 100.0)
        }
    }

    /// Clears the cache and resets statistics
    pub fn clear_cache(&mut self) {
        self.cache = None;
        self.cache_hits = 0;
        self.cache_misses = 0;
    }

    /// Returns whether the cache is currently populated
    pub fn has_cache(&self) -> bool {
        self.cache.is_some()
    }

    pub fn show_my_plot(
        &mut self,
        ui: &mut egui::Ui,
        cva_results: &CVACore,
        current_pair_price: Option<f64>,
    ) {
        let pair_name = &cva_results.pair_name;

        // Build trading model with zone classification
        let trading_model =
            TradingModel::from_cva(Arc::new(cva_results.clone()), current_pair_price);

        // Background bars can be any member of ScoreType
        let background_score_type = ScoreType::CandleBodyVW;
        let cache = self.calculate_plot_data(cva_results, background_score_type);

        let x_min = cache.x_min;
        let total_width = cache.total_width;

        // Legend setup
        let _legend = Legend::default().position(Corner::RightTop);

        // Show the plot within the CentralPanel
        Plot::new("cva")
            .view_aspect(PLOT_CONFIG.plot_aspect_ratio)
            // .legend(_legend)
            .custom_x_axes(vec![create_x_axis(&cache)])
            .custom_y_axes(vec![create_y_axis(pair_name)])
            .x_grid_spacer(move |_input| {
                let step_count = PLOT_CONFIG.plot_axis_divisions;
                (0..=step_count)
                    .map(|i| {
                        let fraction = i as f64 / step_count as f64;
                        let value = x_min + (total_width * fraction);

                        egui_plot::GridMark {
                            value,
                            step_size: total_width / step_count as f64,
                        }
                    })
                    .collect()
            })
            .label_formatter(move |_name, value| {
                let pct = ((value.x - x_min) / total_width) * 100.;
                format!(
                    "{:.2}% {}\n${:.4}",
                    pct, UI_TEXT.plot_strongest_zone, value.y
                )
            })
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_boxed_zoom(false)
            .show(ui, |plot_ui| {
                // Expand Y bounds to include current price if needed
                let (y_min_adjusted, y_max_adjusted) = if let Some(price) = current_pair_price {
                    (cache.y_min.min(price), cache.y_max.max(price))
                } else {
                    (cache.y_min, cache.y_max)
                };

                plot_ui.set_plot_bounds_y(y_min_adjusted..=y_max_adjusted);
                plot_ui.set_plot_bounds_x(cache.x_min..=cache.x_max);

                // draw background plot first
                draw_background_plot(plot_ui, &cache, background_score_type);

                // Draw all classified zones from TradingModel
                draw_classified_zones(plot_ui, &trading_model, cache.x_min, cache.x_max);

                // Draw current price line LAST for max. visibility
                draw_current_price(plot_ui, current_pair_price);
            });
    }

    fn calculate_plot_data(&mut self, cva_results: &CVACore, score_type: ScoreType) -> PlotCache {
        // Source of truth: read zone_count from the data itself
        let zone_count = cva_results.zone_count;
        let time_decay_factor = cva_results.time_decay_factor;

        // Calculate hash of CVA results to detect changes
        let cva_hash = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            (
                &cva_results.pair_name, // Include pair name to detect pair changes!
                cva_results.start_timestamp_ms,
                cva_results.end_timestamp_ms,
                (
                    cva_results.price_range.min_max().0 as i64,
                    cva_results.price_range.min_max().1 as i64,
                ),
            )
                .hash(&mut hasher);
            hasher.finish()
        };

        // Check if we have cached data that matches
        if let Some(cache) = &self.cache
            && cache.zone_count == zone_count
            && cache.score_type == score_type
            && cache.cva_hash == cva_hash
            && cache.time_decay_factor.to_bits() == time_decay_factor.to_bits()
        {
            self.cache_hits += 1;
            #[cfg(debug_assertions)]
            if DEBUG_FLAGS.print_cva_cache_events {
                log::info!(
                    "[plot cache] HIT: {} zones for {} (cache key {:?})",
                    cache.zone_scores.len(),
                    cache.score_type,
                    cache.cva_hash
                );
            }
            return cache.clone();
        }
        // Cache miss
        #[cfg(debug_assertions)]
        if DEBUG_FLAGS.print_plot_cache_stats {
            log::info!(
                "Bar Plot Cache MISS for {} with {} zones.",
                score_type,
                zone_count
            );
        }
        self.cache_misses += 1;

        // Calculate plot bounds
        let (y_min, y_max) = cva_results.price_range.min_max();

        let x_min: f64 = (cva_results.start_timestamp_ms as f64) / 1000.0;
        let x_max: f64 = (cva_results.end_timestamp_ms as f64) / 1000.0;
        let total_width = x_max - x_min;
        let zone_score_scalar = total_width;
        let bar_width_scalar = y_max - y_min;
        let bar_thickness = bar_width_scalar / (zone_count as f64);

        // Use CandleBodyVW for background bars (consolidated volume)
        let data_for_display = maths_utils::normalize_max(cva_results.get_scores_ref(score_type));

        // Apply filter chain to select all zones
        let filter_chain =
            FilterChain::new(score_type, ZoneSelectionCriteria::PercentileRange(0.0, 1.0));

        let combined_indices: Vec<usize> = filter_chain
            .evaluate(cva_results)
            .unwrap_or_default()
            .into_iter()
            .collect();

        // Create color gradient from config
        let grad = colorgrad::GradientBuilder::new()
            .html_colors(PLOT_CONFIG.zone_gradient_colors)
            .build::<colorgrad::CatmullRomGradient>()
            .expect("Failed to create color gradient");

        // Generate bars using the display data (single or combined scores)
        // Dim bars significantly so they serve as background layer
        let bars = combined_indices
            .iter()
            .map(|&original_index| {
                let zone_score = data_for_display[original_index];
                let color = get_zone_color_from_zone_value(zone_score, &grad);
                let dimmed_color = color.linear_multiply(PLOT_CONFIG.background_bar_intensity_pct);
                let y_position = y_min + (original_index as f64) * bar_thickness;
                let x_position = x_min + zone_score * zone_score_scalar;
                Bar::new(y_position, x_position)
                    .fill(dimmed_color)
                    .name(format!("{}", original_index))
            })
            .collect();

        // Create and store cache
        let cache = PlotCache {
            bars,
            x_min,
            x_max,
            y_min,
            y_max,
            bar_thickness,
            total_width,
            zone_count,
            score_type,
            cva_hash,
            time_decay_factor,
            sticky_zone_indices: combined_indices,
            zone_scores: data_for_display,
        };

        self.cache = Some(cache.clone());
        cache
    }
}

fn create_x_axis(plot_cache: &PlotCache) -> AxisHints<'static> {
    let x_min = plot_cache.x_min;
    let total_width = plot_cache.total_width;

    AxisHints::new_x()
        .label(UI_TEXT.plot_x_axis)
        .formatter(move |grid_mark, _range| {
            let pct = ((grid_mark.value - x_min) / total_width) * 100.0;
            format!("{:.0}%", pct)
        })
}

fn create_y_axis(pair_name: &str) -> AxisHints<'static> {
    AxisHints::new_y()
        .label(format!("{}  {}", pair_name, UI_TEXT.plot_y_axis)) // 2 spaces deliberate here
        .formatter(|grid_mark, _range| format!("${:.2}", grid_mark.value))
        .placement(HPlacement::Left)
}

fn get_zone_color_from_zone_value(zone_value: f64, gradient: &CatmullRomGradient) -> Color32 {
    debug_assert!(
        (0.0..=1.).contains(&zone_value),
        "zone_value must be between 0. and 1."
    );
    to_egui_color(gradient.at(zone_value as f32))
}

fn to_egui_color(colorgrad_color: Color) -> Color32 {
    let rgba8 = colorgrad_color.to_rgba8();
    Color32::from_rgba_unmultiplied(rgba8[0], rgba8[1], rgba8[2], rgba8[3])
}

/// Draw background plot with bars representing score type
fn draw_background_plot(
    plot_ui: &mut egui_plot::PlotUi,
    cache: &PlotCache,
    background_score_type: ScoreType,
) {
    let title = format!("{}", background_score_type);
    let x_min = cache.x_min;
    let total_width = cache.total_width;
    let chart = BarChart::new(title, cache.bars.clone())
        .color(PLOT_CONFIG.default_bar_color) // Legend color only
        .width(cache.bar_thickness)
        .horizontal()
        .element_formatter(Box::new(move |bar, _chart| {
            format!(
                "{} {:.2}% {}\n${:.4}",
                UI_TEXT.plot_this_zone_is,
                ((bar.value - x_min) / total_width) * 100.,
                UI_TEXT.plot_strongest_zone,
                bar.argument
            )
        }));
    plot_ui.bar_chart(chart);
}

/// Draw the current price line with outer border for visibility
fn draw_current_price(plot_ui: &mut egui_plot::PlotUi, current_pair_price: Option<f64>) {
    if let Some(price) = current_pair_price {
        // Draw contrasting outer border first
        let outer_line = HLine::new("Current Price", price)
            .color(PLOT_CONFIG.current_price_outer_color)
            .width(PLOT_CONFIG.current_price_outer_width)
            .style(egui_plot::LineStyle::dashed_loose());
        plot_ui.hline(outer_line);
        // Draw inner colored line on top
        let inner_line = HLine::new("Current Price", price)
            .color(PLOT_CONFIG.current_price_color)
            .width(PLOT_CONFIG.current_price_line_width)
            .style(egui_plot::LineStyle::dashed_loose());
        plot_ui.hline(inner_line);
    }
}

/// Draw all classified zones from a TradingModel
fn draw_classified_zones(
    plot_ui: &mut egui_plot::PlotUi,
    model: &TradingModel,
    x_min: f64,
    x_max: f64,
) {
    // Draw sticky superzones (aggregated consolidation areas) - only if enabled
    if PLOT_CONFIG.show_sticky_zones {
        for superzone in &model.zones.sticky_superzones {
            draw_superzone(
                plot_ui,
                superzone,
                x_min,
                x_max,
                "Sticky",
                PLOT_CONFIG.sticky_zone_color,
            );
        }
    }

    // Draw slippy superzones (aggregated low activity areas) - only if enabled
    if PLOT_CONFIG.show_slippy_zones {
        for superzone in &model.zones.slippy_superzones {
            draw_superzone(
                plot_ui,
                superzone,
                x_min,
                x_max,
                "Slippy",
                PLOT_CONFIG.slippy_zone_color,
            );
        }
    }

    // Draw low wick (reversal) superzones (aggregated rejection areas) - only if enabled
    if PLOT_CONFIG.show_low_wicks_zones {
        for superzone in &model.zones.low_wicks_superzones {
            draw_superzone(
                plot_ui,
                superzone,
                x_min,
                x_max,
                "Low Wick(Reversal)",
                PLOT_CONFIG.low_wicks_zone_color,
            );
        }
    }

    // Draw high wick (reversal) superzones (aggregated rejection areas) - only if enabled
    if PLOT_CONFIG.show_high_wicks_zones {
        for superzone in &model.zones.high_wicks_superzones {
            draw_superzone(
                plot_ui,
                superzone,
                x_min,
                x_max,
                "High Wick(Reversal)",
                PLOT_CONFIG.high_wicks_zone_color,
            );
        }
    }

    // Draw support/resistance superzones LAST (on top of sticky zones for visibility)
    if PLOT_CONFIG.show_support_zones {
        for superzone in &model.zones.support_superzones {
            draw_superzone(
                plot_ui,
                superzone,
                x_min,
                x_max,
                "SR: Support",
                PLOT_CONFIG.support_zone_color,
            );
        }
    }

    if PLOT_CONFIG.show_resistance_zones {
        for superzone in &model.zones.resistance_superzones {
            draw_superzone(
                plot_ui,
                superzone,
                x_min,
                x_max,
                "SR: Resistance",
                PLOT_CONFIG.resistance_zone_color,
            );
        }
    }
}

/// Draw a SuperZone from TradingModel as a semi-transparent filled rectangle
fn draw_superzone(
    plot_ui: &mut egui_plot::PlotUi,
    superzone: &SuperZone,
    x_min: f64,
    x_max: f64,
    label: &str,
    color: Color32,
) {
    let zone_bottom = superzone.price_bottom;
    let zone_top = superzone.price_top;

    // Create rectangle vertices (spanning full X-axis)
    let points = PlotPoints::new(vec![
        [x_min, zone_bottom],
        [x_max, zone_bottom],
        [x_max, zone_top],
        [x_min, zone_top],
    ]);

    // Draw semi-transparent filled polygon with matching stroke
    // Each superzone gets a unique identifier and a unique legend entry
    let polygon = Polygon::new(format!("{} #{}", label, superzone.id), points)
        .fill_color(color.linear_multiply(PLOT_CONFIG.zone_fill_opacity_pct))
        .stroke(Stroke::new(1.0, color))
        .name(format!("{} #{}", label, superzone.id)) // Unique legend entry per superzone
        .highlight(false)
        .allow_hover(false);

    plot_ui.polygon(polygon);
}
