use eframe::egui::{Context, Visuals};

use crate::ui::ui_text::UI_TEXT;
use crate::ui::config::UI_CONFIG;

pub fn format_candle_count(count: usize) -> String {
    // let label = UI_TEXT.label_candle;
    format!("{} {}", count, UI_TEXT.label_candle)
}

pub fn format_duration_context(days: f64) -> String {
    let minutes_total = (days * 24.0 * 60.0).round() as i64;

    if minutes_total < 60 {
        // Example: "45m"
        format!("{}m", minutes_total)
    } else if minutes_total < 24 * 60 {
        // Example: "14h 30m"
        let h = minutes_total / 60;
        let m = minutes_total % 60;
        if m > 0 {
            format!("{}h {}m", h, m)
        } else {
            format!("{}h", h)
        }
    } else if days < 30.44 {
        // Example: "9d 14h"
        let d = minutes_total / (24 * 60);
        let h = (minutes_total % (24 * 60)) / 60;
        if h > 0 {
            format!("{}d {}h", d, h)
        } else {
            format!("{}d", d)
        }
    } else if days < 365.25 {
        // Example: "4M 12d" (Months + Days)
        let months = (days / 30.44).floor() as i64;
        let remaining_days = (days % 30.44).round() as i64;
        
        if remaining_days > 0 {
            format!("{}M {}d", months, remaining_days)
        } else {
            format!("{} Months", months)
        }
    } else {
        // Example: "2Y 3M" (Years + Months)
        let years = (days / 365.25).floor() as i64;
        let remaining_days = days % 365.25;
        let months = (remaining_days / 30.44).round() as i64;
        
        if months > 0 {
            format!("{}Y {}M", years, months)
        } else {
            format!("{} Years", years)
        }
    }
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


// /// Formats a price with "Trader Precision".
// /// - Large (>1000): 2 decimals ($95,123.50)
// /// - Medium (1-1000): 4 decimals ($12.4829)
// /// - Small (<1): 6-8 decimals ($0.00000231)
// pub fn format_price(price: f64) -> String {
//     if price == 0.0 {
//         return "$0.00".to_string();
//     }

//     // Determine magnitude
//     let abs_price = price.abs();

//     if abs_price >= 1000.0 {
//         // BTC: 2 decimals is standard for high value
//         format!("${:.2}", price)
//     } else if abs_price >= 1.0 {
//         // SOL/Normal Alts: 4 decimals captures the cents + fractions
//         format!("${:.4}", price)
//     } else if abs_price >= 0.01 {
//         // Pennies: 5 decimals
//         format!("${:.5}", price)
//     } else {
//         // Sub-penny / Meme coins: 8 decimals needed to see movement
//         format!("${:.8}", price)
//     }
// }