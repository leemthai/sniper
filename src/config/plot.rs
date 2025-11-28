//! Plot visualization configuration

use eframe::egui::Color32;

// ============================================================================
// COLOR SCHEMES - Uncomment one set to try it out
// ============================================================================

// --- OPTION 1: Classic Trading Colors (High Contrast) ---
// Sticky = Blue, Current Price = Yellow
// pub const SUPPORT_ZONE_COLOR: Color32 = Color32::from_rgb(0, 180, 0); // Forest green
// pub const RESISTANCE_ZONE_COLOR: Color32 = Color32::from_rgb(200, 0, 0); // Red
// pub const STICKY_ZONE_COLOR: Color32 = Color32::from_rgb(30, 144, 255); // Dodger blue
// pub const SLIPPY_ZONE_COLOR: Color32 = Color32::from_rgb(150, 150, 150); // Gray
// pub const CURRENT_PRICE_COLOR: Color32 = Color32::from_rgb(255, 255, 255); // White (maximum contrast)

// --- OPTION 2: Warm/Cool Spectrum ---
// Sticky = Deep Sky Blue, Current Price = Gold
pub const SUPPORT_ZONE_COLOR: Color32 = Color32::from_rgb(0, 200, 0); // Green
pub const RESISTANCE_ZONE_COLOR: Color32 = Color32::from_rgb(200, 0, 0); // Red
pub const STICKY_ZONE_COLOR: Color32 = Color32::from_rgb(0, 191, 255); // Deep sky blue
pub const SLIPPY_ZONE_COLOR: Color32 = Color32::from_rgb(150, 150, 150); // Gray
pub const LOW_WICKS_ZONE_COLOR: Color32 = Color32::from_rgb(0, 255, 255); // Cyan
pub const HIGH_WICKS_ZONE_COLOR: Color32 = Color32::from_rgb(255, 145, 164); // Salmon pink
pub const CURRENT_PRICE_COLOR: Color32 = Color32::from_rgb(255, 215, 0); // Gold

// --- OPTION 3: Maximum Contrast (RECOMMENDED) ---
// Sticky = Cyan, Current Price = White (maximum visibility)
// pub const SUPPORT_ZONE_COLOR: Color32 = Color32::from_rgb(0, 180, 0);      // Forest green
// pub const RESISTANCE_ZONE_COLOR: Color32 = Color32::from_rgb(200, 0, 0);   // Red
// pub const STICKY_ZONE_COLOR: Color32 = Color32::from_rgb(0, 150, 255);     // Bright cyan
// pub const SLIPPY_ZONE_COLOR: Color32 = Color32::from_rgb(150, 150, 150);   // Gray
// pub const CURRENT_PRICE_COLOR: Color32 = Color32::from_rgb(255, 255, 255); // White

// --- OPTION 4: Semantic Colors ---
// // Sticky = Royal Blue, Current Price = Golden Yellow
// pub const SUPPORT_ZONE_COLOR: Color32 = Color32::from_rgb(34, 139, 34); // Dark green
// pub const RESISTANCE_ZONE_COLOR: Color32 = Color32::from_rgb(200, 0, 0); // Red
// pub const STICKY_ZONE_COLOR: Color32 = Color32::from_rgb(65, 105, 225); // Royal blue
// pub const SLIPPY_ZONE_COLOR: Color32 = Color32::from_rgb(150, 150, 150); // Gray
// pub const CURRENT_PRICE_COLOR: Color32 = Color32::from_rgb(255, 223, 0); // Golden yellow

/// Outer stroke color for current price (for contrast)
pub const CURRENT_PRICE_OUTER_COLOR: Color32 = Color32::from_rgb(255, 0, 0); // Red border

/// Width of zone boundary lines
pub const ZONE_BOUNDARY_LINE_WIDTH: f32 = 2.0;

/// Width of current price line (inner line)
pub const CURRENT_PRICE_LINE_WIDTH: f32 = 4.0;

/// Width of current price outer stroke (for visibility)
pub const CURRENT_PRICE_OUTER_WIDTH: f32 = 8.0;

/// Default bar color for zones
pub const DEFAULT_BAR_COLOR: Color32 = Color32::from_rgb(255, 165, 0); // Orange

/// Gradient colors for zone importance visualization
/// From low importance (navy blue) to high importance (dark red)
pub const ZONE_GRADIENT_COLORS: &[&str] = &[
    "#000080", // Navy blue
    "#4b0082", // Indigo
    "#ffb703", // Amber
    "#ff8c00", // Dark orange
    "#ff4500", // Orange red
    "#b22222", // Firebrick
    "#8b0000", // Dark red
];

/// Plot aspect ratio (width:height)
pub const PLOT_ASPECT_RATIO: f32 = 2.0;

/// Plot x axis divisions
pub const PLOT_X_AXIS_DIVISIONS: u32 = 20;

/// Transparency/opacity for support and resistance zone rectangles (0.0 = invisible, 1.0 = fully opaque)
/// Lower values = more transparent, less visual clutter
pub const ZONE_FILL_OPACITY: f32 = 0.25; // 25% opacity

/// Background bar intensity (original score bars serve as background layer)
/// Lower values = more dimmed, letting zone overlays stand out
// pub const BACKGROUND_BAR_INTENSITY: f32 = 0.25; // 25% of original color
pub const BACKGROUND_BAR_INTENSITY: f32 = 0.75; // Testing value - 75% of original color

// ============================================================================
// ZONE VISIBILITY DEFAULTS
// ============================================================================
// Control which zone types are visible in the plot
// `false` values won't appear at all in the plot
// `true` values appear by default (but can be hidden in the legend)
pub const SHOW_STICKY_ZONES_DEFAULT: bool = true;

pub const SHOW_SUPPORT_ZONES_DEFAULT: bool = false;
pub const SHOW_RESISTANCE_ZONES_DEFAULT: bool = false;
pub const SHOW_SLIPPY_ZONES_DEFAULT: bool = false;
pub const SHOW_LOW_WICKS_ZONES_DEFAULT: bool = false;
pub const SHOW_HIGH_WICKS_ZONES_DEFAULT: bool = false;

/// Maximum number of per-zone journey lines to display in "Journey Outcomes" for the current pair
/// in the Journey Outcomes status area.
pub const MAX_JOURNEY_ZONE_LINES: usize = 10;
