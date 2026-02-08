use eframe::egui::{
    Align2, Color32, FontId, Id, LayerId, Order, Order::Tooltip, Painter, Pos2, Rect, RichText,
    Stroke, Ui, Vec2,
};

#[allow(deprecated)]
use eframe::egui::show_tooltip_at_pointer;

use egui_plot::{Line, PlotPoint, PlotPoints, PlotUi, Polygon};

use crate::analysis::range_gap_finder::GapReason;

use crate::config::{
    CandleResolution, ClosePrice, HighPrice, LowPrice, OpenPrice, Price, PriceLike, BASE_INTERVAL,
};

use crate::config::plot::PLOT_CONFIG;

use crate::models::OhlcvTimeSeries;
use crate::models::{SuperZone, TradeOpportunity, TradingModel};

use crate::ui::app::PlotVisibility;
use crate::ui::styles::{DirectionColor, apply_opacity};
use crate::ui::ui_plot_view::PlotCache;
use crate::ui::ui_text::UI_TEXT;

pub(crate) struct HorizonLinesLayer;

pub(crate) struct OpportunityLayer;

impl PlotLayer for OpportunityLayer {
    fn render(&self, plot_ui: &mut PlotUi, ctx: &LayerContext) {
        if !ctx.visibility.opportunities {
            return;
        }

        let current_price = match ctx.current_price {
            Some(p) if p.is_positive() => p,
            _ => return,
        };

        // 2. SSOT SELECTION LOGIC (Matching render_active_target_panel)
        // The Plot is a slave. It only renders what is explicitly selected.
        let opp_opt = ctx.selected_opportunity;
        let current_pair = &ctx.trading_model.cva.pair_name;

        // 2. Find Best Opportunity
        match opp_opt {
            Some(op) if &op.pair_name == current_pair => {
                // 3. Setup Foreground Painter
                // This guarantees EVERYTHING drawn here is on top of candles, grids, and background.
                // FIX: Create Painter with Clipping
                // This ensures the HUD never bleeds outside the plot area
                let painter = plot_ui
                    .ctx()
                    .layer_painter(LayerId::new(Order::Foreground, Id::new("sniper_hud")))
                    .with_clip_rect(ctx.clip_rect);

                // 4. Coordinates (Time/Price -> Screen Pixels)
                let x_center_plot = (ctx.x_min + ctx.x_max) / 2.0;

                // Map key points to screen space
                let current_pos_screen =
                    plot_ui.screen_from_plot(PlotPoint::new(x_center_plot, current_price.value()));
                let target_pos_screen = plot_ui
                    .screen_from_plot(PlotPoint::new(x_center_plot, op.target_price.value()));
                let sl_pos_screen =
                    plot_ui.screen_from_plot(PlotPoint::new(x_center_plot, op.stop_price.value()));

                // --- 1. RESOLVE SEMANTIC COLORS & STYLES ---
                let direction_color = op.direction.color();
                let sl_color = PLOT_CONFIG.color_stop_loss;

                // Resolve Dimming levels via Config
                let scope_color = apply_opacity(direction_color, PLOT_CONFIG.opacity_scope_base);
                let crosshair_color =
                    apply_opacity(scope_color, PLOT_CONFIG.opacity_scope_crosshair);
                let path_color = apply_opacity(scope_color, PLOT_CONFIG.opacity_path_line);

                // --- A. PATH LINE ---
                painter.line_segment(
                    [current_pos_screen, target_pos_screen],
                    Stroke::new(2.0, path_color),
                );

                // --- B. STOP LOSS ---
                let screen_rect = plot_ui.response().rect;
                let sl_width_px = screen_rect.width() * 0.4;

                let sl_left = sl_pos_screen - Vec2::new(sl_width_px / 2.0, 0.0);
                let sl_right = sl_pos_screen + Vec2::new(sl_width_px / 2.0, 0.0);

                painter.line_segment([sl_left, sl_right], Stroke::new(1.5, sl_color));
                painter.text(
                    sl_left + Vec2::new(0.0, -4.0),
                    Align2::LEFT_BOTTOM,
                    &UI_TEXT.label_stop_loss,
                    FontId::proportional(10.0),
                    sl_color,
                );

                // --- C. SCOPE ---
                painter.circle_stroke(target_pos_screen, 15.0, Stroke::new(2.0, scope_color));
                let hair_len = 20.0;
                let faint_stroke = Stroke::new(1.0, crosshair_color);

                painter.line_segment(
                    [
                        target_pos_screen - Vec2::new(0.0, hair_len),
                        target_pos_screen + Vec2::new(0.0, hair_len),
                    ],
                    faint_stroke,
                );
                painter.line_segment(
                    [
                        target_pos_screen - Vec2::new(hair_len, 0.0),
                        target_pos_screen + Vec2::new(hair_len, 0.0),
                    ],
                    faint_stroke,
                );
                painter.circle_filled(target_pos_screen, 3.0, scope_color);
            }
            Some(_) => {
                // CASE B: MISMATCH --- User has an Op selected, but it's for a DIFFERENT pair (e.g. ETH selected, viewing BTC).
                // So We draw NOTHING.
                // log::warn!(
                //     "This has failed for pair {} some reason: if &op.pair_name == current_pair .... so needs debugging. Not unexpected because I have disengaged from plot code a bit.",
                //     current_pair
                // );
            }

            None => {
                // --- CASE C: NO SELECTION ---
                // User has no active trade selected (maybe none available for this pair OR the user has never selected and)
                // We draw NOTHING.
                // log::warn!(
                //     "No active trade selected - means no available for {}. Can only occur if ctx.selected_opportunity is None",
                //     current_pair
                // );
            }
        }
    }
}

