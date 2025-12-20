pub struct UiText {
    pub data_generation_heading: &'static str,
    pub price_horizon_heading: &'static str,
    pub time_horizon_heading: &'static str,
    pub pair_selector_heading: &'static str,
    pub view_options_heading: &'static str,
    pub view_data_source_heading: &'static str,
    pub signals_heading: &'static str,
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
    pub label_lower_wick_count: &'static str,
    pub label_upper_wick_count: &'static str,
    pub label_hvz: &'static str,
    pub label_lower_wick_zones: &'static str,
    pub label_upper_wick_zones: &'static str,
    pub label_hvz_above: &'static str,
    pub label_hvz_beneath: &'static str,
    pub label_hvz_within: &'static str,
    pub label_help_background: &'static str,
    pub label_help_sim_toggle_direction: &'static str,
    pub label_help_sim_step_size: &'static str,
    pub label_help_sim_activate_price_change: &'static str,
    pub label_help_sim_jump_hvz: &'static str,
    pub label_help_sim_jump_lower_wicks: &'static str,
    pub label_help_sim_jump_higher_wicks: &'static str,

    // Status Bar Labels
    pub label_candle_count: &'static str,
    pub label_volatility: &'static str,

    // Error Messages
    pub error_insufficient_data_title: &'static str,
    pub error_insufficient_data_body: &'static str,

    // Add generic terms (reusable across Status Bar and Slider)
    pub word_candle_singular: &'static str,
    pub word_candle_plural: &'static str,

    // pub ph_label_context: &'static str,
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

    pub cr_title: &'static str,
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
}

pub const UI_TEXT: UiText = UiText {
    data_generation_heading: "Shape Your Trades",
    price_horizon_heading: "Price Horizon",
    time_horizon_heading: "Time Horizon",
    pair_selector_heading: "Select Plot Pair",
    view_options_heading: "View Options",
    view_data_source_heading: "Data Source",
    signals_heading: "🎯 Signals",

    price_horizon_helper_prefix: "Focus on price action within ±",
    price_horizon_helper_suffix: "% of current price",
    time_horizon_helper_prefix: "Focus on trades that complete within ",
    time_horizon_helper_suffix: " days",

    plot_y_axis: "Price",
    plot_x_axis: "Key Zone Strength (0 % of the strongest zone)",
    plot_strongest_zone: "of strongest zone",
    plot_this_zone_is: "This zone is",

    label_volume: "Trading Volume",
    label_lower_wick_count: "Lower Wick Strength",
    label_upper_wick_count: "Upper Wick Strength",
    label_hvz: "High Volume Zones",
    label_lower_wick_zones: "Lower Wick Zones",
    label_upper_wick_zones: "Upper Wick Zones",
    label_reversal_support: "Demand Zone (Buyers Here)",
    label_reversal_resistance: "Supply Zone (Sellers Here)",
    label_hvz_above: "`High Volume Zone` is above (if bullish, acts as future target price)",
    label_hvz_beneath: "`High Volume Zone` is below (if bearish, acts as future target price)",
    label_hvz_within: "Inside `High Volume Zone` now (consolidating...)",

    label_help_background: "Rotate Background Data Selection (between (1) Trading Volume, (2) Lower Wick Strength (Find Demand Zones), (3) Upper Wick Strength (Find Supply Zones)",
    label_help_sim_toggle_direction: "Toggle direction (⬆️ UP / ⬇️ DOWN)",
    label_help_sim_step_size: "Cycle step size (0.1% → 1% → 5% → 10%)",
    label_help_sim_activate_price_change: "Activate price change in current direction",
    label_help_sim_jump_hvz: "Jump to next High Volume Zone",
    label_help_sim_jump_lower_wicks: "Jump to next Demand Zone",
    label_help_sim_jump_higher_wicks: "Jump to next Supply Zone",

    label_candle_count: "Candles",
    label_volatility: "Volatility (Avg True Range)",

    error_insufficient_data_title: "Analysis Paused: Range Too Narrow",

    error_insufficient_data_body: "The current Price Horizon does not capture enough price history to identify reliable zones.\n\n\
                                   👉 Action: Drag the Price Horizon slider to the right (aim for High Density / Yellow areas).",

    word_candle_singular: "Candle",
    word_candle_plural: "Candles",

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
            "Statistically Insignificant (Noise)",
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
            "Immediate price action (High Decay)",
        ),
        (
            "5% - 15%",
            "Swing Trade",
            "Balanced recent history (Med Decay)",
        ),
        (
            "> 15%",
            "Macro / Invest",
            "Deep historical structure (Low Decay)",
        ),
    ],

    ph_help_definitions: &[
        (
            "Evidence",
            "Total duration of actual data (candle count x interval).",
        ),
        (
            "History",
            "Calendar time elapsed between the first and last candle.",
        ),
        (
            "Density",
            "Ratio of Evidence to History. (Yellow = High Data Quality).",
        ),
    ],

    // --- CANDLE RANGE NAVIGATOR (CR) ---
    cr_title: "Candle Ranges (Accordion)",

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
    cr_gap_missing_source: "Missing Data (Exchange)",
    cr_gap_price_above: "Price > Horizon",
    cr_gap_price_below: "Price < Horizon",
    cr_gap_mixed: "Mixed Gap",
};
