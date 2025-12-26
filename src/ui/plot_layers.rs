use eframe::egui::{Color32, Id, LayerId, Order::Tooltip, RichText, Stroke, Ui};

#[allow(deprecated)]
use eframe::egui::show_tooltip_at_pointer;

use egui_plot::{Line, LineStyle, PlotPoints, PlotUi, Polygon};

use crate::config::plot::PLOT_CONFIG;

use crate::models::OhlcvTimeSeries;
use crate::models::cva::ScoreType;
use crate::models::trading_view::{SuperZone, TradingModel};

use crate::ui::app::{CandleResolution, PlotVisibility};
use crate::ui::ui_plot_view::PlotCache;
use crate::ui::ui_text::UI_TEXT;
use crate::ui::utils::format_price;

pub struct HorizonLinesLayer;

impl PlotLayer for HorizonLinesLayer {
    fn render(&self, plot_ui: &mut PlotUi, ctx: &LayerContext) {
        let (ph_min, ph_max) = ctx.ph_bounds;
        // Use White for max contrast against dark backgrounds and grey grid lines. Make it slightly thicker so it is visible whatever is behind it
        let color = Color32::WHITE;
        let width = 4.0;
        let dash_style = LineStyle::Dashed { length: 10.0 };

        // Define Start/End based on Data Limits
        let x_start = ctx.x_min;
        let x_end = ctx.x_max;

        // Top Line
        plot_ui.line(
            Line::new(
                "",
                PlotPoints::new(vec![[x_start, ph_max], [x_end, ph_max]]),
            )
            .color(color)
            .style(dash_style)
            .width(width),
        );

        // Bottom Line (Segment)
        plot_ui.line(
            Line::new(
                "",
                PlotPoints::new(vec![[x_start, ph_min], [x_end, ph_min]]),
            )
            .color(color)
            .style(dash_style)
            .width(width),
        );
    }
}

pub struct CandlestickLayer;

fn draw_gap_separator(plot_ui: &mut PlotUi, x_pos: f64, gap_width: f64, y_bounds: (f64, f64)) {
    let (y_min, y_max) = y_bounds;
    let line_x = x_pos + (gap_width / 2.0);

    // Overshoot the bounds slightly to ensure the line always looks infinite vertical
    let range = y_max - y_min;
    let y_start = y_min - range;
    let y_end = y_max + range;

    plot_ui.line(
        Line::new(
            "",
            PlotPoints::new(vec![[line_x, y_start], [line_x, y_end]]),
        )
        .color(Color32::from_gray(60)) // Subtle gray
        .width(1.0)
        .style(LineStyle::Dashed { length: 5.0 }),
    );
}

impl PlotLayer for CandlestickLayer {
    fn render(&self, plot_ui: &mut PlotUi, ctx: &LayerContext) {
        if ctx.trading_model.segments.is_empty() {
            return;
        }

        let mut visual_x = 0.0;
        let gap_width = PLOT_CONFIG.segment_gap_width;
        let agg_interval_ms = ctx.resolution.interval_ms();

        let (y_min_global, y_max_global) = ctx.trading_model.cva.price_range.min_max();
        let y_bounds_separator = (y_min_global, y_max_global);

        for (seg_idx, segment) in ctx.trading_model.segments.iter().enumerate() {
            let mut i = segment.start_idx;

            while i < segment.end_idx {
                let first = ctx.ohlcv.get_candle(i);

                // --- UTC GRID ALIGNMENT ---
                let boundary_start = (first.timestamp_ms / agg_interval_ms) * agg_interval_ms;
                let boundary_end = boundary_start + agg_interval_ms;

                // --- AGGREGATION ---
                let open = first.open_price;
                let mut close = first.close_price;
                let mut high = first.high_price;
                let mut low = first.low_price;

                let mut next_i = i + 1;
                while next_i < segment.end_idx {
                    let c = ctx.ohlcv.get_candle(next_i);
                    if c.timestamp_ms >= boundary_end {
                        break;
                    }
                    high = high.max(c.high_price);
                    low = low.min(c.low_price);
                    close = c.close_price;
                    next_i += 1;
                }

                // --- DRAWING (Delegated to Helper) ---
                draw_split_candle(
                    plot_ui,
                    visual_x,
                    open,
                    high,
                    low,
                    close,
                    ctx.ph_bounds,
                    ctx.visibility.ghost_candles,
                    ctx.x_min,
                );

                visual_x += 1.0;
                i = next_i;
            }

            // Draw Gap Separator
            if seg_idx < ctx.trading_model.segments.len() - 1 {
                draw_gap_separator(plot_ui, visual_x, gap_width, y_bounds_separator);
                visual_x += gap_width;
            }
        }
    }
}