impl PlotLayer for HorizonLinesLayer {
    fn render(&self, plot_ui: &mut PlotUi, ctx: &LayerContext) {
        let (ph_min, ph_max) = ctx.ph_bounds;

        // Foreground Painter + Clipping
        let painter = plot_ui
            .ctx()
            .layer_painter(LayerId::new(Order::Foreground, Id::new("horizon_lines")))
            .with_clip_rect(ctx.clip_rect);

        let stroke = Stroke::new(2.0, PLOT_CONFIG.color_text_primary);
        let dash = 10.0;
        let gap = 10.0;

        let x_left = ctx.clip_rect.left();
        let x_right = ctx.clip_rect.right();

        // Map Prices to Screen Y
        let y_screen_max = plot_ui
            .screen_from_plot(PlotPoint::new(0.0, ph_max.value()))
            .y;
        let y_screen_min = plot_ui
            .screen_from_plot(PlotPoint::new(0.0, ph_min.value()))
            .y;

        // Top Line
        draw_dashed_line(
            &painter,
            Pos2::new(x_left, y_screen_max),
            Pos2::new(x_right, y_screen_max),
            stroke,
            dash,
            gap,
        );

        // Bottom Line
        draw_dashed_line(
            &painter,
            Pos2::new(x_left, y_screen_min),
            Pos2::new(x_right, y_screen_min),
            stroke,
            dash,
            gap,
        );
    }
}

pub(crate) struct CandlestickLayer;

