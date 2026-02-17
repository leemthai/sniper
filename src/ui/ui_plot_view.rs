use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

use colorgrad::Gradient;
use eframe::egui::{Color32, PointerButton, Rect, Ui, Vec2b};
use egui_plot::{
    Axis, AxisHints, GridInput, GridMark, HPlacement, Plot, PlotPoint, PlotUi, VPlacement,
};

use crate::analysis::range_gap_finder::DisplaySegment;

use crate::config::plot::PLOT_CONFIG;
use crate::config::{CandleResolution, Price, PriceLike};

use crate::engine::SniperEngine;

use crate::models::{CVACore, ScoreType, TradeOpportunity, TradingModel, find_matching_ohlcv};

use crate::ui::ui_text::UI_TEXT;

use crate::utils::TimeUtils;
use crate::utils::maths_utils;

// Import the new Layer System
use crate::ui::plot_layers::{
    BackgroundLayer, CandlestickLayer, HorizonLinesLayer, LayerContext, OpportunityLayer,
    PlotLayer, PriceLineLayer, ReversalZoneLayer, SegmentSeparatorLayer, StickyZoneLayer,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct PlotVisibility {
    pub sticky: bool,
    pub low_wicks: bool,
    pub high_wicks: bool,
    pub background: bool,
    pub price_line: bool,
    pub candles: bool,
    pub opportunities: bool,

    pub horizon_lines: bool,
    pub separators: bool,
}

impl Default for PlotVisibility {
    fn default() -> Self {
        Self {
            sticky: true,
            low_wicks: false,
            high_wicks: false,
            background: true,
            price_line: true,
            candles: true,
            opportunities: true,
            horizon_lines: true,
            separators: true,
        }
    }
}

/// A lightweight representation of a background bar.
#[derive(Clone)]
pub(crate) struct BackgroundBar {
    pub x_max: f64,
    pub y_center: f64,
    pub height: f64,
    pub color: Color32,
}

#[derive(Clone)]
pub(crate) struct PlotCache {
    pub cva_hash: u64,
    pub bars: Vec<BackgroundBar>,
}

#[derive(Default)]
pub(crate) struct PlotView {
    cache: Option<PlotCache>,
}

// Helper: Calculate a human-friendly step size (1, 2, 5, 10, 20, 50...)
fn calculate_adaptive_step(range: f64, target_count: f64) -> f64 {
    let raw_step = range / target_count.max(1.0);
    let mag = 10.0_f64.powi(raw_step.log10().floor() as i32);
    let normalized = raw_step / mag;

    let nice_step = if normalized < 1.5 {
        1.0
    } else if normalized < 3.0 {
        2.0
    } else if normalized < 7.0 {
        5.0
    } else {
        10.0
    };

    nice_step * mag
}

// Helper to build the Time Axis with smart spacing and formatting
fn create_time_axis(
    model: &TradingModel,
    // _ohlcv: &OhlcvTimeSeries, // Unused for math now, but kept for signature
    resolution: CandleResolution,
) -> AxisHints<'static> {
    let segments = model.segments.clone();
    let gap_width = PLOT_CONFIG.segment_gap_width;

    let agg_interval_ms = resolution.duration().as_millis() as i64;

    AxisHints::new(Axis::X)
        .label(&UI_TEXT.plot_x_axis)
        .formatter(move |mark, _range| {
            let visual_x = mark.value;
            let mut current_visual_start = 0.0;

            for seg in &segments {
                // Calc Width using Timestamp Buckets (Matches Render Logic)
                // Integer division snaps timestamps to the grid (e.g. Daily buckets)
                let start_bucket = seg.start_ts / agg_interval_ms;
                let end_bucket = seg.end_ts / agg_interval_ms;
                let seg_len_vis = (end_bucket - start_bucket + 1) as f64;

                let current_visual_end = current_visual_start + seg_len_vis;

                if visual_x >= current_visual_start && visual_x < current_visual_end {
                    // Calculate which bucket we are hovering over
                    let local_offset = (visual_x - current_visual_start).floor() as i64;

                    // Reconstruct the Timestamp for this bucket
                    let bucket_ts = (start_bucket + local_offset) * agg_interval_ms;

                    return TimeUtils::epoch_ms_to_date_string(bucket_ts);
                }

                current_visual_start = current_visual_end + gap_width;

                if visual_x < current_visual_start {
                    return UI_TEXT.plot_x_axis_gap.to_string();
                }
            }
            String::new()
        })
        .placement(VPlacement::Bottom)
}

pub(crate) enum PlotInteraction {
    None,
    UserInteracted, // User dragged/zoomed
    RequestReset,   // User double-clicked
}

