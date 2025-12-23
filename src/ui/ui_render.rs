use eframe::egui::{
    CentralPanel, Color32, Context, Frame, Grid, RichText, ScrollArea, SidePanel,
    TopBottomPanel, Ui, Window,
};

use crate::config::{ANALYSIS, AnalysisConfig};
use crate::models::cva::ScoreType;

use crate::ui::app::CandleResolution;
use crate::ui::config::{UI_CONFIG, UI_TEXT};
use crate::ui::styles::UiStyleExt;
use crate::ui::ui_panels::{
    CandleRangePanel, DataGenerationEventChanged, DataGenerationPanel, Panel, SignalsPanel,
};

use crate::ui::utils::format_price;

use crate::utils::TimeUtils;

use super::app::ZoneSniperApp;

impl ZoneSniperApp {
    pub(super) fn render_right_panel(&mut self, ctx: &Context) {
        let frame = UI_CONFIG.side_panel_frame();

        SidePanel::right("right_panel")
            .min_width(300.0)
            .resizable(true)
            .frame(frame)
            .show(ctx, |ui| {
                ui.add_space(5.0);

                // Safety check for Engine/Model
                if let Some(engine) = &self.engine {
                    if let Some(pair) = &self.selected_pair {
                        if let Some(model) = engine.get_model(pair) {

                            let mut nav = self.get_nav_state();
                            let max_idx = model.segments.len().saturating_sub(1);
                            let safe_last = nav.last_viewed_segment_idx.min(max_idx);

                            // We pass None for current_idx for now (interactive logic comes next)
                            let mut panel = CandleRangePanel::new(&model.segments, nav.current_segment_idx);

                            if let Some(new_idx) = panel.render(ui, safe_last) {
                                nav.current_segment_idx = new_idx;
                                // If we switched to a specific segment (not "Show All"), remember it.
                                if let Some(idx) = new_idx {
                                    nav.last_viewed_segment_idx = idx;
                                }
                                // Write back to app state
                                self.set_nav_state(nav);

                                // Maybe trigger a repaint or scroll?
                                ctx.request_repaint();
                                #[cfg(debug_assertions)]
                                log::info!("Clicked Segment Index: {:?}", new_idx);
                            }
                        } else {
                            ui.label("No model loaded.");
                        }
                    } else {
                        ui.label("No pair selected.");
                    }
                }
            });
    }

    pub(super) fn render_left_panel(&mut self, ctx: &Context) {
        let frame = UI_CONFIG.side_panel_frame();

        SidePanel::left("left_panel")
            .min_width(140.0)
            .resizable(false)
            .frame(frame)
            .show(ctx, |ui| {
                let mut opp_events = Vec::new();

                let data_events = self.data_generation_panel(ui);

                ScrollArea::vertical()
                    .max_height(500.)
                    .id_salt("signal_panel")
                    .show(ui, |ui| {
                        opp_events = self.signals_panel(ui);
                    });
                for pair_name in opp_events {
                    self.handle_pair_selection(pair_name);
                }

                for event in data_events {
                    match event {
                        DataGenerationEventChanged::Pair(new_pair) => {
                            self.handle_pair_selection(new_pair);
                        }
                        DataGenerationEventChanged::PriceHorizonThreshold(new_threshold) => {
                            let prev = self.app_config.price_horizon.threshold_pct;
                            if (prev - new_threshold).abs() > f64::EPSILON {
                                self.app_config.price_horizon.threshold_pct = new_threshold;

                                // --- ADAPTIVE DECAY LOGIC ---
                                // "Drag left to Snipe (High Decay), Drag right to Invest (Low Decay)"
                                let new_decay = AnalysisConfig::calculate_time_decay(new_threshold);

                                // Apply only if changed (prevents log spam if dragging within same band)
                                if (self.app_config.time_decay_factor - new_decay).abs()
                                    > f64::EPSILON
                                {
                                    self.app_config.time_decay_factor = new_decay;
                                }
                                // -----------------------------
                                self.invalidate_all_pairs_for_global_change(
                                    "price horizon threshold changed",
                                );
                            }
                        }
                    }
                }
            });
    }