// --- HELPERS (Keep the main logic clean) ---
fn draw_split_candle(
    ui: &mut PlotUi,
    x: f64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    ph_bounds: (f64, f64),
    show_ghosts: bool,
    min_x_limit: f64,
) {
    let (ph_min, ph_max) = ph_bounds;
    let is_green = close >= open;

    let base_color = if is_green {
        PLOT_CONFIG.candle_bullish_color
    } else {
        PLOT_CONFIG.candle_bearish_color
    };

    // VISUAL FIX 2: Ghost Color
    // Make it look "Dead" / Desaturated.
    // We use a very high transparency (0.2) so it recedes into the background.
    let ghost_color = base_color.linear_multiply(0.2);

    // 1. Draw Wicks
    // Top Ghost
    if show_ghosts && high > ph_max {
        let bottom = low.max(ph_max);
        if high > bottom {
            draw_wick_line(ui, x, high, bottom, ghost_color, min_x_limit);
        }
    }
    // Bottom Ghost
    if show_ghosts && low < ph_min {
        let top = high.min(ph_min);
        if top > low {
            draw_wick_line(ui, x, top, low, ghost_color, min_x_limit);
        }
    }
    // Solid Wick
    let solid_top = high.min(ph_max);
    let solid_bot = low.max(ph_min);
    if solid_top > solid_bot {
        draw_wick_line(ui, x, solid_top, solid_bot, base_color, min_x_limit);
    }

    // 2. Draw Body
    let body_top_raw = open.max(close);
    let body_bot_raw = open.min(close);
    // Doji check
    let body_top = if (body_top_raw - body_bot_raw).abs() < f64::EPSILON {
        body_bot_raw * 1.0001
    } else {
        body_top_raw
    };
    let body_bot = body_bot_raw;

    // Top Ghost Body
    if show_ghosts && body_top > ph_max {
        let b = body_bot.max(ph_max);
        if body_top > b {
            draw_body_rect(ui, x, body_top, b, ghost_color, min_x_limit);
        }
    }
    // Bottom Ghost Body
    if show_ghosts && body_bot < ph_min {
        let t = body_top.min(ph_min);
        if t > body_bot {
            draw_body_rect(ui, x, t, body_bot, ghost_color, min_x_limit);
        }
    }
    // Solid Body
    let solid_body_top = body_top.min(ph_max);
    let solid_body_bot = body_bot.max(ph_min);
    if solid_body_top > solid_body_bot {
        draw_body_rect(
            ui,
            x,
            solid_body_top,
            solid_body_bot,
            base_color,
            min_x_limit,
        );
    }
}

#[inline]
fn draw_wick_line(ui: &mut PlotUi, x: f64, top: f64, bottom: f64, color: Color32, min_x: f64) {

    if x < min_x { return; } // clipping logic


    ui.line(
        Line::new("", PlotPoints::new(vec![[x, bottom], [x, top]]))
            .color(color)
            .width(PLOT_CONFIG.candle_wick_width),
    );
}

#[inline]
fn draw_body_rect(ui: &mut PlotUi, x: f64, top: f64, bottom: f64, color: Color32, min_x: f64) {
    let half_w = PLOT_CONFIG.candle_width_pct / 2.0;

    // CLIPPING LOGIC:
    // Ensure 'left' is never less than min_x (0.0)
    // If x=0 and width=0.8, left becomes 0.0 instead of -0.4.
    let left = (x - half_w).max(min_x);
    let right = x + half_w;

    // Safety: If clipping makes left >= right (fully out of bounds), don't draw
    if left >= right {
        return;
    }

    let pts = vec![
        [left, bottom], 
        [right, bottom],
        [right, top], 
        [left, top],
    ];
    

    ui.polygon(
        Polygon::new("", PlotPoints::new(pts))
            .fill_color(color)
            .stroke(eframe::egui::Stroke::NONE),
    );
}