impl PlotView {
    pub(crate) fn new() -> Self {
        Self { cache: None }
    }

    pub(crate) fn show_my_plot(
        &mut self,
        ui: &mut Ui,
        cva_results: &CVACore,
        trading_model: &TradingModel,
        current_pair_price: Option<Price>,
        background_score_type: ScoreType,
        visibility: &PlotVisibility,
        engine: &SniperEngine,
        resolution: CandleResolution,
        current_segment_idx: Option<usize>,
        auto_scale_y: bool,
        selected_opportunity: Option<TradeOpportunity>,
    ) -> PlotInteraction {
        // Fetch OHLCV Data (Required for Candle Layer)
        // We assume the pair exists since we have a model for it.
        let ts_guard = engine.timeseries.read().unwrap();
        let ohlcv = find_matching_ohlcv(
            &ts_guard.series_data,
            &cva_results.pair_name,
            cva_results.interval_ms,
        )
        .expect(&UI_TEXT.plot_missing_klines);

        // Calculate Bounds (Using Helper)
        let (view_min, view_max, total_visual_width) =
            self.calculate_view_bounds(trading_model, current_segment_idx, resolution);

        // Y-Axis: CONDITIONAL LOCK
        // Calculate Visual Height (Y-Axis) -- MOVED UP
        // We do this BEFORE the plot so the grid spacer knows the real visual range
        let y_bounds_range = self.calculate_y_bounds(cva_results, current_pair_price);

        // Calculate Data (Background Bars)
        let cache = self.calculate_plot_data(cva_results, background_score_type);

        // Extract PH bounds for the grid spacer
        let (ph_min, ph_max) = cva_results.price_range.min_max();

        let time_axis = create_time_axis(trading_model, resolution);
        let price_axis = create_y_axis(&cva_results.pair_name);

        let plot_response = Plot::new("my_plot")
            // .custom_x_axes(vec![create_x_axis(&cache)])
            .custom_x_axes(vec![time_axis])
            .custom_y_axes(vec![price_axis])
            .label_formatter(|_, _| String::new())
            .x_grid_spacer(Self::generate_x_marks)
            .y_grid_spacer(move |input| Self::generate_y_marks(input, ph_min, ph_max))
            .allow_scroll(false)
            .allow_boxed_zoom(false) // Not allowed because it alters both y and x. x is not allowed coz fixed.
            .allow_double_click_reset(false)
            .allow_drag(Vec2b { x: false, y: true })
            .allow_zoom(Vec2b { x: false, y: true })
            .show(ui, |plot_ui| {
                let width = view_max - view_min;
                let safe_width = width.max(10.0); // Safetey: If width is 0 (empty dat), default to small pad
                let pad_x = safe_width * PLOT_CONFIG.plot_x_padding_pct;
                // Set Bounds with Padding. This pushes the view slightly negative (left) and positive (right)
                plot_ui.set_plot_bounds_x((view_min - pad_x)..=(view_max + pad_x));

                if auto_scale_y {
                    plot_ui.set_plot_bounds_y(y_bounds_range);
                } else {
                    Self::enforce_manual_safety_limits(
                        plot_ui,
                        current_pair_price.unwrap_or_default(),
                    );
                }

                // Get the STRICT Price Horizon limits for the "Ghosting" logic
                let (ph_min, ph_max) = cva_results.price_range.min_max();

                // FIX: Calculate STRICT clip rect based on visible plot bounds.
                // This excludes axes labels and margins.
                let bounds = plot_ui.plot_bounds();
                let min = bounds.min();
                let max = bounds.max();

                // Map Data Corners to Screen Pixels
                // Top-Left in Data is (min_x, max_y) -> Screen (x, y)
                let p1 = plot_ui.screen_from_plot(PlotPoint::new(min[0], max[1]));
                // Bottom-Right in Data is (max_x, min_y) -> Screen (x, y)
                let p2 = plot_ui.screen_from_plot(PlotPoint::new(max[0], min[1]));

                let clip_rect = Rect::from_min_max(p1, p2);

                // --- LAYER STACK ---
                let ctx = LayerContext {
                    trading_model,
                    ohlcv,
                    cache: &cache,
                    visibility,
                    // background_score_type,
                    x_min: 0.0,
                    x_max: total_visual_width,
                    current_price: current_pair_price,
                    resolution,
                    ph_bounds: (Price::new(ph_min), Price::new(ph_max)),
                    clip_rect,
                    selected_opportunity: &selected_opportunity,
                };

                // Define Layer Stack (Dynamic)
                let mut layers: Vec<Box<dyn PlotLayer>> = Vec::with_capacity(7); // '7' is basically a hint. If require  more capacity, Rust will allocate at run-time np.

                // LOGIC: Only show Global Context layers (Volume/Zones) if we are viewing the FULL HISTORY ("Show All").
                // If viewing a specific segment, leave these out as not relevant.
                let is_show_all = current_segment_idx.is_none();

                // Global Context Layers (Only in Show All)
                if is_show_all {
                    if visibility.background {
                        layers.push(Box::new(BackgroundLayer));
                    }
                    if visibility.sticky {
                        layers.push(Box::new(StickyZoneLayer));
                    }
                    if visibility.low_wicks || visibility.high_wicks {
                        layers.push(Box::new(ReversalZoneLayer));
                    }
                    // Overlays (Separators)
                    if visibility.separators {
                        layers.push(Box::new(SegmentSeparatorLayer));
                    }
                }

                // Always Available Layers (Context Agnostic)
                if visibility.price_line {
                    layers.push(Box::new(PriceLineLayer));
                }

                // Horizon Lines (Dashed PH boundaries)
                if visibility.horizon_lines {
                    layers.push(Box::new(HorizonLinesLayer));
                }

                // Candles. Note: 'ghost_candles' is handled internally by CandlestickLayer
                if visibility.candles {
                    layers.push(Box::new(CandlestickLayer));
                }

                // Top Layer: Sniping Overlays
                if visibility.opportunities {
                    layers.push(Box::new(OpportunityLayer));
                }

                // Render Loop
                for layer in layers {
                    layer.render(plot_ui, &ctx);
                }
            });

        let r = plot_response.response;
        // Double Click -> Reset (Lock)
        if r.double_clicked() {
            return PlotInteraction::RequestReset;
        }

        // Dragging -> Break Lock (Unlock)
        // Note: we explicitly check if Y drag is allowed by config,
        // though we hardcoded it in Plot::new anyway.
        if r.dragged_by(PointerButton::Primary) || r.dragged_by(PointerButton::Secondary) {
            // Only trigger if we actually moved in Y to avoid accidental clicks?
            // Actually, any drag intent should unlock it.
            return PlotInteraction::UserInteracted;
        }

        // Zooming (Scroll) -> Break Lock
        if r.hovered() && ui.input(|i| i.raw_scroll_delta.y.abs() > 0.0) {
            return PlotInteraction::UserInteracted;
        }

        PlotInteraction::None
    }

