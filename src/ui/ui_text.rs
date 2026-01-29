use std::sync::LazyLock;

pub const ICON_BAN: &str = "\u{f05e}"; // (Ban)
pub const ICON_FILTER: &str = "\u{f0b0}"; // (Filter)
pub const ICON_TREND_UP: &str = "\u{f0535}";
pub const ICON_TREND_DOWN: &str = "\u{f0533}";
pub const ICON_TARGET: &str = "\u{f04fe}";
pub const ICON_CLOCK: &str = "\u{f0954}"; // (Clock)
// pub const ICON_CANDLE:&str  = "\u{f05e2}"; // This is more like a full chart
pub const ICON_CANDLE: &str = "\u{f11c9}";
pub const ICON_POINT_RIGHT: &str = "\u{f02c7}";
pub const ICON_TWO_HORIZONTAL: &str = "\u{f12f0}";
pub const ICON_ONE_HORIZONTAL: &str = "\u{f45b}";
pub const ICON_CHART: &str = "\u{f1918}";
pub const ICON_LOCKED: &str = "\u{ea75}";
pub const ICON_UNLOCKED: &str = "\u{eb74}";
pub const ICON_Y_AXIS: &str = "\u{f0e79}";
pub const ICON_RUST: &str = "\u{e7a8}";
pub const ICON_DOLLAR_BAG: &str = "\u{ef8d}";
pub const ICON_PULSE: &str = "\u{e234}";
pub const ICON_RULER: &str = "\u{e21b}"; // "measuring" 
pub const ICON_COG: &str = "\u{f013}"; // "working" 
pub const ICON_QUEUE: &str = "\u{f1571}"; // queue sizes...
pub const ICON_HELP: &str = "\u{f02d6}";
pub const ICON_KEYBOARD: &str = "\u{f0313}";
pub const ICON_EYE: &str = "\u{f0208}";
// pub const ICON_CLOSE: &str = "\u{f00d}";
pub const ICON_CLOSE_ALL: &str = "\u{eac1}";
pub const ICON_TIME_MACHINE: &str = "\u{f11ef}";
pub const ICON_SIMULATE: &str = "\u{e63b}";
pub const ICON_EXPLAINER: &str = "\u{f00e}"; // Zoom in / inspect / explain
pub const ICON_WARNING: &str = "\u{ea6c}";
pub const ICON_SEGMENTED_TIME: &str = "\u{f084e}";
// (Sort arrows)
pub const ICON_SORT_ASC: &str = "\u{f0de}"; // (Sort Up)
pub const ICON_SORT_DESC: &str = "\u{f0dd}"; // (Sort Down)
pub const ICON_SORT: &str = "\u{f07d}"; // (Sort Neutral)

// pub const ICON_24_HRS:  &str = "\u{f1478}";
pub const ICON_CLOSE: &str = "\u{f00d}";
pub const ICON_PLUS_MINUS: &str = "\u{f14c9}";
pub const ICON_SEARCH: &str = "\u{f0978}";
pub const ICON_UNFILTERED: &str = "\u{f14ef}";
pub const ICON_RECENTER: &str = "\u{f0622}";
pub const ICON_TEST: &str = "\u{f0d2f}"; // Just for testing stuff out.

pub const ICON_STRATEGY_ROI:  &str = "\u{ef08}"; // For ROI strategy => high yield / long term / distasnce . Try e2a6 if ef08 looks nonce
pub const ICON_STRATEGY_AROI:  &str = "\u{f046e}"; // For AROI strategy => fast / speedy / sprint / turnover
pub const ICON_STRATEGY_BALANCED:  &str = "\u{f24e}"; // For balanced strategy => middle path / scale  - other scales are available
pub const ICON_STRATEGY: &str = "\u{ed5f}";

// pub const ICON_CROSSHAIR: &str  = "\u{f05b}"; // (Crosshairs)
// pub const ICON_GLOBE: &str      = "\u{f0ac}"; // (Globe)
// pub const ICON_TIME_LAPSE: &str = "\u{f051a}";
// These 2 unused yet but will be useful in future - up and down triangles
// pub const ICON_GAIN: &str   = "\u{f0d8}"; // d (Caret Up / Triangle Up)
// pub const ICON_LOSS: &str = "\u{f0d7}"; // d (Caret Down / Triangle Down)

