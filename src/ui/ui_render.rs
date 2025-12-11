use eframe::egui::{
    CentralPanel, Color32, Context, Frame, Grid, Key, Margin, RichText, ScrollArea, SidePanel,
    TopBottomPanel, Ui, Window,
};

use crate::config::ANALYSIS;
use crate::models::cva::ScoreType;
use crate::ui::app_simulation::SimDirection;
use crate::ui::config::{UI_CONFIG, UI_TEXT};
use crate::ui::styles::UiStyleExt;
use crate::ui::ui_panels::{DataGenerationEventChanged, Panel};

use super::app::ZoneSniperApp;
use crate::ui::utils::format_price;

impl ZoneSniperApp {
    pub(super) fn render_side_panel(&mut self, ctx: &Context) {
        let side_panel_frame = Frame::new().fill(UI_CONFIG.colors.side_panel);
        SidePanel::left("left_panel")
            .min_width(140.0)
            .frame(side_panel_frame)
            .show(ctx, |ui| {
                let mut opp_events = Vec::new();

                let data_events = self.data_generation_panel(ui);

                ScrollArea::vertical()
                    .max_height(500.)
                    .id_salt("signal_panel")
                    .show(ui, |ui| {
                        opp_events = self.signals_panel(ui);
                    });
                for pair in opp_events {
                    if Some(&pair) != self.selected_pair.as_ref() {
                        self.selected_pair = Some(pair);
                    }
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
                                self.invalidate_all_pairs_for_global_change(
                                    "price horizon threshold changed",
                                );
                            }
                        }
                        DataGenerationEventChanged::TimeHorizonDays(days) => {
                            if self.app_config.time_horizon.default_days != days {
                                self.app_config.time_horizon.default_days = days.clamp(
                                    ANALYSIS.time_horizon.min_days,
                                    ANALYSIS.time_horizon.max_days,
                                );
                                self.mark_all_journeys_stale("Time Horizon changed");
                            }
                        }
                    }
                }
            });
    }

    pub(super) fn render_central_panel(&mut self, ctx: &eframe::egui::Context) {
        let central_panel_frame = Frame::new().fill(UI_CONFIG.colors.central_panel);

        CentralPanel::default()
            .frame(central_panel_frame)
            .show(ctx, |ui| {
                ui.add_space(10.0);

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

                // Debug log to confirm what the UI is sending to the plot
                if self.is_simulation_mode {
                     log::info!("UI sending price to plot: {:?}", current_price);
                }

                // PRIORITY 1: ERRORS
                // If the most recent calculation failed (e.g. "Insufficient Data" at 1%),
                // we must show the error, even if we have an old cached model.
                // The old model is valid for the OLD settings, not the CURRENT ones.
                if let Some(err_msg) = last_error {
                    render_fullscreen_message(ui, "Analysis Failed", &err_msg, true);
                }
                // PRIORITY 2: VALID MODEL
                // If no error, and we have data, draw it.
                else if let Some(model) = engine.get_model(&pair) {
                    self.plot_view.show_my_plot(
                        ui,
                        &model.cva,
                        &model,
                        current_price,
                        self.debug_background_mode,
                        &self.plot_visibility,
                    );

                    // Optional: Small loading indicator overlay if updating in background
                    if is_calculating {
                        ui.ctx().set_cursor_icon(eframe::egui::CursorIcon::Progress);
                    }
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
        let status_frame = Frame::new()
            .fill(UI_CONFIG.colors.side_panel)
            .inner_margin(Margin::symmetric(8, 4));
        TopBottomPanel::bottom("status_panel")
            .frame(status_frame)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        // 2. Simulation Mode / Live Price Logic
                        if let Some(pair) = &self.selected_pair.clone() {
                            if self.is_simulation_mode {
                                // --- SIMULATION MODE UI ---
                                ui.label(
                                    RichText::new("SIMULATION MODE")
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

                        // 4. Background View Mode
                        ui.label_subdued("Background plot view:");
                        let mode_text = match self.debug_background_mode {
                            ScoreType::FullCandleTVW => UI_TEXT.label_volume,
                            ScoreType::LowWickCount => UI_TEXT.label_lower_wick_count,
                            ScoreType::HighWickCount => UI_TEXT.label_upper_wick_count,
                            _ => "Unknown",
                        };
                        ui.label(
                            RichText::new(mode_text)
                                .small()
                                .color(Color32::from_rgb(0, 255, 255)),
                        );
                        ui.separator();

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

                        // 5. System Status (RESTORED)
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
        Window::new("⌨️ Keyboard Shortcuts")
            .open(&mut self.show_debug_help)
            .resizable(false)
            .collapsible(false)
            .default_width(400.0)
            .show(ctx, |ui| {
                ui.heading("Keyboard Shortcuts");
                ui.add_space(10.0);

                ui.label("Press any key to execute the command:");
                ui.add_space(5.0);

                let general_shortcuts = [
                    ("H", "Toggle this help panel"),
                    ("S", "Toggle Simulation Mode"),
                    ("B", UI_TEXT.label_help_background),
                    ("1", &("Toggle ".to_owned() + &UI_TEXT.label_hvz)),
                    (
                        "2",
                        &("Toggle ".to_owned() + &UI_TEXT.label_lower_wick_zones),
                    ),
                    (
                        "3",
                        &("Toggle ".to_owned() + &UI_TEXT.label_upper_wick_zones),
                    ),
                ];

                Grid::new("general_shortcuts_grid")
                    .num_columns(2)
                    .spacing([20.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        Self::render_shortcut_rows(ui, &general_shortcuts);
                    });

                if self.is_simulation_mode {
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(5.0);
                    ui.heading("Simulation Mode Controls");
                    ui.add_space(5.0);

                    let sim_shortcuts = [
                        ("D", UI_TEXT.label_help_sim_toggle_direction),
                        ("X", UI_TEXT.label_help_sim_step_size),
                        ("A", UI_TEXT.label_help_sim_activate_price_change),
                        ("4", UI_TEXT.label_help_sim_jump_hvz),
                        ("5", UI_TEXT.label_help_sim_jump_lower_wicks),
                        ("6", UI_TEXT.label_help_sim_jump_higher_wicks),
                    ];

                    Grid::new("sim_shortcuts_grid")
                        .num_columns(2)
                        .spacing([20.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            Self::render_shortcut_rows(ui, &sim_shortcuts);
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
        let mut panel = crate::ui::ui_panels::SignalsPanel::new(signals);
        panel.render(ui)
    }

    fn data_generation_panel(
        &mut self,
        ui: &mut eframe::egui::Ui,
    ) -> Vec<crate::ui::ui_panels::DataGenerationEventChanged> {
        // Use Engine or Config for available pairs
        let available_pairs = if let Some(engine) = &self.engine {
            engine.get_all_pair_names()
        } else {
            Vec::new()
        };

        // Pass global constant zone_count from ANALYSIS
        let mut panel = crate::ui::ui_panels::DataGenerationPanel::new(
            ANALYSIS.zone_count,
            self.selected_pair.clone(),
            available_pairs,
            &self.app_config.price_horizon,
            self.app_config.time_horizon.default_days,
        );
        panel.render(ui)
    }

    pub(super) fn handle_global_shortcuts(&mut self, ctx: &Context) {
        ctx.input(|i| {
            // Use 1/2/3 keys to toggle plot visibility
            if i.key_pressed(Key::Num1) {
                self.plot_visibility.sticky = !self.plot_visibility.sticky;
            }
            if i.key_pressed(Key::Num2) {
                self.plot_visibility.low_wicks = !self.plot_visibility.low_wicks;
            }
            if i.key_pressed(Key::Num3) {
                self.plot_visibility.high_wicks = !self.plot_visibility.high_wicks;
            }

            if i.key_pressed(Key::H) {
                self.show_debug_help = !self.show_debug_help;
            }

            if i.key_pressed(Key::Escape) && self.show_debug_help {
                self.show_debug_help = false;
            }

            // 'B'ackground plot type toggle
            if i.key_pressed(Key::B) {
                // Cycle: Sticky -> LowWick -> HighWick -> Sticky
                self.debug_background_mode = match self.debug_background_mode {
                    ScoreType::FullCandleTVW => ScoreType::LowWickCount,
                    ScoreType::LowWickCount => ScoreType::HighWickCount,
                    _ => ScoreType::FullCandleTVW,
                };
            }

            if i.key_pressed(Key::S) {
                self.toggle_simulation_mode();
            }

            if self.is_simulation_mode {
                if i.key_pressed(Key::Num4) {
                    self.jump_to_next_zone("sticky");
                }
                if i.key_pressed(Key::Num5) {
                    self.jump_to_next_zone("low-wick");
                }
                if i.key_pressed(Key::Num6) {
                    self.jump_to_next_zone("high-wick");
                }

                if i.key_pressed(Key::D) {
                    self.sim_direction = match self.sim_direction {
                        SimDirection::Up => SimDirection::Down,
                        SimDirection::Down => SimDirection::Up,
                    };
                    #[cfg(debug_assertions)]
                    log::info!("🔄 Direction: {}", self.sim_direction);
                }

                if i.key_pressed(Key::X) {
                    self.sim_step_size.cycle();
                    #[cfg(debug_assertions)]
                    log::info!("📏 Step size: {}", self.sim_step_size);
                }

                if i.key_pressed(Key::A) {
                    let percent = self.sim_step_size.as_percentage();
                    let adjusted_percent = match self.sim_direction {
                        SimDirection::Up => percent,
                        SimDirection::Down => -percent,
                    };
                    self.adjust_simulated_price_by_percent(adjusted_percent);
                }
            }
        });
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
