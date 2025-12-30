pub const ICON_CROSSHAIR: &str  = "\u{f05b}"; //  (Crosshairs)
pub const ICON_BAN: &str        = "\u{f05e}"; //  (Ban)
// pub const ICON_LIST: &str       = "\u{f03a}"; //  (List)
pub const ICON_LIST: &str       = "\u{f0ca}"; //  (List)
pub const ICON_FILTER: &str     = "\u{f0b0}"; //  (Filter)
pub const ICON_DNA: &str        = "\u{f471}"; //  (DNA)

pub const ICON_TREND_UP: &str   = "\u{f0d8}"; // d (Caret Up / Triangle Up)
pub const ICON_TREND_DOWN: &str = "\u{f0d7}"; // d (Caret Down / Triangle Down)

// --- NEW ICONS (FontAwesome) ---
pub const ICON_SCOPE: &str      = "\u{f05b}"; //  (Crosshair)
pub const ICON_CLOCK: &str      = "\u{f017}"; //  (Clock)
pub const ICON_GLOBE: &str      = "\u{f0ac}"; //  (Globe)

pub const ICON_CANDLE:&str  = "\u{f05e2}";

pub struct UiText {

    pub data_generation_heading: &'static str,
    pub price_horizon_heading: &'static str,
    pub pair_selector_heading: &'static str,
    pub view_options_heading: &'static str,
    pub view_data_source_heading: &'static str,
    pub price_horizon_helper_prefix: &'static str,
    pub price_horizon_helper_suffix: &'static str,
    pub time_horizon_helper_prefix: &'static str,
    pub time_horizon_helper_suffix: &'static str,

    pub plot_x_axis: &'static str,
    pub plot_y_axis: &'static str,
    pub plot_strongest_zone: &'static str,
    pub plot_this_zone_is: &'static str,

    pub label_volume: &'static str,
    pub label_reversal_support: &'static str,
    pub label_reversal_resistance: &'static str,
    pub label_hvz: &'static str,
    pub label_lower_wick_zones: &'static str,
    pub label_upper_wick_zones: &'static str,
    pub label_help_sim_toggle_direction: &'static str,
    pub label_help_sim_step_size: &'static str,
    pub label_help_sim_activate_price_change: &'static str,
    pub label_help_sim_jump_hvz: &'static str,
    pub label_help_sim_jump_lower_wicks: &'static str,
    pub label_help_sim_jump_higher_wicks: &'static str,

    // Status Bar Labels
    pub label_candle: &'static str,
    pub label_volatility: &'static str,

    // Error Messages
    pub error_insufficient_data_title: &'static str,
    pub error_insufficient_data_body: &'static str,

    pub ph_label_evidence: &'static str,
    pub ph_label_history: &'static str,
    pub ph_label_density: &'static str,

    pub ph_label_horizon_prefix: &'static str,

    pub ph_startup: &'static str,

    pub ph_help_title: &'static str,
    pub ph_help_metrics_title: &'static str,
    pub ph_help_colors_title: &'static str,
    pub ph_help_tuning_title: &'static str,

    pub ph_help_density_header: (&'static str, &'static str, &'static str),
    pub ph_help_density_rows: &'static [(&'static str, &'static str, &'static str)],

    pub ph_help_scope_header: (&'static str, &'static str, &'static str),
    pub ph_help_scope_rows: &'static [(&'static str, &'static str, &'static str)],

    pub ph_help_definitions: &'static [(&'static str, &'static str)],

    pub cr_title_1: &'static str,
    pub cr_title_2: &'static str,
    pub cr_header_id: &'static str,
    pub cr_header_date: &'static str,
    pub cr_header_len: &'static str,
    pub cr_header_ctx: &'static str,
    pub cr_label_live: &'static str,
    pub cr_label_historical: &'static str,
    pub cr_gap_price_mismatch: &'static str,
    pub cr_gap_missing_source: &'static str,
    pub cr_gap_price_above: &'static str,
    pub cr_gap_price_below: &'static str,
    pub cr_gap_mixed: &'static str,

    pub cr_nav_show_all: &'static str,
    pub cr_nav_return_prefix: &'static str,
    pub cr_nav_return_live: &'static str,

    pub label_success_rate: &'static str,

    // TRADE FINDER / OPPORTUNITIES
    pub tf_header: &'static str,
    pub tf_scope_all: &'static str,
    pub tf_scope_selected: &'static str, // Prefix for "BTCUSDT ONLY"
    
    // METRICS & LABELS
    pub label_roi: &'static str,
    pub label_aroi: &'static str,        // "AROI"
    pub label_sl_variants: &'static str, // "SL Variants"
    pub label_target: &'static str,
    pub label_stop: &'static str,
    
    // ICONS / SYMBOLS
    pub icon_long: &'static str,
    pub icon_short: &'static str,

    // LABELS REPLACED BY ICONS
    pub label_res_icon: &'static str,    // Replaces "Res:"
    pub label_scope_icon: &'static str,  // Replaces "Scope:"
    pub label_filter_icon: &'static str, // Replaces "Filter:"
    
    // BUTTON TEXT
    pub tf_btn_all: &'static str,
    pub tf_btn_long: &'static str,
    pub tf_btn_short: &'static str,


}

