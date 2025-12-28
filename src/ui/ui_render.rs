use eframe::egui::{
    CentralPanel, Context, Grid, RichText, ScrollArea, SidePanel, TopBottomPanel,
    Ui, Window, Order,
};

use crate::analysis::adaptive::AdaptiveParameters;
use crate::config::{ANALYSIS};
use crate::config::TICKER;
use crate::models::cva::ScoreType;

use crate::ui::app::CandleResolution;
use crate::ui::config::{UI_CONFIG, UI_TEXT};
use crate::ui::styles::{UiStyleExt, get_outcome_color, DirectionColor};

use crate::config::plot::PLOT_CONFIG;

use crate::ui::ui_panels::{
    CandleRangePanel, DataGenerationEventChanged, DataGenerationPanel, Panel, SignalsPanel,
};
use crate::ui::ui_plot_view::PlotInteraction;

use crate::ui::utils::format_price;

use crate::utils::TimeUtils;

use super::app::ZoneSniperApp;

impl ZoneSniperApp {

        pub(super) fn render_opportunity_details_modal(&mut self, ctx: &Context) {
        // 1. Check if open
        if !self.show_opportunity_details { return; }

        // 2. Get Data (Thread-safe)
        let Some(pair) = self.selected_pair.clone() else { return; };
        let Some(model) = self.engine.as_ref().and_then(|e| e.get_model(&pair)) else { return; };
        // let current_price = self.get_display_price(&pair).unwrap_or(0.0);

        // 3. Find the "Current Opportunity" (Same logic as HUD)
        let best_opp = model.opportunities.iter()
            .filter(|op| op.expected_roi() > 0.0)
            .max_by(|a, b| a.expected_roi().partial_cmp(&b.expected_roi()).unwrap());

        // 4. Render Window
        Window::new(format!("Opportunity Explainer: {}", pair))
            .collapsible(false)
            .resizable(false)
            .order(Order::Tooltip)
            .open(&mut self.show_opportunity_details)
            .default_width(600.)
            .show(ctx, |ui| {
                if let Some(op) = best_opp {

                    let sim = &op.simulation;
                    // --- LOOKUP TARGET ZONE FOR TITLE ---
                    // Try to find the zone definition to get its bounds
                    let zone_info = model.zones.sticky_superzones.iter()
                        .find(|z| z.id == op.target_zone_id)
                        .map(|z| format!("{} - {}", format_price(z.price_bottom), format_price(z.price_top)))
                        .unwrap_or_else(|| format!("Zone #{}", op.target_zone_id)); // Fallback

                    ui.heading(format!("Best Opportunity: {}", zone_info));
                    // "Setup Type: LONG" (with encoded color)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label_subdued("Setup Type:");
                        ui.label(RichText::new(op.direction.to_string().to_uppercase())
                            .strong()
                            .color(op.direction.color())); // Uses PLOT_CONFIG via trait
                    });
                    ui.separator();


                    // --- CALCULATIONS ---
                    // 1. Get PH %
                    let ph_pct = self.app_config.price_horizon.threshold_pct;
                    
                    // 2. Calculate Actual Lookback used (using new Adaptive logic)
                    let lookback_candles = AdaptiveParameters::calculate_trend_lookback_candles(ph_pct);
                    let interval_ms = model.cva.interval_ms; 
                    let lookback_ms = lookback_candles as i64 * interval_ms;
                    let lookback_str = TimeUtils::format_duration(lookback_ms);

                    // 3. Get Actual Max Journey Time (From Config, NOT derived)
                    let max_time = self.app_config.journey.max_journey_time;
                    let max_time_str = TimeUtils::format_duration(max_time.as_millis() as i64);


                    // SECTION 1: THE MATH
                    ui.label_subheader("Expectancy & Return");
                    let roi = op.expected_roi();
                    let roi_color = get_outcome_color(roi);
                    
                    ui.metric("RoI (per trade)", &format!("{:+.2}%", roi), roi_color);
                    ui.metric("Win Rate", &format!("{:.1}%", sim.success_rate * 100.0), PLOT_CONFIG.color_text_primary);
                    ui.metric("R:R Ratio", &format!("{:.2}", sim.risk_reward_ratio), PLOT_CONFIG.color_text_primary);

                    ui.add_space(10.0);

                    // SECTION 2: MARKET CONTEXT (INLINE STYLE)
                    ui.label_subheader("Market Context (The 'DNA')");
                    let state = &sim.market_state;
                    
                    // Volatility (Standard metric is fine)
                    ui.metric("Volatility", &format!("{:.2}%", state.volatility_pct * 100.0), PLOT_CONFIG.color_info);
                    
                    // Momentum (Inline Explanation)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(RichText::new("Momentum:").small().color(PLOT_CONFIG.color_text_subdued));
                        ui.label(RichText::new(format!("{:+.2}%", state.momentum_pct * 100.0)).small().color(get_outcome_color(state.momentum_pct)));
                        
                        ui.label(RichText::new(format!(
                            "(Price change over the last {}. Adaptive lookback based on Price Horizon {:.3}%)", 
                            lookback_str, ph_pct * 100.0
                        )).small().color(PLOT_CONFIG.color_text_subdued));
                    });

