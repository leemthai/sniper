//! Plot visualization configuration

use eframe::egui::Color32;

pub struct PlotConfig {
    pub support_zone_color: Color32,
    pub resistance_zone_color: Color32,
    pub sticky_zone_color: Color32,
    // Could be sticky zone, could be reversal zone, doesn't distinguish rn
    pub price_within_any_zone_color: Color32,
    pub current_price_color: Color32,
    pub current_price_outer_color: Color32,
    pub low_wicks_zone_color: Color32,
    pub high_wicks_zone_color: Color32,
    // Default bar color for zones
    pub default_bar_color: Color32,
    // Gradient colors for zone importance visualization
    pub zone_gradient_colors: &'static [&'static str],
    /// Width of zone boundary lines
    pub zone_boundary_line_width: f32,
    /// Width of current price line (inner line)
    pub current_price_line_width: f32,
    /// Width of current price outer stroke (for visibility)
    pub current_price_outer_width: f32,
    /// Plot x axis divisions (split axis into n equal parts)
    pub plot_axis_divisions: u32,
    /// Transparency/opacity for support and resistance zone rectangles (0.0 = invisible, 1.0 = fully opaque)
    /// Lower values = more transparent, less visual clutter
    pub zone_fill_opacity_pct: f32,
    /// Background bar intensity (original score bars serve as background layer)
    /// Lower values = more dimmed, letting zone overlays stand out
    pub background_bar_intensity_pct: f32,
    pub active_zone_stroke_color: Color32,
    pub active_zone_stroke_width: f32,

    // --- CANDLESTICKS (NEW) ---
    pub candle_bullish_color: Color32,
    pub candle_bearish_color: Color32,
    pub candle_width_pct: f64,  // 0.0 to 1.0 (relative to time step)
    pub candle_wick_width: f32, // Pixels
    pub segment_gap_width: f64, // Visual space between accordion segments

    pub plot_y_padding_pct: f64, // Y-Axis Padding factor (e.g. 0.05 = 5% padding top and bottom)


}

pub const PLOT_CONFIG: PlotConfig = PlotConfig {
    // STICKY ZONES ("The Terrain" - Earthy/Solid)
    // Darker Green for Support below
    support_zone_color: Color32::from_rgb(34, 139, 34), // Forest Green
    // Darker Red for Resistance above
    resistance_zone_color: Color32::from_rgb(178, 34, 34), // Firebrick Red

    // Generic sticky (fallback)
    sticky_zone_color: Color32::from_rgb(148, 0, 211),

    // ACTIVE ZONE ("The Battlefield" - Highlight)
    // Changed to Gold to denote the current engagement area
    price_within_any_zone_color: Color32::from_rgb(255, 215, 0), // Gold

    // PRICE LINE
    current_price_color: Color32::from_rgb(255, 215, 0), // Gold
    current_price_outer_color: Color32::from_rgb(255, 0, 0), // Red border

    // REVERSAL/WICK ZONES ("The Forces" - Neon/Bright)
    // Cyan = Buyers (Demand)
    low_wicks_zone_color: Color32::from_rgb(0, 255, 255), // Cyan / Electric Blue
    // Magenta = Sellers (Supply)
    high_wicks_zone_color: Color32::from_rgb(255, 0, 255), // Magenta / Fuchsia

    default_bar_color: Color32::from_rgb(255, 165, 0),

    // From low importance (navy blue) to high importance (dark red)
    zone_gradient_colors: &[
        "#000080", // Navy blue
        "#4b0082", // Indigo
        "#ffb703", // Amber
        "#ff8c00", // Dark orange
        "#ff4500", // Orange red
        "#b22222", // Firebrick
        "#8b0000", // Dark red
    ],

    zone_boundary_line_width: 2.0,
    current_price_line_width: 4.0,
    current_price_outer_width: 8.0,
    plot_axis_divisions: 20,

    // 40% Opacity allows the "Neon" wicks to blend with the "Earthy" walls
    // producing unique colors (e.g. Cyan + Red = Purple Conflict Zone)
    zone_fill_opacity_pct: 0.40,

    background_bar_intensity_pct: 0.5,

    // Highlight stroke for active zones
    active_zone_stroke_color: Color32::from_rgb(255, 255, 255), // White for max contrast on Gold
    active_zone_stroke_width: 1.5,

    // --- CANDLESTICKS (NEW) ---
    candle_bullish_color: Color32::from_rgb(38, 166, 154), // TradingView Green
    candle_bearish_color: Color32::from_rgb(239, 83, 80),  // TradingView Red
    candle_width_pct: 0.8, // 80% width leaves a small gap between candles
    candle_wick_width: 1.0,
    segment_gap_width: 4.0, // Visual gap between accordion segments

    // plot_y_padding_pct: 0.55, 
    // plot_y_padding_pct: 0.05, 
    // plot_y_padding_pct: 0.0001, 
    plot_y_padding_pct: 0.25, 

};
