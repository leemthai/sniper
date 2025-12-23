use std::hash::{Hash, Hasher};

use colorgrad::Gradient;
use eframe::egui::{Color32, Ui, Vec2b};
use egui_plot::{AxisHints, HPlacement, Plot, GridMark, VPlacement, Axis};

use crate::config::plot::PLOT_CONFIG;

use crate::engine::SniperEngine;

use crate::models::OhlcvTimeSeries;
use crate::models::cva::{CVACore, ScoreType};
use crate::models::trading_view::TradingModel;
use crate::models::timeseries::find_matching_ohlcv;

use crate::ui::app::PlotVisibility;
use crate::ui::app::CandleResolution;

use crate::ui::ui_text::UI_TEXT;

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

// Helper: Calculate a human-friendly step size (1, 2, 5, 10, 20, 50...)
fn calculate_adaptive_step(range: f64, target_count: f64) -> f64 {
    let raw_step = range / target_count.max(1.0);
    // Find magnitude (power of 10)
    let mag = 10.0_f64.powi(raw_step.log10().floor() as i32);
    let normalized = raw_step / mag; // Scale to 1.0 .. 10.0

    // Snap to "Nice" integers
    let nice_step = if normalized < 1.5 { 1.0 }
                   else if normalized < 3.0 { 2.0 }
                   else if normalized < 7.0 { 5.0 }
                   else { 10.0 };
    
    let result = nice_step * mag;
    
    // Ensure we never step less than 1 visual unit (1 candle)
    result.max(1.0)
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

    AxisHints::new(Axis::X)
        .label("Time")
        .formatter(move |mark, _range| {
            let visual_x = mark.value;
            let mut current_visual_start = 0.0;
            
            for seg in &segments {
                let seg_len_vis = ((seg.end_idx - seg.start_idx) as f64 / step_size as f64).ceil();
                let current_visual_end = current_visual_start + seg_len_vis;

                if visual_x >= current_visual_start && visual_x < current_visual_end {
                    let offset = (visual_x - current_visual_start) * step_size as f64;
                    let raw_idx = seg.start_idx + offset as usize;

                    // --- DEBUG PROBE ---
                    #[cfg(debug_assertions)]
                    if step_size > 200 && raw_idx >= seg.end_idx {
                         // We are trying to read Index 1000 in a segment of size 500
                         // Use a rate limiter or simple check to avoid spam, or just log
                         // Since axis formatter runs often, maybe only log specific X values or specific error conditions?
                         // Let's just log once per frame if possible, or use a trick.
                         // For now, let's just log:
                         // log::warn!("AXIS OVERSHOOT: Seg len {}, Step {}, VisualOffset {:.2} -> RawOffset {}. EXCEEDS LIMIT!", 
                         //    seg.end_idx - seg.start_idx, step_size, local_offset, raw_offset);
                         
                         return "GAP (Overshoot)".to_string(); // Change text to confirm diagnosis on screen
                    }
                    // ------------------

                    // Safety Clamp to segment end (Prevent over-reading into next segment data)
                    if raw_idx < seg.end_idx && raw_idx < timestamps.len() {
                        return TimeUtils::epoch_ms_to_date_string(timestamps[raw_idx]);
                    } else {
                        // This happens if visual_x is at the very fractional edge of a segment
                        //  return String::new(); 
                         return "EDGE".to_string(); 
                    }

                }
                current_visual_start = current_visual_end + gap_width;
                // If visual_x is here, it is in a gap
                if visual_x < current_visual_start {
                    // MARK GAPS EXPLICITLY
                    return "GAP".to_string(); 
                }
            }

            // --- DEBUG 3D/1W ISSUES ---
            // If we fall through here, visual_x is beyond all segments.
            // Uncomment to debug if 3D is generating out-of-bounds X values.
            log::warn!("Axis Formatter OOB: {:.2}", visual_x);
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


    // Ignores historical outliers to prevent compression.
    // Helper: Calculates Y-Axis bounds based on PH and Live Price (Sniper View)
    fn calculate_y_bounds(
        &self,
        cva_results: &CVACore,
        current_price_opt: Option<f64>,
    ) -> std::ops::RangeInclusive<f64> {
        // 1. Get Global Context (PH Bounds)
        let (ph_min, ph_max) = cva_results.price_range.min_max();
        
        // FIX: Use the argument 'current_price_opt'
        let current_price = current_price_opt.unwrap_or(ph_min);

        // 2. Union: Show PH Zone + Current Price
        // We explicitly EXCLUDE segment data bounds to prevent "Depeg/Crash" history 
        // from compressing the view.
        let final_min = ph_min.min(current_price);
        let final_max = ph_max.max(current_price);

        // 3. Apply Configured Padding
        let range = final_max - final_min;
        let pad = range * PLOT_CONFIG.plot_y_padding_pct;
        
        (final_min - pad).max(0.0)..=(final_max + pad)
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
            current_pair_price,
        );
        let y_min_vis = *y_bounds_range.start();

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
                let step = calculate_adaptive_step(max-min, 8.0);

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
                 
                 // FIX 1: Use PH Bounds (Inner) instead of Visual Bounds (Outer)
                 // This ensures the top/bottom labels are slightly inside the plot area.
                 let (ph_min, ph_max) = cva_results.price_range.min_max();
                 let ph_range = ph_max - ph_min;

                 // Mandatory Ends
                 marks.push(GridMark { value: ph_min, step_size: ph_range });
                 marks.push(GridMark { value: ph_max, step_size: ph_range });
                 
                 // Intermediates
                 let divisions = 5;
                 let step = ph_range / divisions as f64;
                 for i in 1..divisions {
                     marks.push(GridMark { value: y_min_vis + (step * i as f64), step_size: step });
                 }
                 marks
            })
            .allow_double_click_reset(false)
            .allow_scroll(false)
            .allow_drag(Vec2b { x: false, y: true })
            .allow_zoom(Vec2b { x: false, y: true })

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