pub struct UiText {
    pub icon_strategy: String,
    pub label_goal: String,
    pub icon_strategy_roi: String,
    pub icon_strategy_aroi: String,
    pub icon_strategy_balanced: String,

    pub icon_close: String,
    pub icon_sort_asc: String,
    pub icon_sort_desc: String,

    // --- Left panel ---
    // pub data_generation_heading: String,
    pub price_horizon_heading: String,
    pub lp_failed_gradient: String,

    // --- PLOT LABELS ---
    pub plot_x_axis: String,
    pub plot_x_axis_gap: String,
    pub plot_y_axis: String,
    pub plot_missing_klines: String,

    // --- ICONS/LABELS ---
    pub label_long: String,
    pub label_short: String,
    pub icon_long: String,
    pub icon_short: String,

    // Center panel
    pub cp_system_starting: String,
    pub cp_init_engine: String,
    pub cp_please_select_pair: String,
    pub cp_analyzing: String,
    pub cp_calculating_zones: String,
    pub cp_queued: String,
    pub cp_wait_thread: String,
    pub cp_wait_prices: String,
    pub cp_listen_binance_stream: String,

    // --- ERRORS ---
    pub error_insufficient_data: String,
    pub error_insufficient_data_body: String,
    pub error_no_model: String,
    pub error_no_pair_selected: String,
    pub error_analysis_failed: String,