/// Context passed to every layer during rendering.
/// This prevents argument explosion.
pub struct LayerContext<'a> {
    pub trading_model: &'a TradingModel,
    pub ohlcv: &'a OhlcvTimeSeries,
    pub cache: &'a PlotCache,
    pub visibility: &'a PlotVisibility,
    pub background_score_type: ScoreType,
    pub x_min: f64,
    pub x_max: f64,
    pub current_price: Option<f64>, // Pass SIM-aware price so layers render correctly in SIM mode
    pub resolution: CandleResolution,
    pub ph_bounds: (f64, f64), // (min, max) of the Price Horizon,
}

/// A standardized layer in the plot stack.
pub trait PlotLayer {
    fn render(&self, ui: &mut PlotUi, ctx: &LayerContext);
}

// ============================================================================
// 1. BACKGROUND LAYER (The Histogram)
// ============================================================================
pub struct BackgroundLayer;

impl PlotLayer for BackgroundLayer {
    fn render(&self, plot_ui: &mut PlotUi, ctx: &LayerContext) {
        // Use Data Bounds (0..Total), not View Bounds (includes padding)
        let x_start_data = ctx.x_min;
        let x_end_data = ctx.x_max;
        let data_width = x_end_data - x_start_data;

        if data_width <= f64::EPSILON {
            return;
        }

        for bar in &ctx.cache.bars {
            let half_h = bar.height / 2.0;

            // Map Score (0.0 .. 1.0) to Data Width
            // This stops the histogram at the exact edge of the candles, respecting the margin.
            let rect_x_start = x_start_data;
            let rect_x_end = x_start_data + (bar.x_max * data_width);

            // Define the rectangle
            let points = PlotPoints::new(vec![
                [rect_x_start, bar.y_center - half_h],
                [rect_x_end, bar.y_center - half_h],
                [rect_x_end, bar.y_center + half_h],
                [rect_x_start, bar.y_center + half_h],
            ]);

            let polygon = Polygon::new("", points)
                .fill_color(bar.color)
                .stroke(Stroke::NONE);

            plot_ui.polygon(polygon);
        }
    }
}

// ============================================================================
// 2. STICKY ZONE LAYER (Consolidation)
// ============================================================================
pub struct StickyZoneLayer;

impl PlotLayer for StickyZoneLayer {
    fn render(&self, plot_ui: &mut PlotUi, ctx: &LayerContext) {
        if !ctx.visibility.sticky {
            return;
        }

        let current_price = ctx.current_price;

        for superzone in &ctx.trading_model.zones.sticky_superzones {
            // 1. Determine Identity (Color/Label) based on price position
            let (label, color) = if let Some(price) = current_price {
                if superzone.contains(price) {
                    ("Active Sticky", PLOT_CONFIG.sticky_zone_color)
                } else if superzone.price_center < price {
                    ("Support", PLOT_CONFIG.support_zone_color)
                } else {
                    ("Resistance", PLOT_CONFIG.resistance_zone_color)
                }
            } else {
                ("Sticky", PLOT_CONFIG.sticky_zone_color)
            };

            let stroke = get_stroke(superzone, current_price, color);

            draw_superzone(
                plot_ui,
                superzone,
                ctx.x_min,
                ctx.x_max,
                label,
                color,
                stroke,
                1.0,
                1.0,
                ZoneShape::Rectangle,
            );
        }
    }
}

// ============================================================================
// 3. REVERSAL ZONE LAYER (Wicks)
// ============================================================================
pub struct ReversalZoneLayer;

