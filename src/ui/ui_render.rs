use eframe::egui::{self, ScrollArea};
use std::time::Duration;

use crate::config::ANALYSIS;
use crate::data::price_stream::PriceStreamManager;
use crate::models::cva::ScoreType;
use crate::ui::app_simulation::SimDirection;
use crate::ui::config::UI_CONFIG;
use crate::ui::ui_panels::{DataGenerationEventChanged, DataGenerationPanel, Panel, SignalsPanel};

use crate::ui::config::UI_TEXT;

use super::app::ZoneSniperApp;

#[cfg(debug_assertions)]
use crate::config::DEBUG_FLAGS;

impl ZoneSniperApp {
    pub(super) fn render_side_panel(&mut self, ctx: &egui::Context) {
        let side_panel_frame = egui::Frame::new().fill(UI_CONFIG.colors.side_panel);
        egui::SidePanel::left("left_panel")
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
                        self.schedule_selected_pair_recalc("selected from signals panel");
                    }
                }

                for event in data_events {
                    match event {
                        DataGenerationEventChanged::ZoneCount(new_count) => {
                            self.zone_count = new_count;
                            self.schedule_selected_pair_recalc("zone count changed");
                        }
                        DataGenerationEventChanged::Pair(new_pair) => {
                            self.handle_pair_selection(new_pair);
                        }
                        DataGenerationEventChanged::AutoDurationThreshold(new_threshold) => {
                            let prev = self.auto_duration_config.relevancy_threshold;
                            if (prev - new_threshold).abs() > f64::EPSILON {
                                self.auto_duration_config.relevancy_threshold = new_threshold;
                                self.invalidate_all_pairs_for_global_change(
                                    "auto-duration threshold changed",
                                );
                            }
                        }
                        DataGenerationEventChanged::TimeHorizonDays(days) => {
                            if self.time_horizon_days != days {
                                self.time_horizon_days = days.clamp(
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

    pub(super) fn render_central_panel(&mut self, ctx: &egui::Context) {
        let central_panel_frame = egui::Frame::new().fill(UI_CONFIG.colors.central_panel);
        egui::CentralPanel::default()
            .frame(central_panel_frame)
            .show(ctx, |ui| {
                ui.add_space(10.0);

                if self.price_stream.is_none() {
                    let stream = PriceStreamManager::new();
                    let all_pairs = self.data_state.timeseries_collection.unique_pair_names();
                    stream.subscribe_all(all_pairs);
                    self.price_stream = Some(stream);
                }

                if !self.monitor_initialized {
                    self.initialize_multi_pair_monitor();
                }

                if self.price_stream.is_some() {
                    let all_pairs = self.data_state.timeseries_collection.unique_pair_names();

                    let selected_pair = self.selected_pair.clone();
                    let selected_waiting_for_price = selected_pair
                        .as_ref()
                        .map(|pair| self.get_display_price(pair).is_none())
                        .unwrap_or(false);

                    for pair in &all_pairs {
                        if selected_waiting_for_price {
                            if let Some(sel) = self.selected_pair.as_ref() {
                                if sel != pair {
                                    continue;
                                }
                            }
                        }

                        if let Some(new_price) = self.get_display_price(pair) {
                            if let Some(trigger) = self.pair_triggers.get_mut(pair) {
                                if trigger.consider_price_move(new_price)
                                    && trigger.ready_to_schedule()
                                {
                                    trigger.pending_price = Some(new_price);
                                }
                            }
                            if let Some(selected) = self.selected_pair.as_ref() {
                                if selected == pair {
                                    let ready = self
                                        .pair_triggers
                                        .get(pair)
                                        .map(|trigger| trigger.ready_to_schedule())
                                        .unwrap_or(false);

                                    if ready && !self.is_calculating() {
                                        self.enqueue_recalc_for_pair(pair.clone());
                                    }
                                }
                            }
                        }
                    }

                    if let Some(pair) = self.selected_pair.clone() {
                        let new_price = self.get_display_price(&pair);

                        if let Some(price) = new_price {
                            if let Some(trigger) = self.pair_triggers.get_mut(&pair) {
                                if trigger.consider_price_move(price) && trigger.ready_to_schedule()
                                {
                                    trigger.pending_price = Some(price);
                                }
                            }
                            self.current_pair_price = Some(price);
                        }
                    }

                    ctx.request_repaint_after(Duration::from_secs(1));
                }

                if let Some(cva_results) = &self.data_state.cva_results {
                    self.plot_view.show_my_plot(
                        ui,
                        cva_results,
                        self.current_pair_price,
                        self.debug_background_mode,
                    );
                } else if !self.is_calculating() {
                    if let Some(error) = &self.data_state.last_error {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.heading("⚠ Unable to Generate Results");
                            ui.add_space(10.0);
                            ui.label(format!("Error: {}", error));
                            ui.add_space(20.0);
                            ui.label("Please check your pair selection and try again.");
                        });
                    } else if self.current_pair_price.is_none() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.spinner();
                            ui.add_space(12.0);
                            ui.heading("Preparing live data...");
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new("Connecting to the price feed")
                                    .color(egui::Color32::from_gray(190)),
                            );
                        });
                    } else {
                        let pair_name = self.selected_pair.clone();
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.spinner();
                            ui.add_space(12.0);
                            if let Some(pair) = pair_name {
                                ui.heading(format!("Preparing analysis for {}...", pair));
                            } else {
                                ui.heading("Preparing analysis...");
                            }
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new("Rebuilding zones with the latest settings")
                                    .color(egui::Color32::from_gray(190)),
                            );
                        });
                    }
                }
            });
    }

    pub(super) fn render_status_panel(&mut self, ctx: &egui::Context) {
        let status_frame = egui::Frame::new()
            .fill(UI_CONFIG.colors.side_panel)
            .inner_margin(egui::Margin::symmetric(8, 4));
        egui::TopBottomPanel::bottom("status_panel")
            .frame(status_frame)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if self.is_simulation_mode {
                            ui.label(
                                egui::RichText::new("🎮 SIMULATION MODE")
                                    .strong()
                                    .color(egui::Color32::from_rgb(255, 150, 0)),
                            );
                            ui.separator();

                            ui.label(
                                egui::RichText::new(format!("{}", self.sim_direction))
                                    .small()
                                    .color(egui::Color32::from_rgb(200, 200, 255)),
                            );
                            ui.label(
                                egui::RichText::new(format!("| Step: {}", self.sim_step_size))
                                    .small()
                                    .color(egui::Color32::from_rgb(200, 200, 255)),
                            );
                            ui.separator();
                        } else {
                            ui.label(
                                egui::RichText::new("📡 LIVE MODE")
                                    .small()
                                    .color(egui::Color32::from_rgb(100, 200, 100)),
                            );
                            ui.separator();
                        }

                        if let Some(ref pair) = self.selected_pair {
                            if self.is_simulation_mode {
                                if let Some(sim_price) = self.simulated_prices.get(pair) {
                                    ui.label(
                                        egui::RichText::new(format!("💰 ${:.2}", sim_price))
                                            .strong()
                                            .color(egui::Color32::from_rgb(255, 200, 100)),
                                    );
                                    if let Some(live_price) = self
                                        .price_stream
                                        .as_ref()
                                        .and_then(|stream| stream.get_price(pair))
                                    {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "(live: ${:.2})",
                                                live_price
                                            ))
                                            .small()
                                            .color(egui::Color32::GRAY),
                                        );
                                    }
                                }
                            } else if let Some(price) = self.current_pair_price {
                                ui.label(
                                    egui::RichText::new(format!("💰 ${:.2}", price))
                                        .color(egui::Color32::from_rgb(100, 200, 255)),
                                );
                            }
                            ui.separator();
                        }

                        if let Some(ref cva_results) = self.data_state.cva_results {
                            let zone_size = (cva_results.price_range.end_range
                                - cva_results.price_range.start_range)
                                / cva_results.zone_count as f64;

                            ui.label(
                                egui::RichText::new(format!(
                                    "📏 Zone Size: ${:.2} (N={})",
                                    zone_size, cva_results.zone_count
                                ))
                                .small()
                                .color(egui::Color32::from_rgb(180, 200, 255)), // Light blue for visibility
                            );

                            ui.separator();
                        }

                        ui.separator();
                        ui.label(
                            egui::RichText::new("View:")
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                        let mode_text = match self.debug_background_mode {
                            ScoreType::FullCandleTVW => UI_TEXT.label_hvz_within,
                            ScoreType::LowWickCount => UI_TEXT.label_hvz_beneath,
                            ScoreType::HighWickCount => UI_TEXT.label_hvz_above,
                            _ => "Unknown",
                        };

                        ui.label(
                            egui::RichText::new(mode_text)
                                .small()
                                .color(egui::Color32::from_rgb(0, 255, 255)), // Cyan for visibility
                        );
                        ui.separator();

                        if let Some(cva) = &self.data_state.cva_results {
                            // We regenerate the model briefly to get the stats.
                            // This is fast for 200 zones.
                            let model = crate::models::trading_view::TradingModel::from_cva(
                                std::sync::Arc::clone(cva),
                                self.current_pair_price,
                            );

                            // Helper to color-code coverage
                            // > 30% is Red (Too much), < 5% is Yellow (Too little?), Green is good
                            let coverage_color = |pct: f64| {
                                if pct > 30.0 {
                                    egui::Color32::from_rgb(255, 100, 100)
                                }
                                // Red warning
                                else {
                                    egui::Color32::from_rgb(150, 255, 150)
                                } // Green ok
                            };

                            ui.label(
                                egui::RichText::new("Coverage:")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );

                            ui.label(
                                egui::RichText::new(format!("Sticky:{:.0}%", model.coverage.sticky_pct))
                                    .small()
                                    .color(coverage_color(model.coverage.sticky_pct)),
                            );

                            ui.label(
                                egui::RichText::new(format!(
                                    "R-Sup:{:.0}%",
                                    model.coverage.support_pct
                                ))
                                .small()
                                .color(coverage_color(model.coverage.support_pct)),
                            );

                            ui.label(
                                egui::RichText::new(format!(
                                    "R-Res:{:.0}%",
                                    model.coverage.resistance_pct
                                ))
                                .small()
                                .color(coverage_color(model.coverage.resistance_pct)),
                            );

                            ui.separator();
                        }
                        let pair_count = self
                            .data_state
                            .timeseries_collection
                            .unique_pair_names()
                            .len();
                        ui.label(
                            egui::RichText::new(format!("📊 {} pairs loaded", pair_count))
                                .small()
                                .color(egui::Color32::GRAY),
                        );

                        if let Some(summary) = self.model_status_summary() {
                            ui.separator();
                            ui.label(
                                egui::RichText::new(summary)
                                    .small()
                                    .color(egui::Color32::from_rgb(255, 165, 0)),
                            );
                        }

                        #[cfg(debug_assertions)]
                        {
                            let horizon_heading = UI_TEXT.price_horizon_heading;
                            let horizon_text = if let Some(ranges) =
                                self.computed_slice_indices.as_ref()
                            {
                                let total_candles: usize = ranges.iter().map(|(s, e)| e - s).sum();
                                let total_ms =
                                    total_candles as f64 * ANALYSIS.interval_width_ms as f64;
                                let days = total_ms / (1000.0 * 60.0 * 60.0 * 24.0);
                                format!(
                                    "🕒 {}: {} candles ({:.1}d)",
                                    horizon_heading, total_candles, days
                                )
                            } else {
                                format!("🕒 {}: calculating…", horizon_heading)
                            };
                            ui.separator();
                            ui.label(
                                egui::RichText::new(horizon_text)
                                    .small()
                                    .color(egui::Color32::from_rgb(150, 200, 255)),
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "🧮 Decay: {:.3}",
                                    self.time_decay_factor
                                ))
                                .small()
                                .color(egui::Color32::from_rgb(180, 200, 255)),
                            );
                        }

                        ui.separator();

                        if let Some(ref stream) = self.price_stream {
                            let health = stream.connection_health();
                            let (icon, color) = if health >= 90.0 {
                                ("🟢", egui::Color32::from_rgb(0, 200, 0))
                            } else if health >= 50.0 {
                                ("🟡", egui::Color32::from_rgb(200, 200, 0))
                            } else {
                                ("🔴", egui::Color32::from_rgb(200, 0, 0))
                            };

                            ui.label(
                                egui::RichText::new(format!(
                                    "{} Live Prices: {:.0}% connected",
                                    icon, health
                                ))
                                .small()
                                .color(color),
                            );
                        }
                    });

                    #[cfg(debug_assertions)]
                    {
                        self.render_journey_debug_info(ui);
                    }
                });
            });
    }

    #[cfg(debug_assertions)]
    fn render_journey_debug_info(&self, ui: &mut egui::Ui) {
        // Only run if DISPLAY_JOURNEY_STATUS_LINES flag is enabled
        if !DEBUG_FLAGS.display_journey_status_lines {
            return;
        }

        ui.add_space(6.0);
        let status_lines = self.model_status_lines();
        let (current_color, current_line) = self.journey_status_line();
        let zone_lines = self.current_journey_zone_lines();
        let (aggregate_color, aggregate_line) = self.journey_aggregate_line();

        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(UI_TEXT.journey_status_heading)
                    .small()
                    .strong()
                    .color(egui::Color32::from_rgb(220, 220, 200)),
            );

            for line in status_lines {
                ui.label(
                    egui::RichText::new(line)
                        .small()
                        .italics()
                        .color(egui::Color32::from_rgb(180, 180, 200)),
                );
            }

            ui.separator();
            ui.label(
                egui::RichText::new(current_line)
                    .small()
                    .color(current_color),
            );

            for (color, line) in zone_lines {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(line).small().color(color));
                });
            }

            ui.separator();
            ui.label(
                egui::RichText::new(aggregate_line)
                    .small()
                    .color(aggregate_color),
            );
        });
    }

    fn render_shortcut_rows(ui: &mut egui::Ui, rows: &[(&str, &str)]) {
        for (key, description) in rows {
            ui.label(egui::RichText::new(*key).monospace().strong());
            ui.label(*description);
            ui.end_row();
        }
    }

    pub(super) fn render_help_panel(&mut self, ctx: &egui::Context) {
        egui::Window::new("⌨️ Keyboard Shortcuts")
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
                    ("B", "Toggle Background Data"),
                ];

                egui::Grid::new("general_shortcuts_grid")
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
                        ("D", "Toggle direction (▲ UP / ▼ DOWN)"),
                        ("X", "Cycle step size (0.1% → 1% → 5% → 10%)"),
                        ("A", "Activate price change in current direction"),
                        ("1", "Jump to next sticky zone"),
                        ("2", "Jump to next slippy zone"),
                        ("3", "Jump to next low wick zone"),
                        ("4", "Jump to next high wick zone"),
                    ];

                    egui::Grid::new("sim_shortcuts_grid")
                        .num_columns(2)
                        .spacing([20.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            Self::render_shortcut_rows(ui, &sim_shortcuts);
                        });
                }

                #[cfg(debug_assertions)]
                {
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(5.0);
                    ui.heading("Debug Shortcuts");
                    ui.add_space(5.0);

                    // Note: any keys added here have to be hand-inserted in handle_global_shortcuts, too
                    let debug_shortcuts = [(
                        "Cuts",
                        "Insert future debug only key-trigger operation here",
                    )];

                    egui::Grid::new("debug_shortcuts_grid")
                        .num_columns(2)
                        .spacing([20.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            Self::render_shortcut_rows(ui, &debug_shortcuts);
                        });
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(5.0);
            });
    }

    fn signals_panel(&mut self, ui: &mut egui::Ui) -> Vec<String> {
        let signals = self.multi_pair_monitor.get_signals();
        let mut panel = SignalsPanel::new(signals);
        panel.render(ui)
    }

    fn data_generation_panel(&mut self, ui: &mut egui::Ui) -> Vec<DataGenerationEventChanged> {
        let available_pairs = self.data_state.timeseries_collection.unique_pair_names();
        let mut panel = DataGenerationPanel::new(
            self.zone_count,
            self.selected_pair.clone(),
            available_pairs,
            &self.auto_duration_config,
            self.time_horizon_days,
        );
        panel.render(ui)
    }

    pub(super) fn handle_global_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.key_pressed(egui::Key::H) {
                self.show_debug_help = !self.show_debug_help;
            }

            if i.key_pressed(egui::Key::Escape) && self.show_debug_help {
                self.show_debug_help = false;
            }

            // 'B'ackground plot type toggle
            if i.key_pressed(egui::Key::B) {
                // Cycle: Sticky -> LowWick -> HighWick -> Sticky
                self.debug_background_mode = match self.debug_background_mode {
                    ScoreType::FullCandleTVW => ScoreType::LowWickCount,
                    ScoreType::LowWickCount => ScoreType::HighWickCount,
                    _ => ScoreType::FullCandleTVW,
                };
            }

            if i.key_pressed(egui::Key::S) {
                self.toggle_simulation_mode();
            }

            if self.is_simulation_mode {
                if i.key_pressed(egui::Key::D) {
                    self.sim_direction = match self.sim_direction {
                        SimDirection::Up => SimDirection::Down,
                        SimDirection::Down => SimDirection::Up,
                    };
                    #[cfg(debug_assertions)]
                    log::info!("🔄 Direction: {}", self.sim_direction);
                }

                if i.key_pressed(egui::Key::X) {
                    self.sim_step_size.cycle();
                    #[cfg(debug_assertions)]
                    log::info!("📏 Step size: {}", self.sim_step_size);
                }

                if i.key_pressed(egui::Key::A) {
                    let percent = self.sim_step_size.as_percentage();
                    let adjusted_percent = match self.sim_direction {
                        SimDirection::Up => percent,
                        SimDirection::Down => -percent,
                    };
                    self.adjust_simulated_price_by_percent(adjusted_percent);
                }

                if i.key_pressed(egui::Key::Num1) {
                    self.jump_to_next_zone("sticky");
                }
                if i.key_pressed(egui::Key::Num2) {
                    self.jump_to_next_zone("slippy");
                }
                if i.key_pressed(egui::Key::Num3) {
                    self.jump_to_next_zone("low-wick");
                }
                if i.key_pressed(egui::Key::Num4) {
                    self.jump_to_next_zone("high-wick");
                }
            }
        });
    }
}
