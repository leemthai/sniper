//! Plot visualization configuration

use eframe::egui::Color32;

pub struct PlotConfig {
    pub support_zone_color: Color32,
    pub resistance_zone_color: Color32,
    pub sticky_zone_color: Color32,
    pub current_price_color: Color32,
    pub current_price_outer_color: Color32,
    pub low_wicks_zone_color: Color32,
    pub high_wicks_zone_color: Color32,
    // Default bar color for zones
    pub default_bar_color: Color32,
    // Gradient colors for zone importance visualization
    pub zone_gradient_colors: &'static [&'static str],
    // Zone visibility constants
    // `false` values won't appear at all in the plot
    // `true` values appear by default (but can be hidden later in the legend)
    pub show_sticky_zones: bool,
    pub show_support_zones: bool,
    pub show_resistance_zones: bool,
    pub show_low_wicks_zones: bool,
    pub show_high_wicks_zones: bool,
    /// Width of zone boundary lines
    pub zone_boundary_line_width: f32,
    /// Width of current price line (inner line)
    pub current_price_line_width: f32,
    /// Width of current price outer stroke (for visibility)
    pub current_price_outer_width: f32,
    /// Plot aspect ratio (width:height)
    pub plot_aspect_ratio: f32,
    /// Plot x axis divisions (split axis into n equal parts)
    pub plot_axis_divisions: u32,
    /// Transparency/opacity for support and resistance zone rectangles (0.0 = invisible, 1.0 = fully opaque)
    /// Lower values = more transparent, less visual clutter
    pub zone_fill_opacity_pct: f32,
    /// Background bar intensity (original score bars serve as background layer)
    /// Lower values = more dimmed, letting zone overlays stand out
    pub background_bar_intensity_pct: f32,
}

pub const PLOT_CONFIG: PlotConfig = PlotConfig {
    support_zone_color: Color32::from_rgb(0, 200, 0), // Green
    resistance_zone_color: Color32::from_rgb(200, 0, 0), // Red
    sticky_zone_color: Color32::from_rgb(0, 191, 255), // Deep sky blue
    current_price_color: Color32::from_rgb(255, 215, 0), // Gold
    current_price_outer_color: Color32::from_rgb(255, 0, 0), // Red border
    low_wicks_zone_color: Color32::from_rgb(0, 255, 255), // Cyan
    high_wicks_zone_color: Color32::from_rgb(255, 145, 164), // Salmon pink
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
    show_sticky_zones: true,
    show_support_zones: true,
    show_resistance_zones: true,
    show_low_wicks_zones: false,
    show_high_wicks_zones: false,
    zone_boundary_line_width: 2.0,
    current_price_line_width: 4.0,
    current_price_outer_width: 8.0,
    plot_aspect_ratio: 2.0,
    plot_axis_divisions: 20,
    zone_fill_opacity_pct: 0.25,
    background_bar_intensity_pct: 0.75,
};
