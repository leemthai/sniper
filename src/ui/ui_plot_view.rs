use std::hash::{Hash, Hasher};

use colorgrad::Gradient;
use eframe::egui::{Color32, Ui};
use egui_plot::{AxisHints, HPlacement, Plot, GridMark, VPlacement};

use crate::config::plot::PLOT_CONFIG;

use crate::engine::SniperEngine;

use crate::models::OhlcvTimeSeries;
use crate::models::cva::{CVACore, ScoreType};
use crate::models::trading_view::TradingModel;
use crate::models::timeseries::find_matching_ohlcv;

use crate::ui::app::PlotVisibility;
use crate::ui::app::CandleResolution;

use crate::ui::ui_text::UI_TEXT;

use crate::analysis::range_gap_finder::DisplaySegment;

use crate::ui::utils::format_price;
use crate::utils::maths_utils;
use crate::utils::TimeUtils;

// Import the new Layer System
use crate::ui::plot_layers::{
    BackgroundLayer, LayerContext, PlotLayer, PriceLineLayer, ReversalZoneLayer, StickyZoneLayer, CandlestickLayer, HorizonLinesLayer,
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

// Helper to build the Time Axis with smart spacing and formatting
fn create_time_axis(
    model: &TradingModel,
    ohlcv: &OhlcvTimeSeries,
    resolution: CandleResolution,
) -> AxisHints<'static> {
    let segments = model.segments.clone();
    let timestamps = ohlcv.timestamps.clone();
    let gap_width = PLOT_CONFIG.segment_gap_width;
    let step_size = resolution.step_size();

    AxisHints::new(egui_plot::Axis::X)
        .label("Time")
        .formatter(move |mark, _range| {
            let visual_x = mark.value;
            let mut current_visual_start = 0.0;
            
            for seg in &segments {
                let seg_len_vis = ((seg.end_idx - seg.start_idx) as f64 / step_size as f64).ceil();
                let current_visual_end = current_visual_start + seg_len_vis;

                if visual_x >= current_visual_start && visual_x < current_visual_end {
                    let offset = (visual_x - current_visual_start) * step_size as f64;
                    let idx = seg.start_idx + offset as usize;
                    if idx < timestamps.len() {
                        return TimeUtils::epoch_ms_to_date_string(timestamps[idx]);
                    }
                }
                current_visual_start = current_visual_end + gap_width;
            }
            String::new()
        })
        .placement(VPlacement::Bottom)
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

    // Helper: Calculates the X-Axis bounds (0..TotalWidth) or (Start..End) based on Time Machine state
fn calculate_view_bounds(
        &self,
        model: &TradingModel,
        current_segment_idx: Option<usize>,
        resolution: CandleResolution, // <--- NEW ARG
    ) -> (f64, f64, f64) {
        let gap_size = PLOT_CONFIG.segment_gap_width;
        let step_size = resolution.step_size();

        // Helper to calc visual width of a candle count
        // Integer division with ceiling (any remainder needs a partial candle space)
        let calc_width = |count: usize| -> f64 {
            (count as f64 / step_size as f64).ceil()
        };

        let total_visual_candles: f64 = model.segments.iter()
            .map(|s| calc_width(s.candle_count))
            .sum();
            
        let gap_count = model.segments.len().saturating_sub(1);
        let total_visual_width = total_visual_candles + (gap_count as f64 * gap_size);

        if let Some(target_idx) = current_segment_idx {
            if target_idx < model.segments.len() {
                let mut start_x = 0.0;
                for i in 0..target_idx {
                    start_x += calc_width(model.segments[i].candle_count);
                    start_x += gap_size;
                }
                let width = calc_width(model.segments[target_idx].candle_count);
                return (start_x, start_x + width, total_visual_width);
            }
        }
        (0.0, total_visual_width, total_visual_width)
    }


    // Helper: Calculates Y-Axis bounds based on PH, Live Price, and Visible Segments
    fn calculate_y_bounds(
        &self,
        cva_results: &CVACore,
        model: &TradingModel,
        current_pair_price: Option<f64>,
        current_segment_idx: Option<usize>,
    ) -> std::ops::RangeInclusive<f64> {
        // 1. Get Global Context (PH Bounds + Live Price)
        let (ph_min, ph_max) = cva_results.price_range.min_max();
        let current_price = current_pair_price.unwrap_or(ph_min);

        // 2. Calculate Actual Data Bounds (Excursions)
        let mut data_min = f64::MAX;
        let mut data_max = f64::MIN;

        let mut check_segment = |seg: &DisplaySegment| {
            if seg.low_price < data_min { data_min = seg.low_price; }
            if seg.high_price > data_max { data_max = seg.high_price; }
        };

        if let Some(target_idx) = current_segment_idx {
            // Time Machine: Only check the specific segment
            if let Some(seg) = model.segments.get(target_idx) {
                check_segment(seg);
            }
        } else {
            // Show All: Check all segments
            for seg in &model.segments {
                check_segment(seg);
            }
        }
        
        // Safety fallback if segments are empty
        if data_min == f64::MAX { 
            data_min = ph_min; 
            data_max = ph_max; 
        }

        // 3. Union: Show PH Zone + Actual Data + Current Price
        let final_min = ph_min.min(data_min).min(current_price);
        let final_max = ph_max.max(data_max).max(current_price);

        // 4. Apply Configured Padding
        let range = final_max - final_min;
        let pad = range * crate::config::plot::PLOT_CONFIG.plot_y_padding_pct;
        
        (final_min - pad)..=(final_max + pad)
    }


    pub fn show_my_plot(
        &mut self,
        ui: &mut Ui,
        cva_results: &CVACore,
        trading_model: &TradingModel,
        current_pair_price: Option<f64>,
        background_score_type: ScoreType,
        visibility: &PlotVisibility,
        engine: &SniperEngine,
        resolution: CandleResolution,
        current_segment_idx: Option<usize>,
    ) {
        // 1. Fetch OHLCV Data (Required for Candle Layer)
        // We assume the pair exists since we have a model for it.
        let ohlcv = find_matching_ohlcv(
            &engine.timeseries.series_data,
            &cva_results.pair_name,
            cva_results.interval_ms,
        )
        .expect("OHLCV data missing for current model");

        // 2. Calculate Bounds (Using Helper)
        let (view_min, view_max, total_visual_width) = self.calculate_view_bounds(trading_model, current_segment_idx, resolution);

        // 3. Calculate Visual Height (Y-Axis) -- MOVED UP
        // We do this BEFORE the plot so the grid spacer knows the real visual range
        let y_bounds_range = self.calculate_y_bounds(
            cva_results,
            trading_model,
            current_pair_price,
            current_segment_idx,
        );
        let y_min_vis = *y_bounds_range.start();
        let y_max_vis = *y_bounds_range.end();
        let total_y_range = y_max_vis - y_min_vis;

        // 1. Calculate Data (Background Bars)
        let cache = self.calculate_plot_data(cva_results, background_score_type);

        let time_axis = create_time_axis(trading_model, ohlcv, resolution);


        Plot::new("my_plot")
            // .custom_x_axes(vec![create_x_axis(&cache)])
            .custom_x_axes(vec![time_axis])
            .custom_y_axes(vec![create_y_axis(&cva_results.pair_name)])
            .label_formatter(|_, _| String::new())
            .x_grid_spacer(move |input| {
                let mut marks = Vec::new();
                let (min, max) = input.bounds;
                let step = 50.0; // Draw a vertical line every 50 visual candles
                
                let start = (min / step).ceil() as i64;
                let end = (max / step).floor() as i64;
                
                for i in start..=end {
                    let value = i as f64 * step;
                    marks.push(GridMark { value, step_size: step });
                }
                marks
            })
            .y_grid_spacer(move |_input| {
                 let mut marks = Vec::new();
                 // Mandatory Ends (Visible Area, not just PH)
                 marks.push(GridMark { value: y_min_vis, step_size: total_y_range });
                 marks.push(GridMark { value: y_max_vis, step_size: total_y_range });
                 
                 // Intermediates
                 let divisions = 5;
                 let step = total_y_range / divisions as f64;
                 for i in 1..divisions {
                     marks.push(GridMark { value: y_min_vis + (step * i as f64), step_size: step });
                 }
                 marks
            })
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_boxed_zoom(false)
            .allow_double_click_reset(false)
            .show(ui, |plot_ui| {
                
                plot_ui.set_plot_bounds_x(view_min..=view_max);
                plot_ui.set_plot_bounds_y(y_bounds_range);

                // 1. Get the STRICT Price Horizon limits for the "Ghosting" logic
                let (ph_min, ph_max) = cva_results.price_range.min_max();


                // --- LAYER STACK ---
                let ctx = LayerContext {
                    trading_model: trading_model,
                    ohlcv: ohlcv,
                    cache: &cache,
                    visibility,
                    background_score_type,
                    x_min: 0.0,
                    x_max: total_visual_width,
                    current_price: current_pair_price,
                    resolution: resolution,
                    ph_bounds: (ph_min, ph_max),
                };

                // 2. Define Layer Stack (Dynamic)
                let mut layers: Vec<Box<dyn PlotLayer>> = Vec::with_capacity(5);

                if visibility.background {
                    layers.push(Box::new(BackgroundLayer));
                }
                if visibility.sticky {
                    layers.push(Box::new(StickyZoneLayer));
                }
                if visibility.low_wicks || visibility.high_wicks {
                    layers.push(Box::new(ReversalZoneLayer));
                }
                if visibility.price_line {
                    layers.push(Box::new(PriceLineLayer));
                }

                // NEW: Horizon Lines (Dashed PH boundaries)
                if visibility.horizon_lines {
                    layers.push(Box::new(HorizonLinesLayer));
                }
                
                // CANDLES ON TOP
                // Note: 'ghost_candles' is handled internally by CandlestickLayer
                if visibility.candles { 
                    layers.push(Box::new(CandlestickLayer)); 
                }

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

        crate::trace_time!("Rebuild Plot Cache", 500, {
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
                    let dimmed_color =
                        color.linear_multiply(PLOT_CONFIG.background_bar_intensity_pct);

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
        })
    }
}

// Helpers retained locally for calculate_plot_data
fn to_egui_color(colorgrad_color: colorgrad::Color) -> Color32 {
    let rgba8 = colorgrad_color.to_rgba8();
    Color32::from_rgba_unmultiplied(rgba8[0], rgba8[1], rgba8[2], 255)
}

// fn create_x_axis(_plot_cache: &PlotCache) -> AxisHints<'static> {
//     AxisHints::new_x()
//         .label(UI_TEXT.plot_x_axis)
//         .formatter(move |grid_mark, _range| {
//             let pct = grid_mark.value * 100.0;
//             format!("{:.0}%", pct)
//         })
// }

fn create_y_axis(pair_name: &str) -> AxisHints<'static> {
    let label = format!("{}  {}", pair_name, UI_TEXT.plot_y_axis);
    AxisHints::new_y()
        .label(label)
        .formatter(|grid_mark, _range| format!("{}", format_price(grid_mark.value)))
        .placement(HPlacement::Right)
}