    // --- PH HELP (Keep simple static slices for tables) ---
    pub ph_help_density_header: (&'static str, &'static str, &'static str),
    pub ph_help_density_rows: &'static [(&'static str, &'static str, &'static str)],
    pub ph_help_scope_header: (&'static str, &'static str, &'static str),
    pub ph_help_scope_rows: &'static [(&'static str, &'static str, &'static str)],
    pub ph_help_definitions: &'static [(&'static str, &'static str)],
    pub ph_label_evidence: String,
    pub ph_label_history: String,
    pub ph_label_density: String,
    pub ph_label_horizon_prefix: String,
    pub ph_startup: String,
    pub ph_definitions: String,
    pub ph_read_heatmap: String,
    pub ph_select_trade_style: String,

    // --- CR (Time Machine) NAVIGATOR ---
    pub cr_title_1: String,
    pub cr_title_2: String,
    pub cr_label_live: String,
    pub cr_label_historical: String,
    pub cr_nav_show_all: String,
    pub cr_nav_return_prefix: String,
    pub cr_nav_return_live: String,
    pub cr_date_range: String,
    pub cr_context: String,
    pub cr_price: String,
    pub cr_missing: String,
    pub cr_high: String,
    pub cr_low: String,
    pub cr_gap: String,
    pub cr_mixed: String,

    pub ph_help_title: String,

    pub tb_sticky: String,
    pub tb_low_wicks: String,
    pub tb_high_wicks: String,
    pub tb_volume_hist: String,
    pub tb_candles: String,
    pub tb_time: String,
    pub tb_gaps: String,
    pub tb_price_limits: String,
    pub tb_live_price: String,
    pub tb_targets: String,
    pub tb_y_locked: String,
    pub tb_y_unlocked: String,

    // --- TRADE FINDER ---
    pub tf_scope_all: String,
    pub tf_scope_selected: String,
    pub tf_time: String,

    // General use
    pub label_recenter: String,
    pub label_id: String,
    pub label_volume: String,
    pub label_volume_24h: String,
    pub label_pair: String,
    pub label_candle: String,
    pub label_momentum: String,
    pub label_momentum_short: String,
    pub label_volatility: String,
    pub label_volatility_short: String,
    pub label_success_rate: String,
    pub label_success_rate_short: String,
    pub label_roi: String,
    pub label_aroi: String,
    pub label_aroi_long: String,
    pub label_sl_variants: String,
    pub label_sl_variants_short: String,
    pub label_target: String,
    pub label_target_text: String,
    pub label_active_target_text: String,
    pub label_source_ph: String,
    pub label_targets_text: String,
    pub label_select_pair: String,
    pub label_no_targets: String,
    pub label_stop_loss: String,
    pub label_stop_loss_short: String,
    pub label_risk_reward: String,
    pub label_limit: String,
    pub label_no_opps: String,
    pub label_connecting: String,
    pub label_connected: String,
    pub label_working: String,
    pub label_queue: String,
    pub label_warning: String,
    pub label_failures: String,

    // Icons
    pub icon_help: String,

    pub opp_exp_current_opp: String,
    pub opp_exp_setup_type: String,
    pub opp_exp_expectancy: String,
    pub opp_exp_market_context: String,
    pub opp_exp_trend_measured: String,
    pub opp_exp_trend_length: String,
    pub opp_exp_relative_volume: String,
    pub opp_exp_relative_volume_explainer: String,
    pub opp_exp_trade_setup: String,
    pub opp_exp_trade_entry: String,
    pub opp_exp_order_time_limit: String,
    pub opp_exp_how_this_works: String,
    pub opp_expr_we_fingerprinted: String,
    pub opp_exp_scanned_history_one: String,
    pub opp_exp_scanned_history_two: String,
    pub opp_exp_scanned_history_three: String,
    pub opp_exp_scanned_history_four: String,
    pub opp_exp_simulate_one: String,
    pub opp_exp_simulate_two: String,
    pub opp_exp_out_of_time: String,
    pub opp_exp_cases_one: String,
    pub opp_exp_cases_two: String,
    pub opp_exp_cases_three: String,
    pub opp_exp_cases_four: String,
    pub opp_exp_cases_five: String,

    // --- STATUS Panel ---
    pub sp_price: String,
    pub sp_live_mode: String,
    pub sp_zone_size: String,
    pub sp_coverage: String,
    pub sp_coverage_sticky: String,
    pub sp_coverage_support: String,
    pub sp_coverage_resistance: String,
    pub sp_stream_status: String,

    // Simulation stuff
    #[cfg(target_arch = "wasm32")]
    pub sp_simulation_mode: String,
    #[cfg(not(target_arch = "wasm32"))]
    pub sp_simulation_mode: String,

    pub sim_help_sim_toggle_direction: String,
    pub sim_help_sim_step_size: String,
    pub sim_help_sim_activate_price_change: String,
    pub sim_help_sim_jump_hvz: String,
    pub sim_help_sim_jump_lower_wicks: String,
    pub sim_help_sim_jump_higher_wicks: String,
    pub sim_mode_controls: String,
    pub sim_step: String,

    pub kbs_name_long: String,
    pub kbs_name: String,
    pub kbs_open_close: String,
    pub kbs_close_all_panes: String,
    pub kbs_view_opp_explainer: String,
    pub kbs_view_time_machine: String,
    pub kbs_toolbar_shortcut_hvz: String,
    pub kbs_toolbar_shortcut_low_wick: String,
    pub kbs_toolbar_shortcut_high_wick: String,
    pub kbs_toolbar_shortcut_histogram: String,
    pub kbs_toolbar_shortcut_candles: String,
    pub kbs_toolbar_shortcut_gap: String,
    pub kbs_toolbar_shortcut_price_limits: String,
    pub kbs_toolbar_shortcut_live_price: String,
    pub kbs_toolbar_shortcut_targets: String,

    pub kbs_sim_mode: String,

    pub ls_title: String,
    pub ls_syncing: String,
    pub ls_main: String,
    pub ls_failed: String,

    pub label_avg_duration: String,
    pub label_risk_select: String,

    pub hover_scroll_to_selected_target: String,
}