impl PlotLayer for CandlestickLayer {
    fn render(&self, plot_ui: &mut PlotUi, ctx: &LayerContext) {
        if ctx.trading_model.segments.is_empty() {
            return;
        }

        // We track the cumulative visual offset for previous segments
        // (assuming segments are contiguous in view space, or ui_plot_view handles the gaps between segments via separate logic.
        //  If ui_plot_view sums up segment widths, we need to replicate that.)
        let mut segment_start_visual_x = 0.0;
        let agg_interval_ms = ctx.resolution.duration().as_millis() as i64;

        // 1. Optimization Setup
        let view_width_steps = (ctx.x_max - ctx.x_min).abs();
        let screen_width_px = plot_ui.response().rect.width() as f64;

        // FIX: Target a minimum physical pixel width per candle (e.g. 3px)
        // This ensures that even after applying the 80% width factor, we have > 2.4px solid body.
        // 1px = Faint/Sub-pixel. 3px = Crisp.
        let min_px_per_candle = 1.0;

        // How many candles can we physically fit on screen at this density?
        let max_candles_on_screen = (screen_width_px / min_px_per_candle).max(1.0);

        // Calculate aggregation batch size based on this limit
        let batch_size = if view_width_steps > 0.0 {
            (view_width_steps / max_candles_on_screen).ceil() as usize
        } else {
            1
        };

        let step = batch_size.max(1);
        let render_width = step as f64 * PLOT_CONFIG.candle_width_pct;

        for segment in &ctx.trading_model.segments {
            // Calculate Segment Start Time for relative offset
            let seg_start_ts = ctx.ohlcv.get_candle(segment.start_idx).timestamp_ms;

            // Align start to the grid
            let grid_start_ts = (seg_start_ts / agg_interval_ms) * agg_interval_ms;

            let mut i = segment.start_idx;

            while i < segment.end_idx {
                // --- BATCHING ---
                let mut batch_open = 0.0;
                let mut batch_high = f64::MIN;
                let mut batch_low = f64::MAX;
                let mut batch_close = 0.0;
                let mut steps_processed = 0;

                // Capture the timestamp of the first candle in this batch
                let first_candle_ts = ctx.ohlcv.get_candle(i).timestamp_ms;
                let current_grid_ts = (first_candle_ts / agg_interval_ms) * agg_interval_ms;

                while steps_processed < step && i < segment.end_idx {
                    let first = ctx.ohlcv.get_candle(i);

                    // Standard Aggregation (Resolution)
                    let boundary_start = (first.timestamp_ms / agg_interval_ms) * agg_interval_ms;
                    let boundary_end = boundary_start + agg_interval_ms;

                    let open = first.open_price.value();
                    let mut close = first.close_price.value();
                    let mut high = first.high_price.value();
                    let mut low = first.low_price.value();

                    let mut next_i = i + 1;
                    while next_i < segment.end_idx {
                        let c = ctx.ohlcv.get_candle(next_i);
                        if c.timestamp_ms >= boundary_end {
                            break;
                        }
                        high = high.max(c.high_price.value());
                        low = low.min(c.low_price.value());
                        close = c.close_price.value();
                        next_i += 1;
                    }

                    if steps_processed == 0 {
                        batch_open = open;
                    }
                    batch_high = batch_high.max(high);
                    batch_low = batch_low.min(low);
                    batch_close = close;

                    i = next_i;
                    steps_processed += 1;
                }

                if steps_processed > 0 {
                    // FIX: Calculate X based on TIME difference from Segment Start
                    // Offset = (CurrentBatchTime - SegmentGridStart) / Interval
                    // This creates gaps correctly where data is missing.
                    let time_offset = (current_grid_ts - grid_start_ts) / agg_interval_ms;

                    // Absolute Plot X = SegmentStart + Offset + CenterBias
                    let draw_x = segment_start_visual_x + time_offset as f64 + 0.5; // +0.5 to center in slot

                    draw_split_candle(
                        plot_ui,
                        draw_x,
                        OpenPrice::new(batch_open),
                        HighPrice::new(batch_high),
                        LowPrice::new(batch_low),
                        ClosePrice::new(batch_close),
                        render_width,
                        ctx.ph_bounds,
                        ctx.x_min,
                    );
                }
            }

            // Advance the visual cursor by the TOTAL width of this segment (including gaps)
            // This ensures the next segment starts at the right place.
            let last_candle_ts = ctx.ohlcv.get_candle(segment.end_idx - 1).timestamp_ms;
            let segment_duration = last_candle_ts - seg_start_ts;
            let segment_width = (segment_duration / agg_interval_ms) as f64 + 1.0;

            segment_start_visual_x += segment_width + PLOT_CONFIG.segment_gap_width;
        }
    }
}