    pub(super) fn render_top_panel(&mut self, ctx: &Context) {
        
        let frame = UI_CONFIG.top_panel_frame();

        TopBottomPanel::top("top_toolbar").frame(frame).min_height(30.0).resizable(false).show(ctx, |ui| {
                            // --- TOP TOOLBAR ---
            ui.horizontal(|ui| {
                // 1. CANDLE RESOLUTION
                ui.label("Res:");
                ui.selectable_value(&mut self.candle_resolution, CandleResolution::M5, "5m");
                ui.selectable_value(&mut self.candle_resolution, CandleResolution::M15, "15m");
                ui.selectable_value(&mut self.candle_resolution, CandleResolution::H1, "1h");
                ui.selectable_value(&mut self.candle_resolution, CandleResolution::H4, "4h");
                ui.selectable_value(&mut self.candle_resolution, CandleResolution::D1, "1D");
                ui.selectable_value(&mut self.candle_resolution, CandleResolution::D3, "3D");
                ui.selectable_value(&mut self.candle_resolution, CandleResolution::W1, "1W");
                ui.selectable_value(&mut self.candle_resolution, CandleResolution::M1, "1M");

                ui.separator();

                // 2. LAYER VISIBILITY
                ui.checkbox(&mut self.plot_visibility.sticky, "Sticky");
                ui.checkbox(&mut self.plot_visibility.low_wicks, "Demand");
                ui.checkbox(&mut self.plot_visibility.high_wicks, "Supply");
                ui.checkbox(&mut self.plot_visibility.background, "Volume Hist");
                ui.checkbox(&mut self.plot_visibility.candles, "Candles");
                
                ui.separator();

                // CONTEXT
                ui.checkbox(&mut self.plot_visibility.ghost_candles, "Ghosts"); // Toggle faint candles
                ui.checkbox(&mut self.plot_visibility.horizon_lines, "PH Lines"); // Toggle dashed horizontal PH border lines
                ui.checkbox(&mut self.plot_visibility.price_line, "Price");
            });
        });
    }

    pub(super) fn render_central_panel(&mut self, ctx: &Context) {
        let central_panel_frame = Frame::new().fill(UI_CONFIG.colors.central_panel);

        CentralPanel::default()
            .frame(central_panel_frame)
            .show(ctx, |ui| {

                // FIX: Grab Nav State HERE (Before borrowing self.engine)
                // This requires us to clone it because it's Copy/Clone
                let nav_state = self.get_nav_state();

                // 1. Safety Check: Engine existence
                let Some(engine) = &self.engine else {
                    render_fullscreen_message(
                        ui,
                        "System Starting...",
                        "Initializing Engine",
                        false,
                    );
                    return;
                };

                // 2. Safety Check: Selected Pair
                let Some(pair) = self.selected_pair.clone() else {
                    render_fullscreen_message(
                        ui,
                        "No Pair Selected",
                        "Select a pair on the left.",
                        false,
                    );
                    return;
                };

                // 3. Get Price State (Do we have a live price?)
                let current_price = self.get_display_price(&pair); // engine.get_price(&pair);

                let (is_calculating, last_error) = engine.get_pair_status(&pair);

                // PRIORITY 1: ERRORS
                // If the most recent calculation failed (e.g. "Insufficient Data" at 1%),
                // we must show the error, even if we have an old cached model.
                // The old model is valid for the OLD settings, not the CURRENT ones.
                if let Some(err_msg) = last_error {
                    // Use rich error format
                    let body = if err_msg.contains("Insufficient data") {
                        // Use our detailed help text
                        format!("{}\n\n{}", UI_TEXT.error_insufficient_data_body, err_msg)
                    } else {
                        err_msg.to_string()
                    };
                    render_fullscreen_message(ui, "Analysis Failed", &body, true);
                }
                // PRIORITY 2: VALID MODEL
                // If no error, and we have data, draw it.
                else if let Some(model) = engine.get_model(&pair) {

                    self.plot_view.show_my_plot(
                        ui,
                        &model.cva,
                        &model,
                        current_price,
                        ScoreType::FullCandleTVW,
                        &self.plot_visibility,
                        engine,
                        self.candle_resolution,
                        nav_state.current_segment_idx,
                    );
                }
                // PRIORITY 3: CALCULATING (Initial Load)
                else if is_calculating {
                    render_fullscreen_message(
                        ui,
                        &format!("Analyzing {}...", pair),
                        "Calculating Zones...",
                        false,
                    );
                }
                // PRIORITY 4: QUEUED / WAITING
                else if current_price.is_some() {
                    render_fullscreen_message(
                        ui,
                        &format!("Queued: {}...", pair),
                        "Waiting for worker thread...",
                        false,
                    );
                }
                // PRIORITY 5: NO DATA STREAM
                else {
                    render_fullscreen_message(
                        ui,
                        "Waiting for Price...",
                        "Listening to Binance Stream...",
                        false,
                    );
                }
            });
    }