    fn calculate_view_bounds(
        &self,
        model: &TradingModel,
        current_segment_idx: Option<usize>,
        resolution: CandleResolution,
    ) -> (f64, f64, f64) {
        let gap_size = PLOT_CONFIG.segment_gap_width;
        let agg_interval_ms = resolution.duration().as_millis() as i64;

        // Helper: Calculate visual width using UTC Grid logic
        // This must match CandlestickLayer logic exactly.
        let calc_width = |seg: &DisplaySegment| -> f64 {
            // Get timestamps of first and last candle in segment
            // Note: DisplaySegment stores start_ts and end_ts.
            // end_ts is the timestamp of the last candle (inclusive).

            let start_bucket = seg.start_ts / agg_interval_ms;
            let end_bucket = seg.end_ts / agg_interval_ms;

            // The number of visual bars is the number of buckets spanned
            let buckets = end_bucket - start_bucket + 1;
            buckets as f64
        };

        let total_visual_candles: f64 = model.segments.iter().map(&calc_width).sum();

        let gap_count = model.segments.len().saturating_sub(1);
        let total_visual_width = total_visual_candles + (gap_count as f64 * gap_size);

        if let Some(target_idx) = current_segment_idx {
            if target_idx < model.segments.len() {
                let mut start_x = 0.0;
                for i in 0..target_idx {
                    start_x += calc_width(&model.segments[i]);
                    start_x += gap_size;
                }
                let width = calc_width(&model.segments[target_idx]);
                return (start_x, start_x + width, total_visual_width);
            }
        }
        (0.0, total_visual_width, total_visual_width)
    }

    fn calculate_y_bounds(
        // Helper: Calculates Y-Axis bounds based on PH and Live Price
        &self,
        cva_results: &CVACore,
        current_price_opt: Option<Price>,
    ) -> std::ops::RangeInclusive<f64> {
        let (ph_min, ph_max) = cva_results.price_range.min_max();
        let current_price = current_price_opt.unwrap_or_default();

        // Calculate Standard Union (PH + Price)
        // We intentionally ignore model.segments for the *Final* calculation to keep Sniper View,
        // but we calculate them below for the Debug Log you requested.
        let final_min = ph_min.min(current_price.value());
        let final_max = ph_max.max(current_price.value());

        // Apply Configured Padding
        let range = final_max - final_min;
        let pad = range * PLOT_CONFIG.plot_y_padding_pct;

        ((final_min - pad).max(0.0))..=(final_max + pad)
    }