// Helper: Draws the candle (splitting logic included)
fn draw_split_candle(
    ui: &mut PlotUi,
    x: f64,
    open: OpenPrice,
    high: HighPrice,
    low: LowPrice,
    close: ClosePrice,
    width: f64,
    ph_bounds: (Price, Price),
    min_x: f64,
) {
    let (ph_min, ph_max) = ph_bounds;

    let is_bullish = Price::from(close) >= Price::from(open);
    let base_color = if is_bullish {
        PLOT_CONFIG.candle_bullish_color
    } else {
        PLOT_CONFIG.candle_bearish_color
    };

    let ghost_color = base_color.linear_multiply(0.2);

    // --- 1. WICKS ---
    let open_p: Price = open.into();
    let close_p: Price = close.into();
    let high_p: Price = high.into();
    let low_p: Price = low.into();
    let ph_min_p: Price = ph_min.into();
    let ph_max_p: Price = ph_max.into();

    // A. Bottom Ghost Wick (Below ph_min)
    // Range: [low, min(high, ph_min)]
    let bg_wick_top = if high_p < ph_min_p { high_p } else { ph_min_p };
    let bg_wick_bot = low_p;
    if bg_wick_top > bg_wick_bot {
        draw_wick_line(ui, x, bg_wick_top, bg_wick_bot, ghost_color, min_x);
    }
    // B. Top Ghost Wick (Above ph_max)
    // Range: [max(low, ph_max), high]
    let tg_wick_top = high_p;
    let tg_wick_bot = if low_p > ph_max_p { low_p } else { ph_max_p };
    if tg_wick_top > tg_wick_bot {
        draw_wick_line(ui, x, tg_wick_top, tg_wick_bot, ghost_color, min_x);
    }

    // C. Solid Wick (Inside Zone)
    // Range: [max(low, ph_min), min(high, ph_max)]
    let solid_wick_top = if open_p > close_p { open_p } else { close_p };
    let solid_wick_bot = if open_p > close_p { close_p } else { open_p };
    if solid_wick_top > solid_wick_bot {
        draw_wick_line(ui, x, solid_wick_top, solid_wick_bot, base_color, min_x);
    }

    // --- 2. BODIES ---

    let body_top_raw = open.value().max(close.value());
    let body_bot_raw = open.value().min(close.value());
    let half_w = width / 2.0;

    // Doji check
    let body_top = if (body_top_raw - body_bot_raw).abs() < f64::EPSILON {
        body_top_raw + 0.00001
    } else {
        body_top_raw
    };
    let body_bot = body_bot_raw;

    // A. Bottom Ghost Body (Below ph_min)
    let bg_body_top = body_top.min(ph_min.value());
    let bg_body_bot = body_bot;
    if bg_body_top > bg_body_bot {
        draw_body_rect(ui, x, half_w, bg_body_top, bg_body_bot, ghost_color, min_x);
    }

    // B. Top Ghost Body (Above ph_max)
    let tg_body_top = body_top;
    let tg_body_bot = body_bot.max(ph_max.value());
    if tg_body_top > tg_body_bot {
        draw_body_rect(ui, x, half_w, tg_body_top, tg_body_bot, ghost_color, min_x);
    }

    // C. Solid Body (Inside Zone)
    let solid_body_top = body_top.min(ph_max.value());
    let solid_body_bot = body_bot.max(ph_min.value());
    if solid_body_top > solid_body_bot {
        draw_body_rect(
            ui,
            x,
            half_w,
            solid_body_top,
            solid_body_bot,
            base_color,
            min_x,
        );
    }
}

#[inline]
fn draw_wick_line(ui: &mut PlotUi, x: f64, top: Price, bottom: Price, color: Color32, min_x: f64) {
    if x < min_x || top <= bottom {
        return;
    }

    ui.line(
        Line::new(
            "",
            PlotPoints::new(vec![[x, bottom.value()], [x, top.value()]]),
        )
        .color(color)
        .width(PLOT_CONFIG.candle_wick_width),
    );
}

#[inline]
fn draw_body_rect(
    ui: &mut PlotUi,
    x: f64,
    half_w: f64,
    top: f64,
    bottom: f64,
    color: Color32,
    min_x: f64,
) {
    if top <= bottom {
        return;
    }

    let left = (x - half_w).max(min_x);
    let right = x + half_w;

    if left >= right {
        return;
    }

    let pts = vec![[left, bottom], [right, bottom], [right, top], [left, top]];
    ui.polygon(
        Polygon::new("", PlotPoints::new(pts))
            .fill_color(color)
            .stroke(Stroke::NONE),
    );
}

