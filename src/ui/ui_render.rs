use eframe::egui::{ScrollArea, Color32, RichText, Ui, Context, Grid, Key, SidePanel, TopBottomPanel, Margin, CentralPanel, Window, Frame};
use std::sync::Arc;
use std::time::Duration;

use crate::config::ANALYSIS;
use crate::data::price_stream::PriceStreamManager;
use crate::models::cva::ScoreType;
use crate::models::trading_view::TradingModel;
use crate::ui::app_simulation::SimDirection;
use crate::ui::ui_panels::{DataGenerationEventChanged, DataGenerationPanel, Panel, SignalsPanel};

use crate::ui::config::{UI_CONFIG, UI_TEXT};
use crate::ui::styles::UiStyleExt;

use super::app::ZoneSniperApp;

#[cfg(debug_assertions)]
use crate::config::DEBUG_FLAGS;

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

    pub(super) fn render_central_panel(&mut self, ctx: &Context) {
        let central_panel_frame = Frame::new().fill(UI_CONFIG.colors.central_panel);
        CentralPanel::default()
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
                        &self.plot_visibility,
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
                                RichText::new("Connecting to the price feed")
                                    .color(Color32::from_gray(190)),
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
                                RichText::new("Rebuilding zones with the latest settings")
                                    .color(Color32::from_gray(190)),
                            );
                        });
                    }
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
                        // 1. Simulation Mode
                        if self.is_simulation_mode {
                            ui.label_warning("🎮 SIMULATION MODE");
                            ui.separator();
                            ui.label_subdued(format!("{}", self.sim_direction));
                            ui.label_subdued(format!("| Step: {}", self.sim_step_size));
                            ui.separator();
                        } else {
                            ui.metric("📡", "LIVE MODE", Color32::from_rgb(100, 200, 100));
                            ui.separator();
                        }

                         // 2. Price Display
                        if let Some(ref pair) = self.selected_pair {
                            if self.is_simulation_mode {
                                if let Some(sim_price) = self.simulated_prices.get(pair) {
                                    ui.metric("💰", &format!("${:.2}", sim_price), Color32::from_rgb(255, 200, 100));
                                    
                                    if let Some(live_price) = self.price_stream.as_ref().and_then(|s| s.get_price(pair)) {
                                        ui.label_subdued(format!("(live: ${:.2})", live_price));
                                    }
                                }
                            } else if let Some(price) = self.current_pair_price {
                                ui.metric("💰", &format!("${:.2}", price), Color32::from_rgb(100, 200, 255));
                            }
                            ui.separator();
                        }

                        // 3. Zone Size
                        if let Some(ref cva_results) = self.data_state.cva_results {
                            let zone_size = (cva_results.price_range.end_range
                                - cva_results.price_range.start_range)
                                / cva_results.zone_count as f64;
                            
                            ui.metric("📏 Zone Size", &format!("${:.2} (N={})", zone_size, cva_results.zone_count), Color32::from_rgb(180, 200, 255));
                            ui.separator();
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
                        ui.label(RichText::new(mode_text).small().color(Color32::from_rgb(0, 255, 255)));
                        ui.separator();

                        ui.label(
                            RichText::new(mode_text)
                                .small()
                                .color(Color32::from_rgb(0, 255, 255)), // Cyan for visibility
                        );
                        ui.separator();

                        // Coverage Statistics
                        if let Some(cva) = &self.data_state.cva_results {
                            // We regenerate the model briefly to get the stats.
                            // This is fast for 200 zones.
                            let model =
                                TradingModel::from_cva(Arc::clone(cva), self.current_pair_price);

                            // Helper to color-code coverage
                            // > 30% is Red (Too much), < 5% is Yellow (Too little?), Green is good
                            let cov_color = |pct: f64| {
                                if pct > 30.0 {
                                    Color32::from_rgb(255, 100, 100)
                                }
                                // Red warning
                                else {
                                    Color32::from_rgb(150, 255, 150)
                                } // Green ok
                            };

                            // NEW STYLE: Using the Trait
                            ui.label_subdued("Coverage");

                            ui.metric("Sticky", &format!("{:.0}%", model.coverage.sticky_pct), 
                                cov_color(model.coverage.sticky_pct));
                            
                            ui.metric("R-Sup", &format!("{:.0}%", model.coverage.support_pct), 
                                cov_color(model.coverage.support_pct));

                            ui.metric("R-Res", &format!("{:.0}%", model.coverage.resistance_pct), 
                                cov_color(model.coverage.resistance_pct));
                                
                            ui.separator();
                        }

                        // 6. System & Model Status
                        let pair_count = self.data_state.timeseries_collection.unique_pair_names().len();
                        ui.label_subdued(format!("📊 {} pairs", pair_count));

                        // NEW: Data-driven status
                        let status = self.model_status_summary();
                        if status.is_calculating {
                            ui.separator();
                            ui.label_warning("⚙ Updating CVA...");
                        }
                        if status.pairs_queued > 0 {
                            ui.separator();
                            ui.label_warning(format!("Journeys: {} queued", status.pairs_queued));
                        }

                        // 7. Debug Horizon Info
                        #[cfg(debug_assertions)]
                        {
                            ui.separator();
                            let heading = UI_TEXT.price_horizon_heading;
                            if let Some(ranges) = self.computed_slice_indices.as_ref() {
                                let total: usize = ranges.iter().map(|(s, e)| e - s).sum();
                                let ms = total as f64 * ANALYSIS.interval_width_ms as f64;
                                let days = ms / (1000.0 * 60.0 * 60.0 * 24.0);
                                
                                // Refactored to use metric style
                                ui.metric(&format!("🕒 {}", heading), &format!("{} candles ({:.1}d)", total, days), Color32::from_rgb(150, 200, 255));
                            } else {
                                ui.label_subdued(format!("🕒 {}: calculating…", heading));
                            }
                            
                            ui.metric("🧮 Decay", &format!("{:.3}", self.time_decay_factor), Color32::from_rgb(180, 200, 255));
                        }
                        ui.separator();

                        // 8. Network health
                        if let Some(ref stream) = self.price_stream {
                            let health = stream.connection_health();
                            let (icon, color) = if health >= 90.0 {
                                ("🟢", Color32::from_rgb(0, 200, 0))
                            } else if health >= 50.0 {
                                ("🟡", Color32::from_rgb(200, 200, 0))
                            } else {
                                ("🔴", Color32::from_rgb(200, 0, 0))
                            };
                            ui.metric(&format!("{} Live Prices", icon), &format!("{:.0}% connected", health), color);
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
    fn render_journey_debug_info(&self, ui: &mut Ui) {
        if !DEBUG_FLAGS.display_journey_status_lines {
            return;
        }

        ui.add_space(6.0);
        let status_lines = self.model_status_lines();
        let (current_color, current_line) = self.journey_status_line();
        let zone_lines = self.current_journey_zone_lines();
        let (aggregate_color, aggregate_line) = self.journey_aggregate_line();

        ui.vertical(|ui| {
            // Header
            ui.label_subdued(UI_TEXT.journey_status_heading.to_uppercase());

            // Status Lines (Gray/Italic concept -> Subdued)
            for line in status_lines {
                ui.label_subdued(line);
            }

            ui.separator();
            
            // Current Line (Dynamic Color)
            ui.label(RichText::new(current_line).small().color(current_color));

            // Zone Lines (Dynamic Colors)
            for (color, line) in zone_lines {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(RichText::new(line).small().color(color));
                });
            }

            ui.separator();
            
            // Aggregate Line (Dynamic Color)
            ui.label(RichText::new(aggregate_line).small().color(aggregate_color));
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
        let signals = self.multi_pair_monitor.get_signals();
        let mut panel = SignalsPanel::new(signals);
        panel.render(ui)
    }

    fn data_generation_panel(&mut self, ui: &mut Ui) -> Vec<DataGenerationEventChanged> {
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