                    // Rel Volume (Inline Explanation)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(RichText::new("Rel. Volume:").small().color(PLOT_CONFIG.color_text_subdued));
                        let vol_color = if state.relative_volume > 1.0 { 
                            PLOT_CONFIG.color_warning 
                        } else { 
                            PLOT_CONFIG.color_text_subdued 
                        };
                        ui.label(RichText::new(format!("{:.2}x", state.relative_volume)).small().color(vol_color));
                        ui.label(RichText::new("(Ratio of Current Volume vs Recent Average.)").small().color(PLOT_CONFIG.color_text_subdued));
                    });

                    ui.add_space(10.0);

                  // SECTION 3: TRADE SETUP
                    ui.label_subheader("Trade Setup");
                    
                    // Entry / Target can stay standard
                    ui.metric("Entry", &format_price(op.start_price), PLOT_CONFIG.color_text_neutral);
                    ui.metric("Target", &format_price(op.target_price), PLOT_CONFIG.color_profit);
                    
                    // Stop Loss (Inline Explanation)
                    let target_dist = (op.target_price - op.start_price).abs() / op.start_price * 100.0;
                    let stop_dist = (op.stop_price - op.start_price).abs() / op.start_price * 100.0;

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(RichText::new("Stop Loss:").small().color(PLOT_CONFIG.color_text_subdued));
                        ui.label(RichText::new(format_price(op.stop_price)).small().color(PLOT_CONFIG.color_stop_loss));
                        
                        ui.label(RichText::new(format!(
                            "(Target {:.2}% / Stop {:.2}%)", 
                            target_dist, stop_dist
                        )).small().color(PLOT_CONFIG.color_text_subdued));
                    });

                    ui.add_space(15.0);
                    ui.separator();
                    ui.add_space(5.0);

                    // SECTION 4: THE STORY
                    ui.label_subheader("How this works");
                    ui.vertical(|ui| {
                        ui.style_mut().spacing.item_spacing.y = 4.0;
                        let story_color = PLOT_CONFIG.color_text_neutral;
                        
                        ui.label(RichText::new(format!(
                            "1. We fingerprinted the market right now (Vol = {:.2}%, Momentum = {:+.2}%, Rel.Vol = {:.2}x).",
                            state.volatility_pct * 100.0,
                            state.momentum_pct * 100.0,
                            state.relative_volume
                        )).small().color(story_color).italics());
                        
                        let match_text = if sim.sample_size < 50 {
                            format!("2. We scanned history and found exactly {} periods that matched this fingerprint.", sim.sample_size)
                        } else {
                            format!("2. We scanned history and found many matches, but we kept only the Top {} closest matches.", sim.sample_size)
                        };
                        ui.label(RichText::new(match_text).small().color(story_color).italics());

                        ui.label(RichText::new(format!(
                            "3. We simulated these {} scenarios. We checked if price hit the Target, the Stop, or ran out of time (Limit: {}).",
                            sim.sample_size,
                            max_time_str
                        )).small().color(story_color).italics());
                        
                        let win_count = (sim.success_rate * sim.sample_size as f64).round() as usize;
                        let win_pct = sim.success_rate * 100.0;
                        
                        // FIXED: Specific Phrasing
                        ui.label(RichText::new(format!(
                            "4. In {} of those {} cases, price hit the Target first. This produces the {:.1}% Win Rate you see above.",
                            win_count, sim.sample_size, win_pct
                        )).small().color(story_color).italics());
                    });

                } else {
                    ui.label("No valid opportunities found. Check your settings or try a different pair.");
                }
            });
    }


    pub(super) fn render_right_panel(&mut self, ctx: &Context) {
        let frame = UI_CONFIG.side_panel_frame();

        SidePanel::right("right_panel")
            .min_width(160.0)
            .resizable(false)
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
                            let mut panel =
                                CandleRangePanel::new(&model.segments, nav.current_segment_idx);

                            if let Some(new_idx) = panel.render(ui, safe_last) {
                                nav.current_segment_idx = new_idx;
                                // If we switched to a specific segment (not "Show All"), remember it.
                                if let Some(idx) = new_idx {
                                    nav.last_viewed_segment_idx = idx;
                                }
                                // Write back to app state
                                self.set_nav_state(nav);

                                self.auto_scale_y = true;
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
                    self.handle_pair_selection(pair_name.clone());
                    self.scroll_to_pair = Some(pair_name);
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
                                self.global_price_horizon.threshold_pct = new_threshold;

                                // --- ADAPTIVE DECAY LOGIC ---
                                // "Drag left to Snipe (High Decay), Drag right to Invest (Low Decay)"
                                let new_decay = AdaptiveParameters::calculate_time_decay(new_threshold);

                                // Apply only if changed (prevents log spam if dragging within same band)
                                if (self.app_config.time_decay_factor - new_decay).abs()
                                    > f64::EPSILON
                                {
                                    self.app_config.time_decay_factor = new_decay;
                                }
                                self.auto_scale_y = true;
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

        TopBottomPanel::top("top_toolbar")
            .frame(frame)
            .min_height(30.0)
            .resizable(false)
            .show(ctx, |ui| {
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
                    ui.checkbox(&mut self.plot_visibility.separators, "TM Separators"); // Toggle segment separators
                    ui.checkbox(&mut self.plot_visibility.horizon_lines, "PH Lines"); // Toggle dashed horizontal PH border lines
                    ui.checkbox(&mut self.plot_visibility.price_line, "Price");
                    ui.checkbox(&mut self.plot_visibility.opportunities, "Targets");

                    // STATUS INDICATOR (TEMP but very useful)
                    if self.auto_scale_y {
                        ui.label(RichText::new("🔒 Y-LOCKED").small().color(PLOT_CONFIG.color_profit));
                    } else {
                        ui.label(RichText::new("🔓 MANUAL Y").small().color(PLOT_CONFIG.color_warning));
                    }
                });
            });
    }

    pub(super) fn render_ticker_panel(&mut self, ctx: &Context) {
        let panel_frame = UI_CONFIG.bottom_panel_frame();

        // Render at bottom. If called BEFORE status panel in update(), it sits below it.
        // If called AFTER, it sits above it.
        TopBottomPanel::bottom("ticker_panel")
            .frame(panel_frame)
            .min_height(TICKER.height)
            .resizable(false)
            .show(ctx, |ui| {
                if let Some(engine) = &self.engine {
                    self.ticker_state.update_data(engine);
                }

                 // Render Ticker and Capture Result
                if let Some(pair) = self.ticker_state.render(ui) {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        self.handle_pair_selection(pair.clone());
                        self.scroll_to_pair = Some(pair);
                    }
                    // WASM: Suppress warning (Variable used)
                    #[cfg(target_arch = "wasm32")]
                    {
                        let _ = pair; 
                    }
                }
            });
        }

    pub(super) fn render_central_panel(&mut self, ctx: &Context) {
        // let central_panel_frame = Frame::new().fill(UI_CONFIG.colors.central_panel);
        let central_panel_frame = UI_CONFIG.central_panel_frame();

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
                    let interaction = self.plot_view.show_my_plot(
                        ui,
                        &model.cva,
                        &model,
                        current_price,
                        ScoreType::FullCandleTVW,
                        &self.plot_visibility,
                        engine,
                        self.candle_resolution,
                        nav_state.current_segment_idx,
                        self.auto_scale_y,
                    );

                    // HANDLE INTERACTION
                    match interaction {
                        PlotInteraction::UserInteracted => {
                            // User wants control. Disable auto-scale.
                            self.auto_scale_y = false;
                        }
                        PlotInteraction::RequestReset => {
                            // User requested reset. Re-enable auto-scale.
                            self.auto_scale_y = true;
                        }
                        PlotInteraction::None => {}
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
        let frame = UI_CONFIG.bottom_panel_frame();
        TopBottomPanel::bottom("status_panel")
            .frame(frame)
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        // 1. Simulation / Live Mode
                        self.render_status_mode(ui);
                        
                        // 2. Zone Info
                        self.render_status_zone_info(ui);
                        
                        ui.separator();

                        // 3. Coverage
                        self.render_status_coverage(ui);

                        // 4. Candle Stats
                        self.render_status_candles(ui);

                        // 5. System Status
                        self.render_status_system(ui);

                        ui.separator();

                        // 6. Network
                        self.render_status_network(ui);
                    });
                });
            });
    }

    fn render_status_mode(&self, ui: &mut Ui) {
        if let Some(pair) = &self.selected_pair {
            if self.is_simulation_mode() {
                #[cfg(target_arch = "wasm32")]
                let label = "WEB DEMO (OFFLINE)";
                #[cfg(not(target_arch = "wasm32"))]
                let label = "SIMULATION MODE";
                
                // Use Warning/Orange for Sim Mode
                ui.label(RichText::new(label).strong().color(PLOT_CONFIG.color_short)); 
                ui.separator();

                ui.label(RichText::new(format!("{}", self.sim_direction)).small().color(PLOT_CONFIG.color_info));
                ui.separator();
                ui.label(RichText::new(format!("| Step: {}", self.sim_step_size)).small().color(PLOT_CONFIG.color_profit));
                ui.separator();

                if let Some(sim_price) = self.simulated_prices.get(pair) {
                    ui.label(RichText::new(format!("💰 {}", format_price(*sim_price))).strong().color(PLOT_CONFIG.color_warning));
                }
            } else {
                // Live Mode
                ui.label(RichText::new("🟢 LIVE MODE").small().color(PLOT_CONFIG.color_profit));
                ui.separator();

                if let Some(price) = self.get_display_price(pair) {
                    ui.label(RichText::new(format!("💰 {}", format_price(price))).strong().color(PLOT_CONFIG.color_profit));
                } else {
                    ui.label("Connecting...");
                }
            }
        }
    }

    fn render_status_zone_info(&self, ui: &mut Ui) {
        if let Some(engine) = &self.engine {
            if let Some(pair) = &self.selected_pair {
                if let Some(model) = engine.get_model(pair) {
                    let cva = &model.cva;
                    let zone_size = (cva.price_range.end_range - cva.price_range.start_range) / cva.zone_count as f64;

                    ui.metric(
                        "📏 Zone Size",
                        &format!("{} (N={})", format_price(zone_size), cva.zone_count),
                        PLOT_CONFIG.color_info, // Light Blue
                    );
                    ui.separator();
                }
            }
        }
    }

    fn render_status_coverage(&self, ui: &mut Ui) {
        if let Some(engine) = &self.engine {
            if let Some(pair) = &self.selected_pair {
                if let Some(model) = engine.get_model(pair) {
                    // Helper closure for coverage colors
                    let cov_color = |pct: f64| {
                        if pct > 30.0 { PLOT_CONFIG.color_loss }      // Red (Too Cluttered)
                        else if pct < 5.0 { PLOT_CONFIG.color_warning } // Yellow (Too sparse)
                        else { PLOT_CONFIG.color_profit }             // Green (Good)
                    };

                    ui.label_subdued("Coverage");
                    ui.metric("Sticky", &format!("{:.0}%", model.coverage.sticky_pct), cov_color(model.coverage.sticky_pct));
                    ui.metric("R-Sup", &format!("{:.0}%", model.coverage.support_pct), cov_color(model.coverage.support_pct));
                    ui.metric("R-Res", &format!("{:.0}%", model.coverage.resistance_pct), cov_color(model.coverage.resistance_pct));
                    ui.separator();
                }
            }
        }
    }

    fn render_status_candles(&self, ui: &mut Ui) {
        if let Some(engine) = &self.engine {
            if let Some(pair) = &self.selected_pair {
                if let Some(model) = engine.get_model(pair) {
                    ui.separator();

                    let relevant = model.cva.relevant_candle_count;
                    let total = model.cva.total_candles;
                    let pct = if total > 0 { (relevant as f64 / total as f64) * 100.0 } else { 0.0 };
                    let time_str = TimeUtils::interval_to_string(model.cva.interval_ms);

                    ui.metric(
                        UI_TEXT.label_candle_count,
                        &format!("{}/{} ({:.1}%) {}", relevant, total, pct, time_str),
                        PLOT_CONFIG.color_text_neutral,
                    );

                    ui.separator();

                    ui.metric(
                        UI_TEXT.label_volatility,
                        &format!("{:.3}%", model.cva.volatility_pct),
                        PLOT_CONFIG.color_warning, // Volatility is attention-worthy
                    );
                }
            }
        }
    }

   fn render_status_system(&self, ui: &mut Ui) {
        if let Some(engine) = &self.engine {
            let total_pairs = engine.get_active_pair_count();
            ui.metric("📊 Pairs", &format!("{}", total_pairs), PLOT_CONFIG.color_text_neutral);

            if let Some(msg) = engine.get_worker_status_msg() {
                ui.separator();
                ui.label(RichText::new(format!("⚙ {}", msg)).small().color(PLOT_CONFIG.color_short));
            }

            let q_len = engine.get_queue_len();
            if q_len > 0 {
                ui.separator();
                ui.label(RichText::new(format!("Queue: {}", q_len)).small().color(PLOT_CONFIG.color_warning));
            }
        }
    }

    fn render_status_network(&self, ui: &mut Ui) {
        if let Some(engine) = &self.engine {
            let health = engine.price_stream.connection_health();
            let (icon, color) = if health >= 90.0 {
                ("🟢", PLOT_CONFIG.color_profit)
            } else if health >= 50.0 {
                ("🟡", PLOT_CONFIG.color_warning)
            } else {
                ("🔴", PLOT_CONFIG.color_loss)
            };
            ui.metric(
                &format!("{} Live Prices", icon),
                &format!("{:.0}% connected", health),
                color,
            );
        }
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
            .order(Order::Tooltip) // Need to set this because Plot draws elements on Order::Foreground (and redraws them every second) so we need be a higher-level than Foreground even
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
            &mut self.scroll_to_pair,
        );

        let events = panel.render(ui, &mut self.show_ph_help);

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

        let color = if is_error {
            PLOT_CONFIG.color_loss
        } else {
            PLOT_CONFIG.color_text_neutral
        };

        let text = RichText::new(subtitle).color(color);

        ui.label(text);
    });
}