/// Context passed to every layer during rendering.
/// This prevents argument explosion.
pub(crate) struct LayerContext<'a> {
    pub trading_model: &'a TradingModel,
    pub ohlcv: &'a OhlcvTimeSeries,
    pub cache: &'a PlotCache,
    pub visibility: &'a PlotVisibility,
    // pub background_score_type: ScoreType,
    pub x_min: f64,
    pub x_max: f64,
    pub current_price: Option<Price>, // Pass SIM-aware price so layers render correctly in SIM mode
    pub resolution: CandleResolution,
    pub ph_bounds: (Price, Price), // (min, max) of the Price Horizon,
    pub clip_rect: Rect,
    pub selected_opportunity: &'a Option<TradeOpportunity>,
}

/// A standardized layer in the plot stack.
pub(crate) trait PlotLayer {
    fn render(&self, ui: &mut PlotUi, ctx: &LayerContext);
}

// BACKGROUND LAYER (The Histogram)
pub(crate) struct BackgroundLayer;

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

// STICKY ZONE LAYER (Consolidation)
pub(crate) struct StickyZoneLayer;

impl PlotLayer for StickyZoneLayer {
    fn render(&self, plot_ui: &mut PlotUi, ctx: &LayerContext) {
        if !ctx.visibility.sticky {
            return;
        }

        let current_price = ctx.current_price;

        for superzone in &ctx.trading_model.zones.sticky_superzones {
            // 1. Determine Identity (Color/Label) based on price position
            let (_, color) = if let Some(price) = current_price {
                if superzone.contains(price) {
                    ("", PLOT_CONFIG.sticky_zone_color)
                } else if superzone.price_center < price {
                    ("", PLOT_CONFIG.support_zone_color)
                } else {
                    ("", PLOT_CONFIG.resistance_zone_color)
                }
            } else {
                ("", PLOT_CONFIG.sticky_zone_color)
            };

            let stroke = get_stroke(superzone, current_price, color);

            draw_superzone(
                plot_ui,
                superzone,
                ctx.x_min,
                ctx.x_max,
                "label",
                color,
                stroke,
                1.0,
                1.0,
                ZoneShape::Rectangle,
            );
        }
    }
}

// REVERSAL ZONE LAYER (Wicks)
pub(crate) struct ReversalZoneLayer;

