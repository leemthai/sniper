use eframe::egui::{Color32, Id, LayerId, Order::Tooltip, RichText, Stroke, Ui};

#[allow(deprecated)]
use eframe::egui::show_tooltip_at_pointer;

use egui_plot::{HLine, PlotPoints, PlotUi, Polygon, Line, LineStyle};

use crate::config::plot::PLOT_CONFIG;

use crate::models::cva::ScoreType;
use crate::models::trading_view::{SuperZone, TradingModel};
use crate::models::OhlcvTimeSeries;

use crate::ui::app::{CandleResolution, PlotVisibility};
use crate::ui::ui_plot_view::PlotCache;
use crate::ui::ui_text::UI_TEXT;
use crate::ui::utils::format_price;

pub struct CandlestickLayer;

fn draw_gap_separator(plot_ui: &mut PlotUi, x_pos: f64, gap_width: f64, y_bounds: (f64, f64)) {
    let (y_min, y_max) = y_bounds;
    let line_x = x_pos + (gap_width / 2.0);

    // Overshoot the bounds slightly to ensure the line always looks infinite vertical
    let range = y_max - y_min;
    let y_start = y_min - range; 
    let y_end = y_max + range;

    plot_ui.line(
        Line::new("", PlotPoints::new(vec![[line_x, y_start], [line_x, y_end]]))
        .color(Color32::from_gray(60)) // Subtle gray
        .width(1.0)
        .style(LineStyle::Dashed { length: 5.0 })
    );
}

impl PlotLayer for CandlestickLayer {


    fn render(&self, plot_ui: &mut PlotUi, ctx: &LayerContext) {
        if ctx.trading_model.segments.is_empty() { return; }

        let mut visual_x = 0.0;
        let candle_width = PLOT_CONFIG.candle_width_pct; 
        let wick_width = PLOT_CONFIG.candle_wick_width;
        let gap_width = PLOT_CONFIG.segment_gap_width;
        let y_bounds = ctx.trading_model.cva.price_range.min_max();

        // Determine Aggregation Step Size
        let step_size = ctx.resolution.step_size();

        for (seg_idx, segment) in ctx.trading_model.segments.iter().enumerate() {
            
            // FIX: Use a Range Iterator with step_by.
            // This cleanly handles the "1 to N" grouping without manual counters.
            for chunk_start in (segment.start_idx..segment.end_idx).step_by(step_size) {
                let chunk_end = (chunk_start + step_size).min(segment.end_idx);

                // --- AGGREGATION LOGIC ---
                // 1. Init with First Candle
                let first = ctx.ohlcv.get_candle(chunk_start);
                let open = first.open_price; // Open never changes!
                let mut high = first.high_price;
                let mut low = first.low_price;
                let mut close = first.close_price;

                // 2. Merge remaining candles in this chunk
                for i in (chunk_start + 1)..chunk_end {
                    let c = ctx.ohlcv.get_candle(i);
                    high = high.max(c.high_price);
                    low = low.min(c.low_price);
                    close = c.close_price; // Close is always the latest
                }
                
                // --- DRAWING ---
                let is_green = close >= open;
                
                let color = if is_green { 
                    PLOT_CONFIG.candle_bullish_color
                } else { 
                    PLOT_CONFIG.candle_bearish_color
                };
                
                // 1. Wick
                let wick_points = PlotPoints::new(vec![
                    [visual_x, low],
                    [visual_x, high]
                ]);
                
                plot_ui.line(Line::new("", wick_points)
                    .color(color)
                    .width(wick_width)
                );
                
                // 2. Body
                let (body_bottom, body_top) = if (open - close).abs() < f64::EPSILON {
                    (open, open * 1.0001) // Doji visibility
                } else {
                    (open.min(close), open.max(close))
                };

                let half_w = candle_width / 2.0;
                let rect_points = vec![
                    [visual_x - half_w, body_bottom],
                    [visual_x + half_w, body_bottom],
                    [visual_x + half_w, body_top],
                    [visual_x - half_w, body_top],
                ];
                
                let poly = Polygon::new("", PlotPoints::new(rect_points))
                    .fill_color(color);

                plot_ui.polygon(poly);

                // Advance Visual X by 1 unit (representing 1 aggregated candle)
                visual_x += 1.0;
            }

            // Draw Gap Separator (if not last segment)
            if seg_idx < ctx.trading_model.segments.len() - 1 {
                draw_gap_separator(plot_ui, visual_x, gap_width, y_bounds);
                visual_x += gap_width;
            }
        }

    }
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

        // // 1. Determine Label
        // let type_label = match ctx.background_score_type {
        //     ScoreType::FullCandleTVW => "Trading Volume",
        //     ScoreType::LowWickCount => "Lower Wick Strength",
        //     ScoreType::HighWickCount => "Upper Wick Strength",
        // };

        // 2. Create Group Name (Appears in Legend)
        // let legend_label = format!("Background Plot: {}", type_label);

        for bar in &ctx.cache.bars {
            let half_h = bar.height / 2.0;

            let points = PlotPoints::new(vec![
                [0.0, bar.y_center - half_h],
                [bar.x_max, bar.y_center - half_h],
                [bar.x_max, bar.y_center + half_h],
                [0.0, bar.y_center + half_h],
            ]);

            // Name passed here enables Legend grouping
            let polygon = Polygon::new("", points)
                .fill_color(bar.color)
                .stroke(Stroke::NONE); // Critical for visual coherence

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
            let label = "Current Price";

            // Outer Line (Border)
            plot_ui.hline(
                HLine::new(label, price)
                    .color(PLOT_CONFIG.current_price_outer_color)
                    .width(PLOT_CONFIG.current_price_outer_width)
                    .style(egui_plot::LineStyle::dashed_loose()),
            );

            // Inner Line (Color)
            plot_ui.hline(
                HLine::new(label, price)
                    .color(PLOT_CONFIG.current_price_color)
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

// fn get_zone_status_color(zone: &SuperZone, current_price: Option<f64>) -> Color32 {
//     if let Some(price) = current_price {
//         if zone.contains(price) {
//             PLOT_CONFIG.sticky_zone_color // Purple (Active)
//         } else if zone.price_center < price {
//             PLOT_CONFIG.support_zone_color // Green
//         } else {
//             PLOT_CONFIG.resistance_zone_color // Red
//         }
//     } else {
//         PLOT_CONFIG.sticky_zone_color
//     }
// }

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