    pub(super) fn render_status_panel(&mut self, ctx: &Context) {
        let frame = UI_CONFIG.bottom_panel_frame();
        TopBottomPanel::bottom("status_panel")
            .frame(frame)
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        // 2. Simulation Mode / Live Price Logic
                        if let Some(pair) = &self.selected_pair.clone() {
                            if self.is_simulation_mode() {
                                #[cfg(target_arch = "wasm32")]
                                let label = "WEB DEMO (OFFLINE)";
                                #[cfg(not(target_arch = "wasm32"))]
                                let label = "SIMULATION MODE";
                                // --- SIMULATION MODE UI ---
                                ui.label(
                                    RichText::new(label)
                                        .strong()
                                        .color(Color32::from_rgb(255, 150, 0)),
                                );
                                ui.separator();

                                // Sim Controls Display
                                ui.label(
                                    RichText::new(format!("{}", self.sim_direction))
                                        .small()
                                        .color(Color32::from_rgb(200, 200, 255)),
                                );
                                ui.separator();
                                ui.label(
                                    RichText::new(format!("| Step: {}", self.sim_step_size))
                                        .small()
                                        .color(Color32::from_rgb(100, 200, 100)),
                                );
                                ui.separator();

                                if let Some(sim_price) = self.simulated_prices.get(pair) {
                                    ui.label(
                                        RichText::new(format!("💰 {}", format_price(*sim_price)))
                                            .strong()
                                            .color(Color32::from_rgb(255, 200, 100)),
                                    );
                                }
                            } else {
                                // --- FIX: LIVE MODE UI ---
                                // This else block was missing/empty in previous versions
                                ui.label(
                                    RichText::new("🟢 LIVE MODE").small().color(Color32::GREEN),
                                );
                                ui.separator();

                                if let Some(price) = self.get_display_price(pair) {
                                    ui.label(
                                        RichText::new(format!("💰 {}", format_price(price)))
                                            .strong()
                                            .color(Color32::from_rgb(100, 200, 100)), // Light Green
                                    );
                                } else {
                                    ui.label("Connecting...");
                                }
                            }
                        }

                        // 3. Zone Size
                        if let Some(engine) = &self.engine {
                            if let Some(pair) = &self.selected_pair {
                                if let Some(model) = engine.get_model(pair) {
                                    let cva = &model.cva;
                                    let zone_size = (cva.price_range.end_range
                                        - cva.price_range.start_range)
                                        / cva.zone_count as f64;

                                    ui.metric(
                                        "📏 Zone Size",
                                        &format!(
                                            "{} (N={})",
                                            format_price(zone_size),
                                            cva.zone_count
                                        ),
                                        Color32::from_rgb(180, 200, 255),
                                    );
                                    ui.separator();
                                }
                            }
                        }

                        ui.separator();

                        // // 4. Background View Mode
                        // ui.label_subdued("Background plot view:");
                        // let (text, color) = if self.plot_visibility.background {
                        //     let t = match self.debug_background_mode {
                        //         ScoreType::FullCandleTVW => UI_TEXT.label_volume,
                        //         ScoreType::LowWickCount => UI_TEXT.label_lower_wick_count,
                        //         ScoreType::HighWickCount => UI_TEXT.label_upper_wick_count,
                        //     };
                        //     (t, Color32::from_rgb(0, 255, 255)) // Cyan
                        // } else {
                        //     ("HIDDEN", Color32::DARK_GRAY)
                        // };
                        // ui.label(RichText::new(text).small().color(color));
                        // ui.separator();

                        // Coverage Statistics
                        // 4. Coverage Statistics
                        if let Some(engine) = &self.engine {
                            if let Some(pair) = &self.selected_pair {
                                if let Some(model) = engine.get_model(pair) {
                                    // Helper to color-code coverage
                                    // > 30% is Red (Too much), < 5% is Yellow (Too little?), Green is good
                                    let cov_color = |pct: f64| {
                                        if pct > 30.0 {
                                            Color32::from_rgb(255, 100, 100) // Red
                                        } else if pct < 5.0 {
                                            Color32::from_rgb(255, 215, 0) // Yellow
                                        } else {
                                            Color32::from_rgb(150, 255, 150) // Green
                                        }
                                    };

                                    ui.label_subdued("Coverage");

                                    ui.metric(
                                        "Sticky",
                                        &format!("{:.0}%", model.coverage.sticky_pct),
                                        cov_color(model.coverage.sticky_pct),
                                    );

                                    ui.metric(
                                        "R-Sup",
                                        &format!("{:.0}%", model.coverage.support_pct),
                                        cov_color(model.coverage.support_pct),
                                    );

                                    ui.metric(
                                        "R-Res",
                                        &format!("{:.0}%", model.coverage.resistance_pct),
                                        cov_color(model.coverage.resistance_pct),
                                    );

                                    ui.separator();
                                }
                            }
                        }

                        // ... inside status panel, after coverage stats ...

                        // 5. Candle & Volatility Stats
                        if let Some(engine) = &self.engine {
                            if let Some(pair) = &self.selected_pair {
                                if let Some(model) = engine.get_model(pair) {
                                    ui.separator();

                                    // Candle Count: "129 / 3923 (14%) 30m"
                                    let relevant = model.cva.relevant_candle_count;
                                    let total = model.cva.total_candles;
                                    let pct = if total > 0 {
                                        (relevant as f64 / total as f64) * 100.0
                                    } else {
                                        0.0
                                    };

                                    let time_str =
                                        TimeUtils::interval_to_string(model.cva.interval_ms);

                                    ui.metric(
                                        UI_TEXT.label_candle_count,
                                        &format!(
                                            "{}/{} ({:.1}%) {}",
                                            relevant, total, pct, time_str
                                        ),
                                        Color32::LIGHT_GRAY,
                                    );

                                    ui.separator();

                                    // Volatility
                                    ui.metric(
                                        UI_TEXT.label_volatility,
                                        &format!("{:.3}%", model.cva.volatility_pct),
                                        Color32::from_rgb(200, 200, 100), // Khaki/Gold
                                    );
                                }
                            }
                        }

                        // 5. System Status
                        if let Some(engine) = &self.engine {
                            let total_pairs = engine.get_active_pair_count();
                            ui.metric("📊 Pairs", &format!("{}", total_pairs), Color32::LIGHT_GRAY);

                            // Worker Status
                            if let Some(msg) = engine.get_worker_status_msg() {
                                ui.separator();
                                ui.label(
                                    RichText::new(format!("⚙ {}", msg))
                                        .small()
                                        .color(Color32::from_rgb(255, 165, 0)), // Orange
                                );
                            }

                            // Queue Size
                            let q_len = engine.get_queue_len();
                            if q_len > 0 {
                                ui.separator();
                                ui.label(
                                    RichText::new(format!("Queue: {}", q_len))
                                        .small()
                                        .color(Color32::YELLOW),
                                );
                            }
                        }

                        ui.separator();

                        // 8. Network health
                        if let Some(engine) = &self.engine {
                            let health = engine.price_stream.connection_health();
                            let (icon, color) = if health >= 90.0 {
                                ("🟢", Color32::from_rgb(0, 200, 0))
                            } else if health >= 50.0 {
                                ("🟡", Color32::from_rgb(200, 200, 0))
                            } else {
                                ("🔴", Color32::from_rgb(200, 0, 0))
                            };
                            ui.metric(
                                &format!("{} Live Prices", icon),
                                &format!("{:.0}% connected", health),
                                color,
                            );
                        }
                    });
                });
            });
    }

    fn render_shortcut_rows(ui: &mut Ui, rows: &[(&str, &str)]) {
        for (key, description) in rows {
            ui.label(RichText::new(*key).monospace().strong());
            ui.label(*description);
            ui.end_row();
        }
    }

    pub(super) fn render_help_panel(&mut self, ctx: &Context) {
        let is_sim_mode = self.is_simulation_mode();

        Window::new("⌨️ Keyboard Shortcuts")
            .open(&mut self.show_debug_help)
            .resizable(false)
            .collapsible(false)
            .default_width(400.0)
            .show(ctx, |ui| {
                ui.heading("Keyboard Shortcuts (Press key to execute commands listed)");
                ui.add_space(10.0);

                // Pre-calculate dynamic strings so they live long enough for the Vec
                let l_sticky = "Toggle ".to_owned() + &UI_TEXT.label_hvz;
                let l_low = "Toggle ".to_owned() + &UI_TEXT.label_lower_wick_zones;
                let l_high = "Toggle ".to_owned() + &UI_TEXT.label_upper_wick_zones;

                // 1. General Shortcuts
                // Use vec![] to create a growable Vector (Arrays [...] are fixed size)
                let mut _general_shortcuts = vec![
                    ("H", "Toggle this help panel"),
                    ("ESC", "Close all open Help Windows"),
                    // ("B", UI_TEXT.label_help_background),
                    ("1", l_sticky.as_str()),
                    ("2", l_low.as_str()),
                    ("3", l_high.as_str()),
                ];

                // Only add 'S' for Native
                #[cfg(not(target_arch = "wasm32"))]
                _general_shortcuts.insert(1, ("S", "Toggle Simulation Mode"));

                Grid::new("general_shortcuts_grid")
                    .num_columns(2)
                    .spacing([20.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        Self::render_shortcut_rows(ui, &_general_shortcuts);
                    });

                if is_sim_mode {
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(5.0);
                    ui.heading("Simulation Mode Controls");
                    ui.add_space(5.0);

                    ui.add_space(5.0);

                    let mut _sim_shortcuts = vec![
                        ("D", UI_TEXT.label_help_sim_toggle_direction),
                        ("X", UI_TEXT.label_help_sim_step_size),
                        ("A", UI_TEXT.label_help_sim_activate_price_change),
                        ("4", UI_TEXT.label_help_sim_jump_hvz),
                        ("5", UI_TEXT.label_help_sim_jump_lower_wicks),
                        ("6", UI_TEXT.label_help_sim_jump_higher_wicks),
                    ];

                    // Only add 'S' for Native
                    #[cfg(not(target_arch = "wasm32"))]
                    _sim_shortcuts.insert(0, ("S", "Enter/Exit Simulation Mode"));

                    Grid::new("sim_shortcuts_grid")
                        .num_columns(2)
                        .spacing([20.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            Self::render_shortcut_rows(ui, &_sim_shortcuts);
                        });
                }

                #[cfg(debug_assertions)]
                {
                    // Note: any keys added here have to be hand-inserted in handle_global_shortcuts, too
                    let debug_shortcuts = [(
                        "INSERT-HERE",
                        "Insert future debug only key-trigger operation here",
                    )];

                    if debug_shortcuts.len() > 1 {
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(5.0);
                        ui.heading("Debug Shortcuts");
                        ui.add_space(5.0);

                        Grid::new("debug_shortcuts_grid")
                            .num_columns(2)
                            .spacing([20.0, 8.0])
                            .striped(true)
                            .show(ui, |ui| {
                                Self::render_shortcut_rows(ui, &debug_shortcuts);
                            });
                    }
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(5.0);
            });
    }

    fn signals_panel(&mut self, ui: &mut Ui) -> Vec<String> {
        // Use the wrapper method we added to App
        let signals = self.get_signals();
        let mut panel = SignalsPanel::new(signals);
        panel.render(ui, &mut false)
    }

    fn data_generation_panel(&mut self, ui: &mut Ui) -> Vec<DataGenerationEventChanged> {
        // 1. Get Data from Engine
        // We clone the Profile to avoid holding an immutable borrow on 'engine'
        // which would prevent us from passing 'self' fields (like auto_duration_config) to the panel.
        let (available_pairs, profile, actual_count) = if let Some(engine) = &self.engine {
            let pairs = engine.get_all_pair_names();

            let (prof, count) = if let Some(pair) = &self.selected_pair {
                (engine.get_profile(pair), engine.get_candle_count(pair))
            } else {
                (None, 0)
            };

            (pairs, prof, count)
        } else {
            (Vec::new(), None, 0)
        };

        // 2. Render Panel
        let mut panel = DataGenerationPanel::new(
            ANALYSIS.zone_count,
            self.selected_pair.clone(),
            available_pairs,
            &self.app_config.price_horizon,
            profile.as_ref(),
            actual_count,
            self.app_config.interval_width_ms,
            self.scroll_to_pair,
        );

        let events = panel.render(ui, &mut self.show_ph_help);

        // We have rendered the frame. If scroll was requested, it happened.
        // Turn it off so the user can scroll manually next frame.
        self.scroll_to_pair = false;

        // Render the window (it handles its own "if open" check internally via .open())
        if self.show_ph_help {
            DataGenerationPanel::render_ph_help_window(ui.ctx(), &mut self.show_ph_help);
        }
        events
    }
}

fn render_fullscreen_message(ui: &mut Ui, title: &str, subtitle: &str, is_error: bool) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);

        if is_error {
            ui.heading(format!("⚠ {}", title));
        } else {
            ui.spinner();
            ui.add_space(12.0);
            ui.heading(title);
        }

        ui.add_space(6.0);

        let text = RichText::new(subtitle).color(if is_error {
            Color32::LIGHT_RED
        } else {
            Color32::from_gray(190)
        });

        ui.label(text);
    });
}
