use eframe::egui::{
    CentralPanel, Context, Grid, RichText, ScrollArea, SidePanel, TopBottomPanel,
    Ui, Window, Order, Align, Layout, Color32, Frame, Sense, CursorIcon,
};

use strum::IntoEnumIterator;
use std::cmp::Ordering;

use crate::analysis::adaptive::AdaptiveParameters;

use crate::config::TICKER;

use crate::models::cva::ScoreType;
use crate::models::trading_view::{LiveOpportunity, DirectionFilter, TradeDirection};


use crate::ui::app::CandleResolution;
use crate::ui::config::{UI_CONFIG, UI_TEXT};
use crate::ui::styles::{UiStyleExt, get_outcome_color, DirectionColor};

use crate::config::plot::PLOT_CONFIG;

use crate::ui::ui_panels::{
    CandleRangePanel, DataGenerationEventChanged, DataGenerationPanel, Panel,
};
use crate::ui::ui_plot_view::PlotInteraction;

use crate::ui::utils::format_price;

use crate::utils::TimeUtils;
use crate::utils::maths_utils::{calculate_percent_diff, calculate_annualized_roi};

use super::app::ZoneSniperApp;

impl ZoneSniperApp {

        pub(super) fn render_opportunity_details_modal(&mut self, ctx: &Context) {
        // 1. Check if open
        if !self.show_opportunity_details { return; }

        // 2. Get Data (Thread-safe)
        let Some(pair) = self.selected_pair.clone() else { return; };
        let Some(model) = self.engine.as_ref().and_then(|e| e.get_model(&pair)) else { return; };
        let current_price = self.get_display_price(&pair).unwrap_or(0.0);

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

                    let calc_price = if current_price > f64::EPSILON { current_price } else { op.start_price };
                    let sim = &op.simulation;
                    // --- LOOKUP TARGET ZONE FOR TITLE ---
                    // Try to find the zone definition to get its bounds
                    let zone_info = model.zones.sticky_superzones.iter()
                        .find(|z| z.id == op.target_zone_id)
                        .map(|z| format!("{} - {}", format_price(z.price_bottom), format_price(z.price_top)))
                        .unwrap_or_else(|| format!("Zone #{}", op.target_zone_id)); // Fallback

                    ui.heading(format!("{}: {}", UI_TEXT.opp_exp_current_opp, zone_info));
                    // "Setup Type: LONG" (with encoded color)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label_subdued(format!("{}", UI_TEXT.opp_exp_setup_type));
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

                    // FIX: Get Dynamic Duration from the Opportunity itself
                    let max_time_ms = op.max_duration_ms;
                    let max_time_str = TimeUtils::format_duration(max_time_ms);
                    let max_candles = if interval_ms > 0 { max_time_ms / interval_ms } else { 0 };

                    // SECTION 1: THE MATH
                    ui.label_subheader(&UI_TEXT.opp_exp_expectancy);
                    let roi = op.live_roi(calc_price);
                    let ann_roi = op.live_annualized_roi(calc_price);
                    let roi_color = get_outcome_color(roi);
                    
                    ui.metric(
                        &format!("{}", UI_TEXT.label_roi),
                        &format!("{:+.2}%", roi), 
                        roi_color
                    );
                    ui.metric(&UI_TEXT.label_aroi_long, &format!("{:+.0}%", ann_roi), roi_color);
                    ui.metric(&UI_TEXT.label_success_rate, &format!("{:.1}%", sim.success_rate * 100.0), PLOT_CONFIG.color_text_primary);
                    ui.metric(&UI_TEXT.label_risk_reward, &format!("1:{:.0}", sim.risk_reward_ratio), PLOT_CONFIG.color_text_primary);

                    ui.add_space(10.0);

                    // SECTION 2: MARKET CONTEXT (INLINE STYLE)
                    ui.label_subheader(&UI_TEXT.opp_exp_market_context);
                    let state = &sim.market_state;
                    
                    // Volatility (Standard metric is fine)
                    ui.metric(&UI_TEXT.label_volatility, &format!("{:.2}%", state.volatility_pct * 100.0), PLOT_CONFIG.color_info);
                    
                    // Momentum (Inline Explanation)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(RichText::new(UI_TEXT.label_momentum.to_string() + ":").small().color(PLOT_CONFIG.color_text_subdued));
                        ui.label(RichText::new(format!("{:+.2}%", state.momentum_pct * 100.0)).small().color(get_outcome_color(state.momentum_pct)));
                        
                        ui.label(RichText::new(format!(
                            " ({} {}. {} {:.2}%)", 
                            UI_TEXT.opp_exp_trend_measured,
                            lookback_str,
                            UI_TEXT.opp_exp_trend_length,
                            ph_pct * 100.0,
                        )).small().color(PLOT_CONFIG.color_text_subdued));
                    });

                    // Rel Volume (Inline Explanation)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(RichText::new(UI_TEXT.opp_exp_relative_volume.to_string() + ":").small().color(PLOT_CONFIG.color_text_subdued));
                        let vol_color = if state.relative_volume > 1.0 { 
                            PLOT_CONFIG.color_warning 
                        } else { 
                            PLOT_CONFIG.color_text_subdued 
                        };
                        ui.label(RichText::new(format!("{:.2}x", state.relative_volume)).small().color(vol_color));
                        ui.label(RichText::new(UI_TEXT.opp_exp_relative_volume_explainer.to_string()).small().color(PLOT_CONFIG.color_text_subdued));
                    });

                    ui.add_space(10.0);

                  // SECTION 3: TRADE SETUP
                    ui.label_subheader(&UI_TEXT.opp_exp_trade_setup);
                    
                    // Entry / Target can stay standard
                    ui.metric(&UI_TEXT.opp_exp_trade_entry, &format_price(calc_price), PLOT_CONFIG.color_text_neutral);
                    // ui.metric(&UI_TEXT.label_target, &format_price(op.target_price), PLOT_CONFIG.color_profit);
                    
                    let target_dist = calculate_percent_diff(op.target_price, calc_price);
                    let stop_dist = calculate_percent_diff(op.stop_price, calc_price);

                    // TARGET ROW (Green)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(RichText::new(UI_TEXT.label_target_text.to_string() + ":").small().color(PLOT_CONFIG.color_text_subdued));
                        ui.label(RichText::new(format_price(op.target_price)).small().color(PLOT_CONFIG.color_profit));
                        
                        // Percentage inline
                        ui.label(RichText::new(format!("(+{:.2}%)", target_dist))
                            .small()
                            .color(PLOT_CONFIG.color_profit));
                    });

                    // Stop Loss Row + Variants
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(RichText::new(format!("{}:", UI_TEXT.label_stop_loss)).small().color(PLOT_CONFIG.color_text_subdued));
                        ui.label(RichText::new(format_price(op.stop_price)).small().color(PLOT_CONFIG.color_stop_loss));
                        
                        ui.label(RichText::new(format!(
                            "({} {:.2}% / {} {:.2}%)",
                            UI_TEXT.label_target_text,
                            target_dist,
                            UI_TEXT.label_stop_loss_short,
                            stop_dist
                        )).small().color(PLOT_CONFIG.color_text_subdued));

                        // NEW: Variants Info
                        if op.variant_count > 1 {
                            ui.label(RichText::new(format!(" ({} {})", op.variant_count, UI_TEXT.label_sl_variants))
                                .small()
                                .italics()
                                .color(PLOT_CONFIG.color_text_subdued));
                        }
                    });


                    // NEW: Time Limit Block
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(RichText::new(format!("{}:", UI_TEXT.opp_exp_order_time_limit)).small().color(PLOT_CONFIG.color_text_subdued));
                        // Use Info Color (Blue) or Primary (White) to make it distinct from price levels
                        ui.label(RichText::new(&max_time_str).small().color(PLOT_CONFIG.color_info));
                        
                        ui.label(RichText::new(format!("(~{} {})", max_candles, UI_TEXT.label_candle)).small().color(PLOT_CONFIG.color_text_subdued));
                    });


                    ui.add_space(15.0);
                    ui.separator();
                    ui.add_space(5.0);

                    // SECTION 4: THE STORY
                    ui.label_subheader(&UI_TEXT.opp_exp_how_this_works);
                    ui.vertical(|ui| {
                        ui.style_mut().spacing.item_spacing.y = 4.0;
                        let story_color = PLOT_CONFIG.color_text_neutral;
                        
                        ui.label(RichText::new(format!(
                            "{} ({} = {:.2}%, {} = {:+.2}%, {} = {:.2}x)",
                            UI_TEXT.opp_expr_we_fingerprinted,
                            UI_TEXT.label_volatility,
                            state.volatility_pct * 100.0,
                            UI_TEXT.label_momentum,
                            state.momentum_pct * 100.0,
                            UI_TEXT.opp_exp_relative_volume,
                            state.relative_volume
                        )).small().color(story_color).italics());
                        
                        let match_text = if sim.sample_size < 50 {
                            format!("{} {} {}", UI_TEXT.opp_exp_scanned_history_one, sim.sample_size, UI_TEXT.opp_exp_scanned_history_two)
                        } else {
                            format!("{} {} {}", UI_TEXT.opp_exp_scanned_history_three, sim.sample_size, UI_TEXT.opp_exp_scanned_history_four)
                        };
                        ui.label(RichText::new(match_text).small().color(story_color).italics());

                        ui.label(RichText::new(format!(
                            "{} {} {} {}, the {}, the {} ({}: {}).",
                            UI_TEXT.opp_exp_simulate_one,
                            sim.sample_size,
                            UI_TEXT.opp_exp_simulate_two,
                            UI_TEXT.label_target_text,
                            UI_TEXT.label_stop_loss,
                            UI_TEXT.opp_exp_out_of_time,
                            UI_TEXT.label_limit,
                            max_time_str
                        )).small().color(story_color).italics());
                        
                        let win_count = (sim.success_rate * sim.sample_size as f64).round() as usize;
                        let win_pct = sim.success_rate * 100.0;
                        
                        ui.label(RichText::new(format!(
                            "{} {} {} {} {} {} {} {:.1}% {} {}",
                            UI_TEXT.opp_exp_cases_one,
                            win_count,
                            UI_TEXT.opp_exp_cases_two,
                            sim.sample_size,
                            UI_TEXT.opp_exp_cases_three,
                            UI_TEXT.label_target_text,
                            UI_TEXT.opp_exp_cases_four,
                            win_pct,
                            UI_TEXT.label_success_rate,
                            UI_TEXT.opp_exp_cases_five,
                        )).small().color(story_color).italics());
                    });

                } else {
                    ui.label(&UI_TEXT.label_no_opps);
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
                                ctx.request_repaint();
                            }
                        } else {
                            ui.label(&UI_TEXT.error_no_model);
                        }
                    } else {
                        ui.label(&UI_TEXT.error_no_pair_selected);
                    }
                }
            });
    }

    /// Renders the Trade Finder as a dedicated Side Panel.
    /// Call this AFTER 'render_right_panel' in app.rs if you want it to sit to the LEFT of the nav panel.
    pub(super) fn render_trade_finder_panel(&mut self, ctx: &Context) {
        let frame = UI_CONFIG.side_panel_frame();

        SidePanel::right("trade_finder_panel") // <--- UNIQUE ID IS CRITICAL
            .min_width(280.0) // Wider for data visibility
            .resizable(true)  // Let user drag it
            .frame(frame)
            .show(ctx, |ui| {
                self.render_trade_finder_content(ui);
            });
    }

    fn render_trade_finder_content(&mut self, ui: &mut Ui) {
        // --- 1. HEADER ---
        ui.add_space(5.0);
        ui.heading(&UI_TEXT.tf_header); 
        ui.add_space(5.0);

        // --- 2. SCOPE TOGGLE (Selectable Row) ---
        ui.horizontal(|ui| {
            ui.label(RichText::new(&UI_TEXT.label_target).size(16.0).color(PLOT_CONFIG.color_text_neutral));
            ui.style_mut().spacing.item_spacing.x = 5.0; 
            
            // Button 1: ALL PAIRS
            if ui.selectable_label(!self.tf_filter_pair_only, &UI_TEXT.tf_scope_all).clicked() {
                self.tf_filter_pair_only = false;
            }
            
            // Button 2: PAIR ONLY
            let pair_label = if let Some(p) = &self.selected_pair { 
                format!("{} {}", p, UI_TEXT.tf_scope_selected) 
            } else { 
                format!("SELECTED {}", UI_TEXT.tf_scope_selected) 
            };
            
            if ui.selectable_label(self.tf_filter_pair_only, pair_label).clicked() {
                self.tf_filter_pair_only = true;
            }
        });
        ui.separator();

        // --- 2. DIRECTION FILTER ---
        ui.horizontal(|ui| {
            // ICON: Filter Funnel
            ui.label(RichText::new(&UI_TEXT.label_filter_icon).size(16.0).color(PLOT_CONFIG.color_text_neutral));
            ui.style_mut().spacing.item_spacing.x = 5.0; 
            
            let f = &mut self.tf_filter_direction;
            
            // Buttons using UI_TEXT fields (with icons embedded)
            if ui.selectable_label(*f == DirectionFilter::All, &UI_TEXT.tf_btn_all_trades).clicked() { *f = DirectionFilter::All; }
            if ui.selectable_label(*f == DirectionFilter::Long, &UI_TEXT.label_long).clicked() { *f = DirectionFilter::Long; }
            if ui.selectable_label(*f == DirectionFilter::Short, &UI_TEXT.label_short).clicked() { *f = DirectionFilter::Short; }
        });
        ui.separator();

        // --- 3. AGGREGATION ---
        let mut opportunities = if let Some(eng) = &self.engine {
            eng.get_all_live_opportunities()
        } else {
            vec![]
        };

        // Filter: Scope
        if self.tf_filter_pair_only {
            if let Some(current) = &self.selected_pair {
                opportunities.retain(|op| op.opportunity.pair_name == *current);
            }
        }

        // Filter: Direction
        match self.tf_filter_direction {
            DirectionFilter::Long => opportunities.retain(|op| op.opportunity.direction == TradeDirection::Long),
            DirectionFilter::Short => opportunities.retain(|op| op.opportunity.direction == TradeDirection::Short),
            DirectionFilter::All => {},
        }

        // Filter: Positive ROI Only (Remove losers)
        // We filter based on the LIVE ROI to be safe (if it drifted negative, hide it)
        opportunities.retain(|op| op.opportunity.expected_roi() > 0.0);

        // Sort: STABLE SORT (Use Static Metrics)
        // We calculate the AROI based on the *Start Price* (Static), not *Live Price*
        opportunities.sort_by(|a, b| {
            let a_static_roi = a.opportunity.expected_roi(); // Uses start_price
            let a_static_aroi = calculate_annualized_roi(a_static_roi, a.opportunity.avg_duration_ms as f64);
            
            let b_static_roi = b.opportunity.expected_roi();
            let b_static_aroi = calculate_annualized_roi(b_static_roi, b.opportunity.avg_duration_ms as f64);
            
            b_static_aroi.partial_cmp(&a_static_aroi).unwrap_or(Ordering::Equal)
        });

        // --- 4. RENDER LIST ---
        ScrollArea::vertical().id_salt("tf_list_scroll").show(ui, |ui| {
            if opportunities.is_empty() {
                ui.add_space(20.0);
                ui.centered_and_justified(|ui| ui.label(&UI_TEXT.label_no_opps));
                return;
            }

            for live_op in opportunities {
                self.render_trade_card(ui, live_op);
            }
        });
    }

    fn render_trade_card(&mut self, ui: &mut Ui, live_op: LiveOpportunity) {
        let op = &live_op.opportunity;
        
        // Match selection
        let is_selected = self.selected_opportunity.as_ref().map_or(false, |sel| 
            sel.target_zone_id == op.target_zone_id && sel.pair_name == op.pair_name
        );

        // Higher contrast selection color
        let card_color = if is_selected { 
            Color32::from_white_alpha(30) // Stronger highlight
        } else { 
            Color32::TRANSPARENT 
        };

        // FRAME is the Button
        let inner_response = Frame::new()
            .fill(card_color)
            .inner_margin(6.0) // More padding
            .corner_radius(4.0)     // Rounded corners
            .show(ui, |ui| {
                
                ui.horizontal(|ui| {
                    // COLUMN A: Pair & Direction
                    ui.vertical(|ui| {
                        ui.set_min_width(90.0); // Fixed width for alignment
                        
                        // Pair Name
                        ui.label(RichText::new(&op.pair_name).strong().size(14.0).color(PLOT_CONFIG.color_text_primary));
                        
                        // Direction Icon
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            let dir_color = op.direction.color();
                            // ui.label(RichText::new(arrow).color(dir_color));
                            ui.label(RichText::new(op.direction.to_string().to_uppercase()).small().color(dir_color));
                        });
                    });

                    // COLUMN B: ROI Stats
                    ui.vertical(|ui| {
                        ui.set_min_width(80.0);
                        
                        let roi_color = get_outcome_color(live_op.live_roi);
                        ui.label(RichText::new(format!("{}: {:+.2}%", UI_TEXT.label_roi, live_op.live_roi)).strong().color(roi_color));
                        
                        // AROI
                        ui.label(RichText::new(format!("{}: {:+.0}%", UI_TEXT.label_aroi, live_op.annualized_roi))
                            .small()
                            .color(roi_color.linear_multiply(0.8)));
                    });

                    // COLUMN C: Info Badges
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if op.variant_count > 1 {
                            ui.label(RichText::new(format!("({} {})", op.variant_count, UI_TEXT.label_sl_variants))
                                .small()
                                .italics()
                                .color(PLOT_CONFIG.color_text_subdued));
                        }
                    });
                });
                
                ui.add_space(2.0);

                // FOOTER: Targets
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}: +{:.2}%", &UI_TEXT.tf_target, live_op.reward_pct)).small().color(PLOT_CONFIG.color_profit));
                    ui.label(RichText::new("|").small().color(PLOT_CONFIG.color_text_subdued));
                    ui.label(RichText::new(format!("{}: -{:.2}%", &UI_TEXT.label_stop_loss_short, live_op.risk_pct)).small().color(PLOT_CONFIG.color_stop_loss));
                });
            });

        // HANDLE INTERACTION
        // 1. Make the whole frame clickable
        let response = inner_response.response.interact(Sense::click());

        // 2. FIX: Force "Arrow" cursor (Default) on hover
        // This overrides the text-selection "Caret" cursor that appears over labels.
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Default);
        }

        // 3. Handle Click
        if response.clicked() {
            self.handle_pair_selection(op.pair_name.clone());
            self.scroll_to_pair = Some(op.pair_name.clone());
            self.selected_opportunity = Some(op.clone());
        }
        
        ui.separator();
    }

    pub(super) fn render_left_panel(&mut self, ctx: &Context) {
        let frame = UI_CONFIG.side_panel_frame();

        SidePanel::left("left_panel")
            .min_width(140.0)
            .resizable(false)
            .frame(frame)
            .show(ctx, |ui| {

                let data_events = self.data_generation_panel(ui);

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
                    ui.label(RichText::new(&UI_TEXT.tb_time).size(16.0).color(PLOT_CONFIG.color_text_neutral));

                    for res in CandleResolution::iter() {
                        ui.selectable_value(&mut self.candle_resolution, res, res.to_string());
                     }

                    ui.separator();

                    // 2. LAYER VISIBILITY
                    ui.checkbox(&mut self.plot_visibility.sticky, &UI_TEXT.tb_sticky);
                    ui.checkbox(&mut self.plot_visibility.low_wicks, &UI_TEXT.tb_low_wicks);
                    ui.checkbox(&mut self.plot_visibility.high_wicks, &UI_TEXT.tb_high_wicks);
                    ui.checkbox(&mut self.plot_visibility.background, &UI_TEXT.tb_volume_hist);
                    ui.checkbox(&mut self.plot_visibility.candles, &UI_TEXT.tb_candles);

                    ui.separator();

                    // CONTEXT
                    ui.checkbox(&mut self.plot_visibility.separators, &UI_TEXT.tb_gaps);
                    ui.checkbox(&mut self.plot_visibility.horizon_lines, &UI_TEXT.tb_price_limits);
                    ui.checkbox(&mut self.plot_visibility.price_line, &UI_TEXT.tb_live_price);
                    ui.checkbox(&mut self.plot_visibility.opportunities, &UI_TEXT.tb_targets);

                    // STATUS INDICATOR (TEMP but very useful)
                    if self.auto_scale_y {
                        ui.label(RichText::new(&UI_TEXT.tb_y_locked).small().color(PLOT_CONFIG.color_profit));
                    } else {
                        ui.label(RichText::new(&UI_TEXT.tb_y_unlocked).small().color(PLOT_CONFIG.color_warning));
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
                        &UI_TEXT.cp_system_starting,
                        &UI_TEXT.cp_init_engine,
                        false,
                    );
                    return;
                };

                // 2. Safety Check: Selected Pair
                let Some(pair) = self.selected_pair.clone() else {
                    render_fullscreen_message(
                        ui,
                        &UI_TEXT.error_no_pair_selected,
                        &UI_TEXT.cp_please_select_pair,
                        false,
                    );
                    return;
                };

                // 3. Get Price State (Do we have a live price?)
                let current_price = self.get_display_price(&pair); // engine.get_price(&pair);

                let (is_calculating, last_error) = engine.get_pair_status(&pair);

                // PRIORITY 1: ERRORS
                // If the most recent calculation failed (e.g. "Insufficient Data" at 1%), show the error, even if we have an old cached model.
                if let Some(err_msg) = last_error {
                    let body = if err_msg.contains("Insufficient data") {
                        format!("{}\n\n{}", UI_TEXT.error_insufficient_data_body, err_msg)
                    } else {
                        err_msg.to_string()
                    };
                    render_fullscreen_message(ui, &UI_TEXT.error_analysis_failed, &body, true);
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
                        self.selected_opportunity.clone(),
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
                        &format!("{} {}...", UI_TEXT.cp_analyzing, pair),
                        &UI_TEXT.cp_calculating_zones,
                        false,
                    );
                }
                // PRIORITY 4: QUEUED / WAITING
                else if current_price.is_some() {
                    render_fullscreen_message(
                        ui,
                        &format!("{}: {}...", UI_TEXT.cp_queued, pair),
                        &UI_TEXT.cp_wait_thread,
                        false,
                    );
                }
                // PRIORITY 5: NO DATA STREAM
                else {
                    render_fullscreen_message(
                        ui,
                        &UI_TEXT.cp_wait_prices,
                        &UI_TEXT.cp_listen_binance_stream,
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
                let label = &UI_TEXT.sp_simulation_mode;
                
                // SIM Mode - Use Warning/Orange for Sim Mode
                ui.label(RichText::new(label).strong().color(PLOT_CONFIG.color_short)); 
                ui.separator();
                ui.label(RichText::new(format!("{}", self.sim_direction)).small().color(PLOT_CONFIG.color_info));
                ui.separator();
                ui.label(RichText::new(format!("{}: {}", UI_TEXT.sim_step, self.sim_step_size)).small().color(PLOT_CONFIG.color_profit));
                ui.separator();
                if let Some(sim_price) = self.simulated_prices.get(pair) {
                    ui.label(RichText::new(format!("{} {}", UI_TEXT.sp_price, format_price(*sim_price))).strong().color(PLOT_CONFIG.color_short));
                }
            } else {
                // Live Mode
                ui.label(RichText::new(format!("{} ",&UI_TEXT.sp_live_mode)).small().color(PLOT_CONFIG.color_profit));
                ui.separator();

                if let Some(price) = self.get_display_price(pair) {
                    ui.label(RichText::new(format!("{} {}", UI_TEXT.sp_price, format_price(price))).strong().color(PLOT_CONFIG.color_text_primary));
                } else {
                    ui.label(format!("{} ...", UI_TEXT.label_connecting));
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
                        &UI_TEXT.sp_zone_size,
                        &format!("{}", format_price(zone_size)),
                        PLOT_CONFIG.color_info,
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
                    ui.label_subdued(&UI_TEXT.sp_coverage);
                    ui.metric(&UI_TEXT.sp_coverage_sticky, &format!("{:.1}%", model.coverage.sticky_pct), cov_color(model.coverage.sticky_pct));
                    ui.metric(&UI_TEXT.sp_coverage_support, &format!("{:.1}%", model.coverage.support_pct), cov_color(model.coverage.support_pct));
                    ui.metric(&UI_TEXT.sp_coverage_resistance, &format!("{:.1}%", model.coverage.resistance_pct), cov_color(model.coverage.resistance_pct));
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

                    ui.metric(
                        &UI_TEXT.label_candle,
                        &format!("{}/{} ({:.1}%)", relevant, total, pct),
                        PLOT_CONFIG.color_text_neutral,
                    );

                    ui.separator();

                    ui.metric(
                        &UI_TEXT.label_volatility,
                        &format!("{:.3}%", model.cva.volatility_pct),
                        PLOT_CONFIG.color_warning, // Volatility is attention-worthy
                    );
                }
            }
        }
    }

   fn render_status_system(&self, ui: &mut Ui) {
        if let Some(engine) = &self.engine {
            if let Some(msg) = engine.get_worker_status_msg() {
                ui.separator();
                ui.label(RichText::new(format!("{} {}", UI_TEXT.label_working, msg)).small().color(PLOT_CONFIG.color_short));
            }

            let q_len = engine.get_queue_len();
            if q_len > 0 {
                ui.separator();
                ui.label(RichText::new(format!("{}: {}", UI_TEXT.label_queue, q_len)).small().color(PLOT_CONFIG.color_warning));
            }
        }
    }

    fn render_status_network(&self, ui: &mut Ui) {
        if let Some(engine) = &self.engine {
            let health = engine.price_stream.connection_health();
            let color = if health >= 90.0 {
                PLOT_CONFIG.color_profit
            } else if health >= 50.0 {
                PLOT_CONFIG.color_warning
            } else {
                PLOT_CONFIG.color_loss
            };
            ui.metric(
                &UI_TEXT.sp_stream_status,
                &format!("{:.0}% {}", health, UI_TEXT.label_connected),
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

        Window::new(&UI_TEXT.kbs_name_long)
            .open(&mut self.show_debug_help)
            .resizable(false)
            .order(Order::Tooltip) // Need to set this because Plot draws elements on Order::Foreground (and redraws them every second) so we need be a higher-level than Foreground even
            .collapsible(false)
            .default_width(400.0)
            .show(ctx, |ui| {
                ui.heading("Press keys to execute commands");
                ui.add_space(10.0);

                // 1. General Shortcuts
                let mut _general_shortcuts = vec![
                    ("ESC", UI_TEXT.kbs_close_all_panes.as_str()),
                    ("K (or H)", UI_TEXT.kbs_open_close.as_str()),
                    ("1", UI_TEXT.kbs_toolbar_shortcut_hvz.as_str()),
                    ("2", UI_TEXT.kbs_toolbar_shortcut_low_wick.as_str()),
                    ("3", UI_TEXT.kbs_toolbar_shortcut_low_wick.as_str()),

                    ("4", UI_TEXT.kbs_toolbar_shortcut_histogram.as_str()),
                    ("5", UI_TEXT.kbs_toolbar_shortcut_candles.as_str()),
                    ("6", UI_TEXT.kbs_toolbar_shortcut_gap.as_str()),
                    ("7", UI_TEXT.kbs_toolbar_shortcut_price_limits.as_str()),
                    ("8", UI_TEXT.kbs_toolbar_shortcut_live_price.as_str()),
                    ("9", UI_TEXT.kbs_toolbar_shortcut_targets.as_str()),
                    // Note, can use '0' as well here ie numeric zero, if we need antoher one

                    ("O", UI_TEXT.kbs_view_opp_explainer.as_str()),
                    ("T", UI_TEXT.kbs_view_time_machine.as_str()),
                ];

                // Only add 'S' for Native
                #[cfg(not(target_arch = "wasm32"))]
                _general_shortcuts.push(("S", &UI_TEXT.kbs_sim_mode));

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
                    ui.heading(&UI_TEXT.sim_mode_controls);
                    ui.add_space(5.0);

                    ui.add_space(5.0);

                    let mut _sim_shortcuts = vec![
                        ("D", UI_TEXT.sim_help_sim_toggle_direction.as_str()),
                        ("X", UI_TEXT.sim_help_sim_step_size.as_str()),
                        ("A", UI_TEXT.sim_help_sim_activate_price_change.as_str()),
                        ("Y", UI_TEXT.sim_help_sim_jump_hvz.as_str()),
                        ("L", UI_TEXT.sim_help_sim_jump_lower_wicks.as_str()),
                        ("W", UI_TEXT.sim_help_sim_jump_higher_wicks.as_str()),
                    ];

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
                    // Note: any keys added here have to be hand-inserted in handle_global_shortcuts to activate them, too
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

    fn data_generation_panel(&mut self, ui: &mut Ui) -> Vec<DataGenerationEventChanged> {
        // 1. Get Data from Engine
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
            ui.heading(format!("{} {}", UI_TEXT.label_warning, title));
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