impl PlotLayer for ReversalZoneLayer {
    fn render(&self, plot_ui: &mut PlotUi, ctx: &LayerContext) {
        let current_price = ctx.current_price;

        // A. Low Wicks (Support)
        if ctx.visibility.low_wicks {
            for superzone in &ctx.trading_model.zones.low_wicks_superzones {
                // let is_relevant = current_price
                //     .map(|p| superzone.contains(p) || superzone.price_center < p)
                //     .unwrap_or(false);

                let color = PLOT_CONFIG.low_wicks_zone_color;
                let label = UI_TEXT.label_reversal_support;
                let stroke = get_stroke(superzone, current_price, color);

                draw_superzone(
                    plot_ui,
                    superzone,
                    ctx.x_min,
                    ctx.x_max,
                    label,
                    color,
                    stroke,
                    0.5,
                    1.5,
                    ZoneShape::TriangleUp,
                );
            }
        }

        // B. High Wicks (Resistance)
        if ctx.visibility.high_wicks {
            for superzone in &ctx.trading_model.zones.high_wicks_superzones {
                // let is_relevant = current_price
                //     .map(|p| superzone.contains(p) || superzone.price_center > p)
                //     .unwrap_or(false);

                // if is_relevant {
                let color = PLOT_CONFIG.high_wicks_zone_color;
                let label = UI_TEXT.label_reversal_resistance;
                let stroke = get_stroke(superzone, current_price, color);

                draw_superzone(
                    plot_ui,
                    superzone,
                    ctx.x_min,
                    ctx.x_max,
                    label,
                    color,
                    stroke,
                    0.5,
                    1.5,
                    ZoneShape::TriangleDown,
                );
            }
        }
    }
}

// ============================================================================
// 4. PRICE LINE LAYER
// ============================================================================
pub struct PriceLineLayer;

impl PlotLayer for PriceLineLayer {
    fn render(&self, plot_ui: &mut PlotUi, ctx: &LayerContext) {
        if let Some(price) = ctx.current_price {
            let color = PLOT_CONFIG.current_price_color;
            let outer_color = PLOT_CONFIG.current_price_outer_color;

            let x_start = ctx.x_min;
            let x_end = ctx.x_max;

            // Outer Glow/Border
            plot_ui.line(
                Line::new("", PlotPoints::new(vec![[x_start, price], [x_end, price]]))
                    .color(outer_color)
                    .width(PLOT_CONFIG.current_price_outer_width)
                    .style(egui_plot::LineStyle::dashed_loose()),
            );

            // Inner Solid
            plot_ui.line(
                Line::new("", PlotPoints::new(vec![[x_start, price], [x_end, price]]))
                    .color(color)
                    .width(PLOT_CONFIG.current_price_line_width),
            );
        }
    }
}

// ============================================================================
// HELPER FUNCTIONS (Private to this module)
// ============================================================================

enum ZoneShape {
    Rectangle,
    TriangleUp,
    TriangleDown,
}

fn get_stroke(zone: &SuperZone, current_price: Option<f64>, base_color: Color32) -> Stroke {
    let is_active = current_price.map(|p| zone.contains(p)).unwrap_or(false);
    if is_active {
        Stroke::new(
            PLOT_CONFIG.active_zone_stroke_width,
            PLOT_CONFIG.active_zone_stroke_color,
        )
    } else {
        Stroke::new(1.0, base_color)
    }
}

fn draw_superzone(
    plot_ui: &mut PlotUi,
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
    // Calculate Geometry
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
            [z_x_min, superzone.price_top],       // Top Left
            [z_x_max, superzone.price_top],       // Top Right
            [z_x_center, superzone.price_bottom], // Bottom Point
        ],
    };

    let points = PlotPoints::new(points_vec);
    let final_color =
        fill_color.linear_multiply(PLOT_CONFIG.zone_fill_opacity_pct * opacity_factor);

    let polygon = Polygon::new(label, points)
        .fill_color(final_color)
        .stroke(stroke)
        .highlight(true);

    plot_ui.polygon(polygon);

    // Manual Hit Test
    if let Some(pointer) = plot_ui.pointer_coordinate() {
        if pointer.y >= superzone.price_bottom
            && pointer.y <= superzone.price_top
            && pointer.x >= z_x_min
            && pointer.x <= z_x_max
        {
            let tooltip_layer = LayerId::new(Tooltip, Id::new("zone_tooltips"));

            #[allow(deprecated)]
            show_tooltip_at_pointer(
                plot_ui.ctx(),
                tooltip_layer,
                Id::new(format!("tooltip_{}", superzone.id)),
                |ui: &mut Ui| {
                    ui.label(RichText::new(label).strong().color(fill_color));
                    ui.separator();
                    ui.label(format!("ID: #{}", superzone.id));
                    ui.label(format!(
                        "Range: {} - {}",
                        format_price(superzone.price_bottom),
                        format_price(superzone.price_top)
                    ));
                    let height = superzone.price_top - superzone.price_bottom;
                    ui.label(format!("Height: {}", format_price(height)));
                },
            );
        }
    }
}
