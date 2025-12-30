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
    pub plot_x_padding_pct: f64,

    // --- SEMANTIC COLORS ---
    pub color_profit: Color32,
    pub color_loss: Color32,
    pub color_long: Color32,
    pub color_short: Color32,
    pub color_stop_loss: Color32,

    pub color_info: Color32,
    pub color_warning: Color32,

    pub color_text_neutral: Color32, // Main values (white)
    pub color_text_primary: Color32, // For the galley tint (Light Gray)
    pub color_text_subdued: Color32,
    /// Explanations/Context (Darker Gray)
    // --- VISUAL CONSTANTS (Opacity/Dimming) ---
    pub opacity_scope_base: f32, // Main circle intensity
    pub opacity_scope_crosshair: f32, // Crosshair relative to scope
    pub opacity_path_line: f32,       // Path line relative to scope

    // GAP SEMANTICS
    pub color_separator: Color32,
    pub color_gap_above: Color32,   // Price went > PH (Resistance-ish)
    pub color_gap_below: Color32,   // Price went < PH (Support-ish)
    pub color_gap_missing: Color32, // Exchange down / Data hole
    pub opacity_separator: f32,

    // HEATMAP LEGEND COLORS
    pub color_heatmap_low: Color32,  // Deep Purple
    pub color_heatmap_med: Color32,  // Orange/Red
    pub color_heatmap_high: Color32, // Bright Yellow

    // UI WIDGET STYLES
    pub color_widget_background: Color32, // Dark background for custom widgets
    pub color_widget_border: Color32,     // Subtle border

    // HELP COLORS
    pub color_help_fg: Color32,
    pub color_help_bg: Color32,
    pub color_help_bg_hover: Color32,
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

    plot_y_padding_pct: 0.02, // Visual plot padding above PH max and below PH min

    // X-Axis Padding (Horizontal)
    // 2% of the width is added to Left and Right.
    // E.g. if viewing 100 candles, this adds 2 "empty" candles of space on each side.
    plot_x_padding_pct: 0.02,

    // SEMANTICS
    color_profit: Color32::from_rgb(100, 255, 100),
    color_loss: Color32::from_rgb(255, 80, 80),
    color_long: Color32::from_rgb(0, 191, 255),
    color_short: Color32::from_rgb(255, 165, 0),

    // NEW DEFINITIONS
    color_info: Color32::from_rgb(173, 216, 230), // Light Blue (Volatility)
    color_warning: Color32::from_rgb(255, 215, 0), // Gold/Yellow (High Vol)

    color_help_fg: Color32::from_rgb(120, 170, 240), // help fg
    color_help_bg: Color32::WHITE, // help background
    color_help_bg_hover: Color32::ORANGE, // help background on hover

    color_text_primary: Color32::WHITE,
    color_text_neutral: Color32::LIGHT_GRAY,
    color_text_subdued: Color32::GRAY,

    color_stop_loss: Color32::from_rgb(255, 80, 80),

    // VISUAL CONSTANTS
    opacity_scope_base: 0.8,
    opacity_scope_crosshair: 1.0, // relative to base (0.8 * 1.0)
    opacity_path_line: 0.6,       // relative to base (0.8 * 0.6)

    color_separator: Color32::from_gray(80), // Subtle vertical separator line on plot
    // GAP COLORS
    color_gap_above: Color32::from_rgb(100, 255, 100), // Green (Candles are Above)
    color_gap_below: Color32::from_rgb(255, 100, 100), // Red (Candles are Below)
    color_gap_missing: Color32::from_rgb(180, 100, 255), // Purple
    // VISUAL CONSTANTS
    opacity_separator: 0.25, // Very subtle (25% opacity)

    // HEATMAP LEGEND
    color_heatmap_low: Color32::from_rgb(45, 11, 89),
    color_heatmap_med: Color32::from_rgb(237, 105, 37),
    color_heatmap_high: Color32::from_rgb(251, 180, 26),

    // UI WIDGETS
    color_widget_background: Color32::from_black_alpha(40),
    color_widget_border: Color32::from_gray(60),
};
