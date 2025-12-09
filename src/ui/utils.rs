use eframe::egui::{Context, RichText, Ui, Visuals};

use crate::ui::config::UI_CONFIG;

/// Creates a colored heading with uppercase text and monospace font
pub fn colored_heading(text: impl Into<String>) -> RichText {
    let uppercase_text = text.into().to_uppercase() + ":";
    RichText::new(uppercase_text)
        .color(UI_CONFIG.colors.heading)
        .monospace()
}

/// Creates a colored sub-section headingusing the configured label color
// Why is this still monospace?
pub fn colored_subsection_heading(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(UI_CONFIG.colors.subsection_heading)
}

/// Sets up custom visuals for the entire application
pub fn setup_custom_visuals(ctx: &Context) {
    let mut visuals = Visuals::dark();

    // Customize the dark theme
    visuals.window_fill = UI_CONFIG.colors.central_panel;
    visuals.panel_fill = UI_CONFIG.colors.side_panel;

    // Make the widgets stand out a bit more
    visuals.widgets.noninteractive.fg_stroke.color = UI_CONFIG.colors.label;
    visuals.widgets.inactive.fg_stroke.color = UI_CONFIG.colors.label;
    visuals.widgets.hovered.fg_stroke.color = UI_CONFIG.colors.heading;
    visuals.widgets.active.fg_stroke.color = UI_CONFIG.colors.heading;

    // Set the custom visuals
    ctx.set_visuals(visuals);
}

/// Creates a section heading with standard spacing
pub fn section_heading(ui: &mut Ui, text: impl Into<String>) {
    ui.add_space(10.0);
    ui.heading(colored_heading(text));
    ui.add_space(5.0);
}

/// Creates a separator with standard spacing
pub fn spaced_separator(ui: &mut Ui) {
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);
}

/// Formats a price with "Trader Precision".
/// - Large (>1000): 2 decimals ($95,123.50)
/// - Medium (1-1000): 4 decimals ($12.4829)
/// - Small (<1): 6-8 decimals ($0.00000231)
pub fn format_price(price: f64) -> String {
    if price == 0.0 {
        return "$0.00".to_string();
    }

    // Determine magnitude
    let abs_price = price.abs();

    if abs_price >= 1000.0 {
        // BTC: 2 decimals is standard for high value
        format!("${:.2}", price)
    } else if abs_price >= 1.0 {
        // SOL/Normal Alts: 4 decimals captures the cents + fractions
        format!("${:.4}", price)
    } else if abs_price >= 0.01 {
        // Pennies: 5 decimals
        format!("${:.5}", price)
    } else {
        // Sub-penny / Meme coins: 8 decimals needed to see movement
        format!("${:.8}", price)
    }
}