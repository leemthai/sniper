use eframe::egui::Color32;

pub struct TickerConfig {
    pub height: f32,
    pub speed_pixels_per_sec: f32,
    pub font_size: f32,
    pub item_spacing: f32,
    pub background_color: Color32,

    // Pizazz Settings
    pub rainbow_mode: bool,
    pub rainbow_speed: f64, // How fast colors cycle

    // Colors
    pub text_color_neutral: Color32,
    pub text_color_up: Color32,
    pub text_color_down: Color32,

    pub text_color_link: Color32,
    pub custom_messages: &'static [(&'static str, Option<&'static str>)],
    pub min_change_pct_for_color: f64,
}

pub const TICKER: TickerConfig = TickerConfig {
    height: 18.0,
    speed_pixels_per_sec: 30.0,
    font_size: 10.0,
    item_spacing: 40.0,
    background_color: Color32::from_rgb(10, 10, 15), // Very dark
    
    rainbow_mode: true,
    rainbow_speed: 2.0, // Controls the animation frequecy of the color cycle
    
    text_color_neutral: Color32::LIGHT_GRAY,
    text_color_up: Color32::GREEN,
    text_color_down: Color32::RED,
    
    text_color_link: Color32::from_rgb(100, 200, 255), // Light Blue for links
    
    // Define your messages here
    custom_messages: &[
        ("🎄 MERRY CHRISTMAS 🎄", None),
        ("Built with Rust \u{e7a8}", Some("https://www.rust-lang.org")),
        (
            "Zone Sniper Pro",
            Some("https://github.com/leemthai/sniper"),
        ),
        ],
    min_change_pct_for_color: 0.01,
};