    fn generate_x_marks(input: GridInput) -> Vec<GridMark> {
        let mut marks = Vec::new();
        let (min, max) = input.bounds;
        let range = max - min;

        let step = calculate_adaptive_step(range, 8.0);

        let start = (min / step).ceil() as i64;
        let end = (max / step).floor() as i64;

        for i in start..=end {
            let value = i as f64 * step;
            marks.push(GridMark {
                value,
                step_size: step,
            });
        }
        marks
    }

    fn generate_y_marks(input: egui_plot::GridInput, _ph_min: f64, _ph_max: f64) -> Vec<GridMark> {
        // Helper: Generates Y-Axis grid marks (Price)
        // FIX: Use input.bounds (Visible Area) instead of PH bounds to ensure
        // ticks always cover the screen, preventing the axis from vanishing.
        let mut marks = Vec::new();
        let (min, max) = input.bounds; // Visible range
        let range = max - min;

        // Use the adaptive step logic so we always get ~8 ticks
        // (Ensure calculate_adaptive_step is available in this scope)
        let step = calculate_adaptive_step(range, 8.0);

        let start = (min / step).ceil() as i64;
        let end = (max / step).floor() as i64;

        for i in start..=end {
            let value = i as f64 * step;
            marks.push(GridMark {
                value,
                step_size: step,
            });
        }

        // Optional: If you still strictly want PH bounds labeled, push them explicitly.
        // But standard grid lines usually look cleaner.
        // marks.push(GridMark { value: _ph_min, step_size: step });
        // marks.push(GridMark { value: _ph_max, step_size: step });

        marks
    }

    fn enforce_manual_safety_limits(plot_ui: &mut PlotUi, current_price: Price) {
        // Helper: Enforces sane zoom/pan limits when the user is in Manual Mode

        let bounds = plot_ui.plot_bounds();
        let mut min = *bounds.range_y().start();
        let mut max = *bounds.range_y().end();
        let mut range = max - min;
        let mut changed = false;

        let base_price = current_price.value().max(1.0);

        // ZOOM LIMITS (Range)
        let min_allowed_range = base_price * 0.00001; // 0.001%
        let max_allowed_range = base_price * 2.0; // 200% (View 0 to 180k for BTC)

        if range < min_allowed_range {
            let center = (min + max) / 2.0;
            range = min_allowed_range;
            min = center - range / 2.0;
            max = center + range / 2.0;
            changed = true;
        } else if range > max_allowed_range {
            let center = (min + max) / 2.0;
            range = max_allowed_range;
            min = center - range / 2.0;
            max = center + range / 2.0;
            changed = true;
        }

        // PAN LIMITS (Position)

        // A. Hard Floor: Bottom cannot be negative
        if min < 0.0 {
            let diff = 0.0 - min;
            min += diff;
            max += diff;
            changed = true;
        }

        // B. Hard Ceiling: Top cannot exceed 5x Current Price
        // This stops you from dragging into "Millions" territory on a $100 coin.
        let hard_ceiling = base_price * 5.0;

        if max > hard_ceiling {
            let diff = max - hard_ceiling;
            min -= diff;
            max -= diff;
            changed = true;
        }

        // Apply if we hit any bumper
        if changed {
            plot_ui.set_plot_bounds_y(min..=max);
        }
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

fn create_y_axis(pair_name: &str) -> AxisHints<'static> {
    let label = format!("{}  {}", pair_name, UI_TEXT.plot_y_axis);
    AxisHints::new_y()
        .label(label)
        .formatter(|mark, range| {
            //Calculate the Visible Span
            let span = range.end() - range.start();

            // Decide Precision based on Zoom Level (Span)
            // This ensures all labels share the same width/precision
            // regardless of their individual value.
            let decimals = if span >= 1000.0 {
                2 // Large range (e.g. BTC): $95,200.50
            } else if span >= 1.0 {
                4 // Medium range (e.g. SOL): $145.2050
            } else if span >= 0.001 {
                6 // Small range (e.g. Stable/Low Cap): $1.000200
            } else {
                8 // Micro range: $0.00004500
            };

            // Forces exactly 'decimals' places.
            format!("${:.1$}", mark.value, decimals)
        })
        .placement(HPlacement::Right)
}