impl PlotLayer for ReversalZoneLayer {
    fn render(&self, plot_ui: &mut PlotUi, ctx: &LayerContext) {
        let current_price = ctx.current_price;

        // A. Low Wicks (Support)
        if ctx.visibility.low_wicks {
            for superzone in &ctx.trading_model.zones.low_wicks_superzones {
                let color = PLOT_CONFIG.low_wicks_zone_color;
                let stroke = get_stroke(superzone, current_price, color);

                draw_superzone(
                    plot_ui,
                    superzone,
                    ctx.x_min,
                    ctx.x_max,
                    "",
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
                // if is_relevant {
                let color = PLOT_CONFIG.high_wicks_zone_color;
                let stroke = get_stroke(superzone, current_price, color);

                draw_superzone(
                    plot_ui,
                    superzone,
                    ctx.x_min,
                    ctx.x_max,
                    "",
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

// SEGMENT SEPARATOR LAYER (Vertical Gaps)
pub(crate) struct SegmentSeparatorLayer;

impl PlotLayer for SegmentSeparatorLayer {
    fn render(&self, plot_ui: &mut PlotUi, ctx: &LayerContext) {
        if ctx.trading_model.segments.is_empty() {
            return;
        }

        let gap_width = PLOT_CONFIG.segment_gap_width;
        let mut visual_x = 0.0;
        let step_size = ctx.resolution.steps_from(BASE_INTERVAL);

        // Foreground Painter + Clipping
        let painter = plot_ui
            .ctx()
            .layer_painter(LayerId::new(Order::Foreground, Id::new("separators")))
            .with_clip_rect(ctx.clip_rect);

        let y_top = ctx.clip_rect.top();
        let y_bot = ctx.clip_rect.bottom();

        for (seg_idx, segment) in ctx.trading_model.segments.iter().enumerate() {
            let seg_candles_vis =
                ((segment.end_idx - segment.start_idx) as f64 / step_size as f64).ceil();
            visual_x += seg_candles_vis;

            if seg_idx < ctx.trading_model.segments.len() - 1 {
                let line_plot_x = visual_x + (gap_width / 2.0);

                // Map X to Screen
                let x_screen = plot_ui.screen_from_plot(PlotPoint::new(line_plot_x, 0.0)).x;

                // Only draw if within horizontal bounds of the plot rect
                if x_screen >= ctx.clip_rect.left() && x_screen <= ctx.clip_rect.right() {
                    let next_segment = &ctx.trading_model.segments[seg_idx + 1];

                    let base_color = match next_segment.gap_reason {
                        GapReason::PriceAbovePH => PLOT_CONFIG.color_gap_above,
                        GapReason::PriceBelowPH => PLOT_CONFIG.color_gap_below,
                        GapReason::MissingSourceData => PLOT_CONFIG.color_gap_missing,
                        _ => PLOT_CONFIG.color_separator, // Mixed/Generic -> Default Gray
                    };

                    let stroke = Stroke::new(
                        1.0,
                        apply_opacity(base_color, PLOT_CONFIG.opacity_separator),
                    );

                    draw_dashed_line(
                        &painter,
                        Pos2::new(x_screen, y_top),
                        Pos2::new(x_screen, y_bot),
                        stroke,
                        5.0, // Dash
                        5.0, // Gap
                    );
                }

                visual_x += gap_width;
            }
        }
    }
}

// 4. PRICE LINE LAYER
pub struct PriceLineLayer;

impl PlotLayer for PriceLineLayer {
    fn render(&self, plot_ui: &mut PlotUi, ctx: &LayerContext) {
        if let Some(price) = ctx.current_price {
            // Foreground Painter + Clipping
            let painter = plot_ui
                .ctx()
                .layer_painter(LayerId::new(Order::Foreground, Id::new("price_line")))
                .with_clip_rect(ctx.clip_rect);

            let color = PLOT_CONFIG.current_price_color;
            let width = PLOT_CONFIG.current_price_line_width; // Use standard width

            // Map Price to Screen Y
            let y_screen = plot_ui
                .screen_from_plot(PlotPoint::new(0.0, price.value()))
                .y;

            // Draw Simple Solid Line across width
            painter.line_segment(
                [
                    Pos2::new(ctx.clip_rect.left(), y_screen),
                    Pos2::new(ctx.clip_rect.right(), y_screen),
                ],
                Stroke::new(width, color),
            );
        }
    }
}

enum ZoneShape {
    Rectangle,
    TriangleUp,
    TriangleDown,
}

fn get_stroke(zone: &SuperZone, current_price: Option<Price>, base_color: Color32) -> Stroke {
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

    let top_p = superzone.price_top.value();
    let bottom_p = superzone.price_bottom.value();

    let points_vec = match shape {
        ZoneShape::Rectangle => vec![
            [z_x_min, bottom_p],
            [z_x_max, bottom_p],
            [z_x_max, top_p],
            [z_x_min, top_p],
        ],
        ZoneShape::TriangleUp => vec![
            [z_x_min, bottom_p], // Bottom Left
            [z_x_max, bottom_p], // Bottom Right
            [z_x_center, top_p], // Top Point
        ],
        ZoneShape::TriangleDown => vec![
            [z_x_min, top_p],       // Top Left
            [z_x_max, top_p],       // Top Right
            [z_x_center, bottom_p], // Bottom Point
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
        if pointer.y >= bottom_p
            && pointer.y <= top_p
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
                        superzone.price_bottom, superzone.price_top
                    ));
                    let height = superzone.price_top - superzone.price_bottom;
                    ui.label(format!("Height: {}", height));
                },
            );
        }
    }
}

/// Helper: Manually draws a dashed line between two screen points.
fn draw_dashed_line(
    painter: &Painter,
    p1: Pos2,
    p2: Pos2,
    stroke: Stroke,
    dash_len: f32,
    gap_len: f32,
) {
    let vec = p2 - p1;
    let total_len = vec.length();

    if total_len < 0.1 {
        return;
    }

    let dir = vec / total_len;
    let mut current_dist = 0.0;

    while current_dist < total_len {
        let end_dist = (current_dist + dash_len).min(total_len);

        let start_pos = p1 + (dir * current_dist);
        let end_pos = p1 + (dir * end_dist);

        painter.line_segment([start_pos, end_pos], stroke);

        current_dist += dash_len + gap_len;
    }
}