// THE SINGLETON
pub static UI_TEXT: LazyLock<UiText> = LazyLock::new(|| {
    UiText {
        label_goal: "Goal".to_string(),
        icon_strategy: ICON_STRATEGY.to_string(),
        icon_strategy_roi: ICON_STRATEGY_ROI.to_string(),
        icon_strategy_aroi: ICON_STRATEGY_AROI.to_string(),
        icon_strategy_balanced: ICON_STRATEGY_BALANCED.to_string(),

        label_recenter: ICON_RECENTER.to_string(),
        hover_scroll_to_selected_target: "Scroll to Selected Target".to_string(),
        icon_close: ICON_CLOSE.to_string(),
        label_pair: "Pair".to_string(),
        icon_sort_asc: ICON_SORT_ASC.to_string(),
        icon_sort_desc: ICON_SORT_DESC.to_string(),

        // Status panel
        sp_price: ICON_DOLLAR_BAG.to_string(),
        sp_live_mode: ICON_PULSE.to_string() + " LIVE MODE",

        sp_zone_size: ICON_RULER.to_string() + " Zone Size",
        sp_coverage: "Coverage".to_string(),
        sp_coverage_sticky: "High Volume".to_string(),
        sp_coverage_support: "Support".to_string(),
        sp_coverage_resistance: "Resist.".to_string(),
        sp_stream_status: "Stream Status".to_string(),

        // Simulation
        sp_simulation_mode: if cfg!(target_arch = "wasm32") {
            "WEB DEMO (OFFLINE)"
        } else {
            "SIMULATION MODE"
        }
        .to_string(),

        // Simulation help text (part of main help panel)
        // sim_mode_name: "Simulation Mode".to_string(),
        sim_mode_controls: "Simulation Mode Controls".to_string(),
        sim_help_sim_toggle_direction: "Toggle price direction (UP / DOWN)".to_string(),
        sim_help_sim_step_size: "Cycle step size (0.1% -> 1% -> 5% -> 10%)".to_string(),
        sim_help_sim_activate_price_change: "Activate price change".to_string(),
        sim_help_sim_jump_hvz: "Jump to next High Volume Zone".to_string(),
        sim_help_sim_jump_lower_wicks: "Jump to next Demand Zone".to_string(),
        sim_help_sim_jump_higher_wicks: "Jump to next Supply Zone".to_string(),
        sim_step: "Step".to_string(),

        // Price Horion Help Panel
        ph_help_title: "Price Horizon Guide".to_string(),
        ph_label_evidence: "Evidence".to_string(),
        ph_label_history: "History".to_string(),
        ph_label_density: "Density".to_string(),
        ph_label_horizon_prefix: ICON_PLUS_MINUS.to_string(),
        ph_startup: "Analyzing Price Structure...".to_string(),
        ph_definitions: "Definitions".to_string(),
        ph_read_heatmap: "1. Reading the Heatmap (Data Density)".to_string(),
        ph_select_trade_style: "2. Selecting your Scope (Trade Style)".to_string(),

        ph_help_density_header: ("Color", "Density", "Significance"),
        ph_help_density_rows: &[
            ("Deep Purple", "Low (< 10%)", "Insignificant (noise)"),
            ("Orange/Red", "Medium", "Standard Confidence"),
            ("Bright Yellow", "High (> 80%)", "High Significance"),
        ],
        ph_help_scope_header: ("Horizon %", "Style", "Focus"),
        ph_help_scope_rows: &[
            ("< 5%", "Sniper / Scalp", "Immediate price action"),
            ("5% - 15%", "Swing Trade", "Balanced history"),
            ("> 15%", "Macro / Invest", "Deep structure"),
        ],
        ph_help_definitions: &[
            ("Evidence", "Total duration of actual data."),
            ("History", "Calendar time elapsed."),
            ("Density", "Ratio of Evidence to History."),
        ],

        // Opportunity explainer
        opp_exp_current_opp: "Current Opportunity".to_string(),
        opp_exp_setup_type: "Setup Type".to_string(),
        opp_exp_expectancy: "Expectancy & Return".to_string(),
        opp_exp_market_context: "Market Context (Used to find similar trade setups)".to_string(),
        opp_exp_trend_measured: "Trend measured over the last".to_string(),
        opp_exp_trend_length: "Trend window length derived from PH of".to_string(),
        opp_exp_relative_volume: "Relative Volume".to_string(),
        opp_exp_relative_volume_explainer: " (Ratio of Current Volume vs Recent Average.)"
            .to_string(),
        opp_exp_trade_setup: "Trade Setup".to_string(),
        opp_exp_trade_entry: "Entry".to_string(),
        opp_exp_order_time_limit: "Order Time Limit".to_string(),
        opp_exp_how_this_works: "How This Works".to_string(),
        opp_expr_we_fingerprinted: "1. We fingerprinted the market right now".to_string(),
        opp_exp_scanned_history_one: "2. We scanned history and found exactly".to_string(),
        opp_exp_scanned_history_two: "periods that matched this fingerprint.".to_string(),
        opp_exp_scanned_history_three:
            "2. We scanned history and found many matches, but we kept only the top".to_string(),
        opp_exp_scanned_history_four: "closest matches.".to_string(),
        opp_exp_simulate_one: "3. We simulated these".to_string(),
        opp_exp_simulate_two: "scenarios. We checked if price hit the".to_string(),
        opp_exp_out_of_time: "or ran out of time".to_string(),
        opp_exp_cases_one: "4. In".to_string(),
        opp_exp_cases_two: "of those".to_string(),
        opp_exp_cases_three: "cases, price hit the".to_string(),
        opp_exp_cases_four: "first. This produces the".to_string(),
        opp_exp_cases_five: "you see above.".to_string(),

        // General use labels (not specific to one panel)
        label_id: "ID".to_string(),
        label_volume: "Volume".to_string(),
        label_volume_24h: format!("{}\n{}", "24h", "Vol."),
        label_queue: ICON_QUEUE.to_string(),
        label_working: ICON_COG.to_string(),
        label_connecting: "Connecting".to_string(),
        label_connected: "connected".to_string(),
        label_volatility: "Volatility".to_string(),
        label_volatility_short: "VL".to_string(),
        label_momentum: "Momentum".to_string(),
        label_momentum_short: "Mom.".to_string(),
        label_target: ICON_TARGET.to_string(),
        label_target_text: "Target".to_string(),
        label_active_target_text: "Active Target".to_string(),
        label_targets_text: "Targets".to_string(),
        label_source_ph: "Source: PH".to_string(),
        label_select_pair: "Select a pair from the list below".to_string(),
        label_no_targets: "No Active Targets".to_string(),
        label_success_rate: "Success Rate".to_string(),
        label_success_rate_short: "Succ.".to_string(),
        label_roi: "ROI".to_string(),
        label_aroi: "AROI".to_string(),
        label_aroi_long: "AROI (Annualized RoI)".to_string(),
        label_sl_variants: "Variants".to_string(),
        label_sl_variants_short: "Vrts.".to_string(),
        label_stop_loss: "Stop Loss".to_string(),
        label_stop_loss_short: "S/L".to_string(),
        label_risk_reward: "Risk/Reward Ratio".to_string(),
        label_long: format!("LONG {}", ICON_TREND_UP),
        label_short: format!("SHORT {}", ICON_TREND_DOWN),
        label_limit: "Limit".to_string(),
        icon_long: ICON_TREND_UP.to_string(),
        icon_short: ICON_TREND_DOWN.to_string(),
        label_no_opps:
            "No valid opportunities found. Please reset filters or select a different pair."
                .to_string(),
        label_warning: ICON_WARNING.to_string(),
        label_failures: "failures".to_string(),

        // TradeFinder Pane
        tf_scope_all: "ALL PAIRS".to_string(),
        tf_scope_selected: "ONLY".to_string(),
        label_candle: ICON_CANDLE.to_string(),
        tf_time: ICON_CLOCK.to_string(),

        // --- Left Panel ---
        // data_generation_heading: "Shape Your Trades".to_string(),
        price_horizon_heading: "Price Horizon".to_string(),
        lp_failed_gradient: "Failed to build Price Horizon Gradient".to_string(),

        // Loading screen
        ls_title: "ZONE SNIPER INITIALIZATION".to_string(),
        ls_syncing: "Syncing".to_string(),
        ls_failed: "FAILED".to_string(),
        ls_main: "klines from Binance Public API. Please be patient. This may take some time if it hasn't been run for a while or you are collecting many pairs. Subsequent runs will complete much quicker.".to_string(),

        // Center panel i.e. where the plot goes
        cp_system_starting: "System Starting...".to_string(),
        cp_init_engine: "Initializing Engine".to_string(),
        cp_please_select_pair: "Please select a pair.".to_string(),
        cp_analyzing: "Analyzing".to_string(),
        cp_calculating_zones: "Calculating Zones...".to_string(),
        cp_queued: "Queued".to_string(),
        cp_wait_thread: "Waiting for worker thread...".to_string(),
        cp_wait_prices: "Waiting for Prices...".to_string(),
        cp_listen_binance_stream: "Listening to Binance Stream...".to_string(),

        // Actual Plot
        plot_y_axis: "Price".to_string(),
        plot_x_axis: "Segmented Time ".to_string() + ICON_SEGMENTED_TIME,
        plot_x_axis_gap: "GAP".to_string(),
        plot_missing_klines: "OHLCV kline data missing for current model".to_string(),

        // --- ERRORS ---
        error_analysis_failed: "Analysis Failed".to_string(),
        error_no_model: "No model loaded.".to_string(),
        error_no_pair_selected: "No pair selected.".to_string(),
        error_insufficient_data: "Insufficient data".to_string(),
        error_insufficient_data_body:
            "The current Time Tuner selection does enough price history for the pair in question.\n\n".to_string()
                + ICON_POINT_RIGHT
                + " Select a more inclusive button on the Time Tuner i.e. click a button to the right of currently selected Time Tuner button.",

        // --- Candle Range (Time Machine) NAVIGATOR ---
        cr_title_1: "Time Machine".to_string(),
        cr_title_2: "Candle Ranges".to_string(),
        cr_label_live: "LIVE".to_string(),
        cr_label_historical: "Historical".to_string(),
        cr_nav_show_all: "SHOW ALL RANGES".to_string(),
        cr_nav_return_prefix: "RETURN TO SEGMENT".to_string(),
        cr_nav_return_live: "RETURN TO LIVE".to_string(),
        cr_date_range: "Date Range".to_string(),
        cr_context: "Context".to_string(),
        cr_price: "Price".to_string(),
        cr_missing: "Missing".to_string(),
        cr_high: "High".to_string(),
        cr_low: "Low".to_string(),
        cr_gap: "Gap".to_string(),
        cr_mixed: "Mixed".to_string(),

        // Icons
        icon_help: ICON_HELP.to_string(),

        // Toolbar
        tb_time: ICON_CLOCK.to_string(),
        tb_sticky: "High Volume Zones".to_string(),
        tb_low_wicks: "Lower Wicks".to_string(),
        tb_high_wicks: "Higher Wicks".to_string(),
        tb_volume_hist: "Volume Hist.".to_string(),
        tb_candles: ICON_CANDLE.to_string(),
        tb_gaps: "Data Gap".to_string(),
        tb_price_limits: "PH Boundary".to_string() + " " + ICON_TWO_HORIZONTAL,
        tb_live_price: "Live Price".to_string() + " " + ICON_ONE_HORIZONTAL,
        tb_targets: ICON_TARGET.to_string(),
        tb_y_locked: ICON_Y_AXIS.to_string() + " " + ICON_LOCKED,
        tb_y_unlocked: ICON_Y_AXIS.to_string() + " " + ICON_UNLOCKED,

        // Keyboard Shortcuts Pane
        kbs_name_long: ICON_KEYBOARD.to_string() + " Keyboard Shortcuts",
        kbs_name: "Keyboard Shortcuts".to_string(),

        kbs_toolbar_shortcut_hvz: format!("{} High Volume Zones", ICON_EYE),
        kbs_toolbar_shortcut_low_wick: format!("{} Lower Wick Zones", ICON_EYE),
        kbs_toolbar_shortcut_high_wick: format!("{} Higher Wick Zones", ICON_EYE),
        kbs_toolbar_shortcut_histogram: format!("{} Volume Hist.", ICON_EYE),
        kbs_toolbar_shortcut_candles: format!("{} {}", ICON_EYE, ICON_CANDLE),
        kbs_toolbar_shortcut_gap: format!("{} Data Gap", ICON_EYE),
        kbs_toolbar_shortcut_price_limits: format!("{} PH Boundary", ICON_EYE),
        kbs_toolbar_shortcut_live_price: format!("{} Live Price", ICON_EYE),
        kbs_toolbar_shortcut_targets: format!("{} Targets", ICON_EYE),

        kbs_view_opp_explainer: format!("{} Opportunity Explainer", ICON_EXPLAINER),
        kbs_close_all_panes: format!("{} Close all open overlay panes", ICON_CLOSE_ALL),
        kbs_open_close: format!("{} Keyboard Shortcuts", ICON_KEYBOARD),
        kbs_view_time_machine: format!("{} Time Machine Pane", ICON_TIME_MACHINE),

        kbs_sim_mode: format!("{} Simulation Mode", ICON_SIMULATE),

        label_avg_duration: "Avg. Duration".to_string(),
        label_risk_select: "Stop Loss Variants".to_string(),
    }
});