pub const UI_TEXT: UiText = UiText {

        // ICONS
    label_res_icon:    ICON_CLOCK,
    label_scope_icon:  ICON_SCOPE,
    label_filter_icon: ICON_FILTER,

    // BUTTONS (With embedded icons)
    tf_btn_all:   "ALL", 
    tf_btn_long:  "LONG",
    tf_btn_short: "SHORT",


    data_generation_heading: "Shape Your Trades",
    price_horizon_heading: "Price Horizon",
    pair_selector_heading: "Select Plot Pair",
    view_options_heading: "View Options",
    view_data_source_heading: "Data Source",

    price_horizon_helper_prefix: "Focus on price action within ±",
    price_horizon_helper_suffix: "% of current price",
    time_horizon_helper_prefix: "Focus on trades that complete within ",
    time_horizon_helper_suffix: " days",

    plot_y_axis: "Price",
    plot_x_axis: "Key Zone Strength (0 % of the strongest zone)",
    plot_strongest_zone: "of strongest zone",
    plot_this_zone_is: "This zone is",

    label_volume: "Trading Volume",
    
    label_hvz: "High Volume Zones",
    label_lower_wick_zones: "Lower Wick Zones",
    label_upper_wick_zones: "Upper Wick Zones",
    label_reversal_support: "Demand Zone (Buyers Here)",
    label_reversal_resistance: "Supply Zone (Sellers Here)",

    label_help_sim_toggle_direction: "Toggle direction (⬆️ UP / ⬇️ DOWN)",
    label_help_sim_step_size: "Cycle step size (0.1% → 1% → 5% → 10%)",
    label_help_sim_activate_price_change: "Activate price change in current direction",
    label_help_sim_jump_hvz: "Jump to next High Volume Zone",
    label_help_sim_jump_lower_wicks: "Jump to next Demand Zone",
    label_help_sim_jump_higher_wicks: "Jump to next Supply Zone",

    label_volatility: "Volatility (Avg True Range)",
    
    error_insufficient_data_title: "Analysis Paused: Range Too Narrow",
    
    error_insufficient_data_body: "The current Price Horizon does not capture enough price history to identify reliable zones.\n\n\
    👉 Action: Drag the Price Horizon slider to the right (aim for High Density / Yellow areas).",
    
    label_candle: ICON_CANDLE,

    ph_label_evidence: "Evidence", // Active Duration
    ph_label_history: "History",   // Span
    ph_label_density: "Density",   // Quality

    ph_label_horizon_prefix: "Horizon: ±",

    ph_startup: "Analyzing Price Structure...",

    ph_help_title: "Price Horizon Guide",
    ph_help_metrics_title: "Metrics",
    ph_help_colors_title: "Signal Quality",
    ph_help_tuning_title: "Tuning Guide",

    // Table 1: HEATMAP (Density)
    ph_help_density_header: ("Color", "Density", "Significance"),
    ph_help_density_rows: &[
        (
            "Deep Purple",
            "Low (< 10%)",
            "Statistically Insignificant (noise)",
        ),
        ("Orange/Red", "Medium", "Standard Statistical Confidence"),
        (
            "Bright Yellow",
            "High (> 80%)",
            "High Statistical Significance",
        ),
    ],

    // Table 2: SCOPE (Trade Style)
    ph_help_scope_header: ("Horizon %", "Style", "Focus"),
    ph_help_scope_rows: &[
        (
            "< 5%",
            "Sniper / Scalp",
            "Immediate price action",
        ),
        (
            "5% - 15%",
            "Swing Trade",
            "Balanced recent history",
        ),
        (
            "> 15%",
            "Macro / Invest",
            "Deep historical structure",
        ),
    ],

    ph_help_definitions: &[
        (
            "Evidence",
            "Total duration of actual data (candle count * interval).",
        ),
        (
            "History",
            "Calendar time elapsed between the first and last candle within this price range.",
        ),
        (
            "Density",
            "Ratio of Evidence to History. (Yellow = highest data quality).",
        ),
    ],

    // --- CANDLE RANGE NAVIGATOR (CR) ---
    cr_title_1: "Time Machine",
    cr_title_2: "Candle Ranges Intersect PH Range",

    // Headers
    cr_header_id: "#",
    cr_header_date: "Date Range",
    cr_header_len: "Length",
    cr_header_ctx: "Context",

    // Context Labels
    cr_label_live: "LIVE",
    cr_label_historical: "Historical",

    // Gap Reasons
    cr_gap_price_mismatch: "Price out of Range",
    cr_gap_missing_source: "Data missing from Exchange)",
    cr_gap_price_above: "Price > Horizon",
    cr_gap_price_below: "Price < Horizon",
    cr_gap_mixed: "Mixed Gap",

    // NAVIGATION
    cr_nav_show_all: "SHOW ALL RANGES",
    cr_nav_return_prefix: "RETURN TO SEGMENT",
    cr_nav_return_live: "RETURN TO LIVE",

    label_success_rate: "Success Rate",

    tf_header: "TRADE FINDER",
    tf_scope_all: "ALL PAIRS",
    tf_scope_selected: "ONLY", // e.g. "BTCUSDT ONLY"
    
    label_roi: "ROI",
    label_aroi: "AROI",
    label_sl_variants: "SL Variants",
    label_target: "Target",
    label_stop: "Stop",
    
    icon_long: ICON_TREND_UP, 
    icon_short: ICON_TREND_DOWN, 

};
