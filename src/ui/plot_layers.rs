use eframe::egui::{Color32, Stroke, RichText, LayerId, Order::Tooltip, Id, Ui};

#[allow(deprecated)]
use eframe::egui::show_tooltip_at_pointer;

use egui_plot::{PlotPoints, Polygon, PlotUi, HLine};


use crate::config::plot::PLOT_CONFIG;
use crate::models::cva::ScoreType;
use crate::models::trading_view::{SuperZone, TradingModel};
use crate::ui::app::PlotVisibility;
use crate::ui::ui_plot_view::PlotCache;
use crate::ui::ui_text::UI_TEXT;
use crate::ui::utils::format_price;


/// Context passed to every layer during rendering.
/// This prevents argument explosion.
pub struct LayerContext<'a> {
    pub trading_model: &'a TradingModel,
    pub cache: &'a PlotCache,
    pub visibility: &'a PlotVisibility,
    pub background_score_type: ScoreType,
    pub x_min: f64,
    pub x_max: f64,
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
        use egui_plot::{PlotPoints, Polygon};

        // 1. Determine Label
        let type_label = match ctx.background_score_type {
            ScoreType::FullCandleTVW => "Trading Volume",
            ScoreType::LowWickCount => "Lower Wick Count",
            ScoreType::HighWickCount => "Upper Wick Count",
            _ => "Unknown",
        };

        // 2. Create Group Name (Appears in Legend)
        let legend_label = format!("Zone Strength: {}", type_label);

        for bar in &ctx.cache.bars {
            let half_h = bar.height / 2.0;
            
            let points = PlotPoints::new(vec![
                [0.0, bar.y_center - half_h],
                [bar.x_max, bar.y_center - half_h],
                [bar.x_max, bar.y_center + half_h],
                [0.0, bar.y_center + half_h],
            ]);

            // Name passed here enables Legend grouping
            let polygon = Polygon::new(&legend_label, points)
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
        if !ctx.visibility.sticky { return; }

        let current_price = ctx.trading_model.current_price;

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
                plot_ui, superzone, ctx.x_min, ctx.x_max, 
                label, color, stroke, 
                1.0, 1.0, ZoneShape::Rectangle
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
        let current_price = ctx.trading_model.current_price;

        // A. Low Wicks (Support)
        if ctx.visibility.low_wicks {
            for superzone in &ctx.trading_model.zones.low_wicks_superzones {
                let is_relevant = current_price
                    .map(|p| superzone.contains(p) || superzone.price_center < p)
                    .unwrap_or(false);

                if is_relevant {
                    let color = get_zone_status_color(superzone, current_price);
                    let label = UI_TEXT.label_reversal_support;
                    let stroke = get_stroke(superzone, current_price, color);

                    draw_superzone(
                        plot_ui, superzone, ctx.x_min, ctx.x_max, 
                        label, color, stroke, 
                        0.5, 1.5, ZoneShape::TriangleUp
                    );
                }
            }
        }

        // B. High Wicks (Resistance)
        if ctx.visibility.high_wicks {
            for superzone in &ctx.trading_model.zones.high_wicks_superzones {
                let is_relevant = current_price
                    .map(|p| superzone.contains(p) || superzone.price_center > p)
                    .unwrap_or(false);

                if is_relevant {
                    let color = get_zone_status_color(superzone, current_price);
                    let label = UI_TEXT.label_reversal_resistance;
                    let stroke = get_stroke(superzone, current_price, color);

                    draw_superzone(
                        plot_ui, superzone, ctx.x_min, ctx.x_max, 
                        label, color, stroke, 
                        0.5, 1.5, ZoneShape::TriangleDown
                    );
                }
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
        if let Some(price) = ctx.trading_model.current_price {
            let label = "Current Price";

            // Outer Line (Border)
            plot_ui.hline(
                HLine::new(label, price)
                    .color(PLOT_CONFIG.current_price_outer_color)
                    .width(PLOT_CONFIG.current_price_outer_width)
                    .style(egui_plot::LineStyle::dashed_loose())
            );

            // Inner Line (Color)
            plot_ui.hline(
                HLine::new(label, price)
                    .color(PLOT_CONFIG.current_price_color)
                    .width(PLOT_CONFIG.current_price_line_width)
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

fn get_zone_status_color(zone: &SuperZone, current_price: Option<f64>) -> Color32 {
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
        .highlight(true);

    plot_ui.polygon(polygon);

    // Manual Hit Test
    if let Some(pointer) = plot_ui.pointer_coordinate() {
        if pointer.y >= superzone.price_bottom 
           && pointer.y <= superzone.price_top
           && pointer.x >= z_x_min 
           && pointer.x <= z_x_max 
        {
            let tooltip_layer = LayerId::new(
                Tooltip,
                Id::new("zone_tooltips")
            );

            #[allow(deprecated)]
            show_tooltip_at_pointer(
                plot_ui.ctx(),
                tooltip_layer,
                Id::new(format!("tooltip_{}", superzone.id)),
                |ui: &mut Ui| {
                    ui.label(RichText::new(label).strong().color(fill_color));
                    ui.separator();
                    ui.label(format!("ID: #{}", superzone.id));
                    ui.label(format!("Range: {} - {}", format_price(superzone.price_bottom), format_price(superzone.price_top)));
                    let height = superzone.price_top - superzone.price_bottom;
                    ui.label(format!("Height: {}", format_price(height)));
                }
            );
        }
    }
}