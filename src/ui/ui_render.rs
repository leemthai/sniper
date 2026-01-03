use eframe::egui::{
    Align, CentralPanel, Context, FontId, Grid, Layout, Order, RichText, SidePanel, TopBottomPanel, Ui, Window,
};

use egui_extras::{TableBuilder, Column, TableRow};

use std::cmp::Ordering;
use strum::IntoEnumIterator;

use crate::analysis::adaptive::AdaptiveParameters;

use crate::config::plot::PLOT_CONFIG;
use crate::config::{TICKER};

use crate::domain::pair_interval::PairInterval;
use crate::models::cva::ScoreType;
use crate::models::trading_view::{
    DirectionFilter, SortColumn, SortDirection, TradeDirection, TradeFinderRow, TradeOpportunity,
};

use crate::ui::app::CandleResolution;
use crate::ui::config::{UI_CONFIG, UI_TEXT};
use crate::ui::styles::{DirectionColor, UiStyleExt, get_outcome_color};
use crate::ui::ui_panels::{
    CandleRangePanel, DataGenerationEventChanged, DataGenerationPanel, Panel,
};
use crate::ui::ui_plot_view::PlotInteraction;
use crate::ui::utils::format_price;

use crate::utils::TimeUtils;
use crate::utils::maths_utils::{calculate_percent_diff, format_volume_compact};

use super::app::ZoneSniperApp;

impl ZoneSniperApp {
    // Helper to sort rows (Updated for Zero-Op handling)
    fn sort_trade_finder_rows(&self, rows: &mut Vec<TradeFinderRow>) {
        rows.sort_by(|a, b| {
            // 1. Always push "No Opportunity" rows to the bottom
            let a_has = a.opportunity.is_some();
            let b_has = b.opportunity.is_some();

            if a_has != b_has {
                if a_has {
                    return Ordering::Less;
                }
                // A (Has) < B (Empty) -> A First
                else {
                    return Ordering::Greater;
                }
            }

            // 2. Standard Sort
            let cmp = match self.tf_sort_col {
                SortColumn::PairName => a.pair_name.cmp(&b.pair_name),

                SortColumn::QuoteVolume24h => a.quote_volume_24h.total_cmp(&b.quote_volume_24h),

                SortColumn::Volatility => {
                    let va = a
                        .market_state
                        .as_ref()
                        .map(|m| m.volatility_pct)
                        .unwrap_or(0.0);
                    let vb = b
                        .market_state
                        .as_ref()
                        .map(|m| m.volatility_pct)
                        .unwrap_or(0.0);
                    va.total_cmp(&vb)
                }
                SortColumn::Momentum => {
                    let ma = a
                        .market_state
                        .as_ref()
                        .map(|m| m.momentum_pct)
                        .unwrap_or(0.0);
                    let mb = b
                        .market_state
                        .as_ref()
                        .map(|m| m.momentum_pct)
                        .unwrap_or(0.0);
                    ma.total_cmp(&mb)
                }

                SortColumn::LiveRoi => {
                    let val_a = a
                        .opportunity
                        .as_ref()
                        .map(|o| o.live_roi)
                        .unwrap_or(f64::NEG_INFINITY);
                    let val_b = b
                        .opportunity
                        .as_ref()
                        .map(|o| o.live_roi)
                        .unwrap_or(f64::NEG_INFINITY);
                    val_a.total_cmp(&val_b)
                }
                SortColumn::AnnualizedRoi => {
                    let val_a = a
                        .opportunity
                        .as_ref()
                        .map(|o| o.annualized_roi)
                        .unwrap_or(f64::NEG_INFINITY);
                    let val_b = b
                        .opportunity
                        .as_ref()
                        .map(|o| o.annualized_roi)
                        .unwrap_or(f64::NEG_INFINITY);
                    val_a.total_cmp(&val_b)
                }
                SortColumn::AvgDuration => {
                    let val_a = a
                        .opportunity
                        .as_ref()
                        .map(|o| o.opportunity.avg_duration_ms)
                        .unwrap_or(i64::MAX);
                    let val_b = b
                        .opportunity
                        .as_ref()
                        .map(|o| o.opportunity.avg_duration_ms)
                        .unwrap_or(i64::MAX);
                    val_b.cmp(&val_a)
                }
                SortColumn::OpportunityCount => {
                    a.opportunity_count_total.cmp(&b.opportunity_count_total)
                }
                SortColumn::VariantCount => {
                    let va = a
                        .opportunity
                        .as_ref()
                        .map(|o| o.opportunity.variant_count())
                        .unwrap_or(0);
                    let vb = b
                        .opportunity
                        .as_ref()
                        .map(|o| o.opportunity.variant_count())
                        .unwrap_or(0);
                    va.cmp(&vb)
                }

                _ => std::cmp::Ordering::Equal,
            };

            match self.tf_sort_dir {
                SortDirection::Ascending => cmp,
                SortDirection::Descending => cmp.reverse(),
            }
        });
    }

    pub(super) fn render_opportunity_details_modal(&mut self, ctx: &Context) {
        // 1. Check if open
        if !self.show_opportunity_details {
            return;
        }

        // 2. Get Data (Thread-safe)
        let Some(pair) = self.selected_pair.clone() else {
            return;
        };
        let Some(model) = self.engine.as_ref().and_then(|e| e.get_model(&pair)) else {
            return;
        };
        let current_price = self.get_display_price(&pair).unwrap_or(0.0);

        // 3. Find the "Current Opportunity" (Same logic as HUD)
        let best_opp = model
            .opportunities
            .iter()
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
                    let calc_price = if current_price > f64::EPSILON {
                        current_price
                    } else {
                        op.start_price
                    };
                    let sim = &op.simulation;
                    // --- LOOKUP TARGET ZONE FOR TITLE ---
                    // Try to find the zone definition to get its bounds
                    let zone_info = model
                        .zones
                        .sticky_superzones
                        .iter()
                        .find(|z| z.id == op.target_zone_id)
                        .map(|z| {
                            format!(
                                "{} - {}",
                                format_price(z.price_bottom),
                                format_price(z.price_top)
                            )
                        })
                        .unwrap_or_else(|| format!("Zone #{}", op.target_zone_id)); // Fallback

                    ui.heading(format!("{}: {}", UI_TEXT.opp_exp_current_opp, zone_info));
                    // "Setup Type: LONG" (with encoded color)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label_subdued(format!("{}", UI_TEXT.opp_exp_setup_type));
                        ui.label(
                            RichText::new(op.direction.to_string().to_uppercase())
                                .strong()
                                .color(op.direction.color()),
                        ); // Uses PLOT_CONFIG via trait
                    });
                    ui.separator();

                    // --- CALCULATIONS ---
                    // 1. Get PH %
                    let ph_pct = self.app_config.price_horizon.threshold_pct;

                    // 2. Calculate Actual Lookback used (using new Adaptive logic)
                    let lookback_candles =
                        AdaptiveParameters::calculate_trend_lookback_candles(ph_pct);
                    let interval_ms = model.cva.interval_ms;
                    let lookback_ms = lookback_candles as i64 * interval_ms;
                    let lookback_str = TimeUtils::format_duration(lookback_ms);

                    // Get Dynamic Duration from the Opportunity itself
                    let max_time_ms = op.max_duration_ms;
                    let max_time_str = TimeUtils::format_duration(max_time_ms);
                    let max_candles = if interval_ms > 0 {
                        max_time_ms / interval_ms
                    } else {
                        0
                    };

                    // SECTION 1: THE MATH
                    ui.label_subheader(&UI_TEXT.opp_exp_expectancy);
                    let roi = op.live_roi(calc_price);
                    let ann_roi = op.live_annualized_roi(calc_price);
                    let roi_color = get_outcome_color(roi);

                    ui.metric(
                        &format!("{}", UI_TEXT.label_roi),
                        &format!("{:+.2}%", roi),
                        roi_color,
                    );
                    ui.metric(
                        &UI_TEXT.label_aroi_long,
                        &format!("{:+.0}%", ann_roi),
                        roi_color,
                    );

                    // Avg Duration
                    let avg_time_str = TimeUtils::format_duration(op.avg_duration_ms);
                    ui.metric(
                        &UI_TEXT.label_avg_duration,
                        &avg_time_str,
                        PLOT_CONFIG.color_text_neutral,
                    );

                    ui.metric(
                        &UI_TEXT.label_success_rate,
                        &format!("{:.1}%", sim.success_rate * 100.0),
                        PLOT_CONFIG.color_text_primary,
                    );
                    ui.metric(
                        &UI_TEXT.label_risk_reward,
                        &format!("1:{:.0}", sim.risk_reward_ratio),
                        PLOT_CONFIG.color_text_primary,
                    );

                    ui.add_space(10.0);

                    // SECTION 2: MARKET CONTEXT (INLINE STYLE)
                    ui.label_subheader(&UI_TEXT.opp_exp_market_context);
                    let state = &sim.market_state;

                    // Volatility
                    ui.metric(
                        &UI_TEXT.label_volatility,
                        &format!("{:.2}%", state.volatility_pct * 100.0),
                        PLOT_CONFIG.color_info,
                    );

                    // Momentum
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(
                            RichText::new(UI_TEXT.label_momentum.to_string() + ":")
                                .small()
                                .color(PLOT_CONFIG.color_text_subdued),
                        );
                        ui.label(
                            RichText::new(format!("{:+.2}%", state.momentum_pct * 100.0))
                                .small()
                                .color(get_outcome_color(state.momentum_pct)),
                        );

                        ui.label(
                            RichText::new(format!(
                                " ({} {}. {} {:.2}%)",
                                UI_TEXT.opp_exp_trend_measured,
                                lookback_str,
                                UI_TEXT.opp_exp_trend_length,
                                ph_pct * 100.0,
                            ))
                            .small()
                            .color(PLOT_CONFIG.color_text_subdued),
                        );
                    });

                    // Relative Volume
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(
                            RichText::new(UI_TEXT.opp_exp_relative_volume.to_string() + ":")
                                .small()
                                .color(PLOT_CONFIG.color_text_subdued),
                        );
                        let vol_color = if state.relative_volume > 1.0 {
                            PLOT_CONFIG.color_warning
                        } else {
                            PLOT_CONFIG.color_text_subdued
                        };
                        ui.label(
                            RichText::new(format!("{:.2}x", state.relative_volume))
                                .small()
                                .color(vol_color),
                        );
                        ui.label(
                            RichText::new(UI_TEXT.opp_exp_relative_volume_explainer.to_string())
                                .small()
                                .color(PLOT_CONFIG.color_text_subdued),
                        );
                    });

                    ui.add_space(10.0);

                    // SECTION 3: TRADE SETUP
                    ui.label_subheader(&UI_TEXT.opp_exp_trade_setup);

                    // Entry / Target can stay standard
                    ui.metric(
                        &UI_TEXT.opp_exp_trade_entry,
                        &format_price(calc_price),
                        PLOT_CONFIG.color_text_neutral,
                    );
                    let target_dist = calculate_percent_diff(op.target_price, calc_price);
                    let stop_dist = calculate_percent_diff(op.stop_price, calc_price);

                    // TARGET ROW
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(
                            RichText::new(UI_TEXT.label_target_text.to_string() + ":")
                                .small()
                                .color(PLOT_CONFIG.color_text_subdued),
                        );
                        ui.label(
                            RichText::new(format_price(op.target_price))
                                .small()
                                .color(PLOT_CONFIG.color_profit),
                        );
                        ui.label(
                            RichText::new(format!("(+{:.2}%)", target_dist))
                                .small()
                                .color(PLOT_CONFIG.color_profit),
                        );
                    });

                    // Stop Loss Row + Variants
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(
                            RichText::new(format!("{}:", UI_TEXT.label_stop_loss))
                                .small()
                                .color(PLOT_CONFIG.color_text_subdued),
                        );
                        ui.label(
                            RichText::new(format_price(op.stop_price))
                                .small()
                                .color(PLOT_CONFIG.color_stop_loss),
                        );
                        ui.label(
                            RichText::new(format!(
                                "({} {:.2}% / {} {:.2}%)",
                                UI_TEXT.label_target_text,
                                target_dist,
                                UI_TEXT.label_stop_loss_short,
                                stop_dist
                            ))
                            .small()
                            .color(PLOT_CONFIG.color_text_subdued),
                        );

                        // Variants Info
                        if op.variant_count() > 1 {
                            ui.label(
                                RichText::new(format!(
                                    " ({} {})",
                                    op.variant_count(),
                                    UI_TEXT.label_sl_variants
                                ))
                                .small()
                                .italics()
                                .color(PLOT_CONFIG.color_text_subdued),
                            );
                        }
                    });

                    // Time Limit Block
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(
                            RichText::new(format!("{}:", UI_TEXT.opp_exp_order_time_limit))
                                .small()
                                .color(PLOT_CONFIG.color_text_subdued),
                        );
                        ui.label(
                            RichText::new(&max_time_str)
                                .small()
                                .color(PLOT_CONFIG.color_info),
                        );
                        ui.label(
                            RichText::new(format!("(~{} {})", max_candles, UI_TEXT.label_candle))
                                .small()
                                .color(PLOT_CONFIG.color_text_subdued),
                        );
                    });

                    ui.add_space(15.0);
                    ui.separator();
                    ui.add_space(5.0);

                    // SECTION 4: THE STORY
                    ui.label_subheader(&UI_TEXT.opp_exp_how_this_works);
                    ui.vertical(|ui| {
                        ui.style_mut().spacing.item_spacing.y = 4.0;
                        let story_color = PLOT_CONFIG.color_text_neutral;

                        ui.label(
                            RichText::new(format!(
                                "{} ({} = {:.2}%, {} = {:+.2}%, {} = {:.2}x)",
                                UI_TEXT.opp_expr_we_fingerprinted,
                                UI_TEXT.label_volatility,
                                state.volatility_pct * 100.0,
                                UI_TEXT.label_momentum,
                                state.momentum_pct * 100.0,
                                UI_TEXT.opp_exp_relative_volume,
                                state.relative_volume
                            ))
                            .small()
                            .color(story_color)
                            .italics(),
                        );

                        let match_text = if sim.sample_size < 50 {
                            format!(
                                "{} {} {}",
                                UI_TEXT.opp_exp_scanned_history_one,
                                sim.sample_size,
                                UI_TEXT.opp_exp_scanned_history_two
                            )
                        } else {
                            format!(
                                "{} {} {}",
                                UI_TEXT.opp_exp_scanned_history_three,
                                sim.sample_size,
                                UI_TEXT.opp_exp_scanned_history_four
                            )
                        };
                        ui.label(
                            RichText::new(match_text)
                                .small()
                                .color(story_color)
                                .italics(),
                        );
                        ui.label(
                            RichText::new(format!(
                                "{} {} {} {}, the {}, the {} ({}: {}).",
                                UI_TEXT.opp_exp_simulate_one,
                                sim.sample_size,
                                UI_TEXT.opp_exp_simulate_two,
                                UI_TEXT.label_target_text,
                                UI_TEXT.label_stop_loss,
                                UI_TEXT.opp_exp_out_of_time,
                                UI_TEXT.label_limit,
                                max_time_str
                            ))
                            .small()
                            .color(story_color)
                            .italics(),
                        );

                        let win_count =
                            (sim.success_rate * sim.sample_size as f64).round() as usize;
                        let win_pct = sim.success_rate * 100.0;

                        ui.label(
                            RichText::new(format!(
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
                            ))
                            .small()
                            .color(story_color)
                            .italics(),
                        );
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

                            let mut panel =
                                CandleRangePanel::new(&model.segments, nav.current_segment_idx);

                            if let Some(new_idx) = panel.render(ui, safe_last) {
                                nav.current_segment_idx = new_idx;
                                // If we switched to a specific segment (not "Show All"), remember it.
                                if let Some(idx) = new_idx {
                                    nav.last_viewed_segment_idx = idx;
                                }
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

    /// Helper: Renders the Header, Scope, and Direction controls
    fn render_trade_finder_filters(&mut self, ui: &mut Ui) {
        // --- 1. HEADER ---
        ui.add_space(5.0);
        ui.heading(&UI_TEXT.tf_header);
        ui.add_space(5.0);

        // --- 2. SCOPE TOGGLE ---
        ui.horizontal(|ui| {
            // Icon/Label
            ui.label(
                RichText::new(&UI_TEXT.label_target)
                    .size(16.0)
                    .color(PLOT_CONFIG.color_text_neutral),
            );
            ui.style_mut().spacing.item_spacing.x = 5.0;

            // "ALL PAIRS" Button
            if ui
                .selectable_label(!self.tf_filter_pair_only, &UI_TEXT.tf_scope_all)
                .clicked()
            {
                self.tf_filter_pair_only = false;
                // LOGIC FIX: When switching to ALL, scroll to the current selection so we don't lose it
                if let Some(p) = &self.selected_pair {
                    #[cfg(debug_assertions)]
                    log::info!("UI: Scope changed to ALL. Requesting scroll to {}", p);
                    self.scroll_to_pair = Some(p.clone());
                }
            }

            // "BTC ONLY"
            let base_asset = self
                .selected_pair
                .as_ref()
                .and_then(|p| PairInterval::get_base(p))
                .unwrap_or("SELECTED");

            let pair_label = format!("{} {}", base_asset, UI_TEXT.tf_scope_selected);
            if ui
                .selectable_label(self.tf_filter_pair_only, pair_label)
                .clicked()
            {
                self.tf_filter_pair_only = true;
            }
            // --- SEPARATOR ---
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // --- DIRECTION FILTER ---
            ui.label(
                RichText::new(&UI_TEXT.label_filter_icon)
                    .size(16.0)
                    .color(PLOT_CONFIG.color_text_neutral),
            );
            ui.style_mut().spacing.item_spacing.x = 5.0;

            let f = &mut self.tf_filter_direction;

            // 3-Way Toggle
            if ui
                .selectable_label(*f == DirectionFilter::All, &UI_TEXT.tf_btn_all_trades)
                .clicked()
            {
                *f = DirectionFilter::All;
            }
            if ui
                .selectable_label(*f == DirectionFilter::Long, &UI_TEXT.label_long)
                .clicked()
            {
                *f = DirectionFilter::Long;
            }
            if ui
                .selectable_label(*f == DirectionFilter::Short, &UI_TEXT.label_short)
                .clicked()
            {
                *f = DirectionFilter::Short;
            }
        });
    }


    // --- COLUMN HELPERS ---

    /// Helper for Empty Cells (SSOT)
    fn display_no_data(&self, ui: &mut Ui) {
        ui.label("-");
    }

    fn render_trade_finder_content(&mut self, ui: &mut Ui) {
        // 1. Controls
        self.render_trade_finder_filters(ui);

        // 2. Data Fetch & Filter
        let mut rows = self.get_filtered_rows();

        // 3. Auto-Heal
        self.auto_heal_selection(&rows);

        // 4. Sort
        self.sort_trade_finder_rows(&mut rows);

        // 5. Render Table
        if rows.is_empty() {
            ui.label(&UI_TEXT.label_no_opps);
            return;
        }

        // Available height for the table (minus headers/footers if any)
        let available_height = ui.available_height();

        let viable_min_column_width= 10.;

        TableBuilder::new(ui)
            .striped(true)
            .resizable(false)
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::initial(100.0).at_least(80.0))  // Pair + Dir
            .column(Column::initial(55.0).at_least(viable_min_column_width))   // ROI / AROI
            .column(Column::initial(55.0).at_least(viable_min_column_width))   // Vol / Mom
            .column(Column::initial(55.0).at_least(viable_min_column_width))   // Time / Ops
            .column(Column::initial(55.0).at_least(viable_min_column_width))   // Volume
            .column(Column::initial(55.0).at_least(viable_min_column_width))   // Variant
            // .column(Column::remainder())                    // Variants
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height)
            .header(36.0, |mut header| {
                self.render_tf_table_header(&mut header);
            })
            .body(|mut body| {
                for (i, row) in rows.iter().enumerate() {
                    // Row Height: 40.0 allows 2 lines of text comfortably
                    body.row(40.0, |mut table_row| {
                        self.render_tf_table_row(&mut table_row, row, i);
                    });
                }
            });
            
        // 6. Scroll Ack
        // Note: TableBuilder handles scrolling internally differently. 
        // If we need auto-scroll to specific row, TableBuilder has a .scroll_to_row() method,
        // but for now, let's see if the manual logic inside the cells still works (it usually does).
        if let Some(target) = &self.scroll_to_pair {
             // We clear it here assuming the row logic caught it. 
             // If scrolling stops working, we will hook into TableBuilder's scroll API later.
             self.scroll_to_pair = None;
        }
    }


        /// Renders the complex "Dual Sort" headers inside the Table
    fn render_tf_table_header(&mut self, header: &mut TableRow) {
        
        let mut header_stack = |ui: &mut Ui, col_top: SortColumn, txt_top: &str, col_bot: Option<(SortColumn, &str)>| {
            ui.vertical(|ui| {
                // Top Item
                self.render_stable_sort_label(ui, col_top, txt_top);
                // Bottom Item
                if let Some((c, t)) = col_bot {
                    self.render_stable_sort_label(ui, c, t);
                }
            });
        };

        // Col 1: Pair
        header.col(|ui| { 
            header_stack(ui, SortColumn::PairName, &UI_TEXT.label_pair, None); 
        });

        // Col 2: Return
        header.col(|ui| { 
            header_stack(ui, SortColumn::LiveRoi, &UI_TEXT.label_roi, Some((SortColumn::AnnualizedRoi, &UI_TEXT.label_aroi))); 
        });

        // Col 3: Market
        header.col(|ui| { 
            header_stack(ui, SortColumn::Volatility, &UI_TEXT.label_volatility_short, Some((SortColumn::Momentum, &UI_TEXT.label_momentum_short))); 
        });

        // Col 4: Time
        header.col(|ui| { 
            header_stack(ui, SortColumn::AvgDuration, &UI_TEXT.tf_time, Some((SortColumn::OpportunityCount, &UI_TEXT.label_opps_short))); 
        });

        // Col 5: Volume
        header.col(|ui| { 
            header_stack(ui, SortColumn::QuoteVolume24h, &UI_TEXT.label_volume_24h, None); 
        });

        // Col 6: Risk
        header.col(|ui| { 
            header_stack(ui, SortColumn::VariantCount, &UI_TEXT.label_stop_loss_short, None); 
        });
    }

    /// Renders the data cells for a single row
    fn render_tf_table_row(&mut self, table_row: &mut TableRow, row: &TradeFinderRow, _index: usize) {
        
        // Selection Logic
        let is_selected = self.selected_pair.as_deref() == Some(&row.pair_name)
            && match (&self.selected_opportunity, &row.opportunity) {
                (Some(sel), Some(live_op)) => sel.id == live_op.opportunity.id,
                (None, None) => true,
                _ => false,
            };

        // --- ROW HIGHLIGHT ---
        // TableBuilder doesn't support "Selected Row Background" natively on top of striping easily.
        // We paint it manually on the row rect if selected.
        if is_selected {
            // We need to access the row rect. 
            // Limitation: 'table_row' doesn't expose the full rect easily until cols are added.
            // Alternative: We highlight the cells or the Pair Name button specifically.
            // Let's rely on the Pair Name Button highlight for now, it's consistent with your style.
        }

        // --- COL 1: PAIR + DIRECTION ---
        table_row.col(|ui| {
            ui.horizontal(|ui| {
                // A. Interactive Pair Name
                let response = ui.interactive_label(
                    &row.pair_name,
                    is_selected,
                    PLOT_CONFIG.color_text_primary,
                    FontId::proportional(14.0),
                );

                if response.clicked() {
                    self.handle_pair_selection(row.pair_name.clone());
                    if let Some(live_op) = &row.opportunity {
                        self.selected_opportunity = Some(live_op.opportunity.clone());
                    } else {
                        self.selected_opportunity = None;
                    }
                }
                
                // Scroll Hook
                if let Some(target) = &self.scroll_to_pair {
                    if target == &row.pair_name {
                        response.scroll_to_me(Some(Align::Center));
                    }
                }

                // B. Direction Arrow
                if let Some(live_op) = &row.opportunity {
                    let op = &live_op.opportunity;
                    let dir_color = op.direction.color();
                    let arrow = match op.direction {
                        TradeDirection::Long => &UI_TEXT.icon_long,
                        TradeDirection::Short => &UI_TEXT.icon_short,
                    };
                    ui.label(RichText::new(arrow).color(dir_color));
                }
            });
        });

        // --- COL 2: RETURN ---
        table_row.col(|ui| {
            if let Some(live_op) = &row.opportunity {
                let roi_color = get_outcome_color(live_op.live_roi);
                ui.vertical(|ui| {
                    ui.label(RichText::new(format!("{:+.2}%", live_op.live_roi)).small().color(roi_color));
                    ui.label(RichText::new(format!("{:+.0}%", live_op.annualized_roi)).small().color(roi_color.linear_multiply(0.7)));
                });
            } else {self.display_no_data(ui); }
        });

        // --- COL 3: MARKET ---
        table_row.col(|ui| {
            if let Some(ms) = &row.market_state {
                ui.vertical(|ui| {
                    ui.label(RichText::new(format!("{:.3}%", ms.volatility_pct * 100.0)).small().color(PLOT_CONFIG.color_info));
                    let mom_color = get_outcome_color(ms.momentum_pct);
                    ui.label(RichText::new(format!("{:+.2}%", ms.momentum_pct * 100.0)).small().color(mom_color));
                });
            } else {self.display_no_data(ui); }
        });

        // --- COL 4: TIME / OPS ---
        table_row.col(|ui| {
            ui.vertical(|ui| {
                if let Some(live_op) = &row.opportunity {
                    let time_str = TimeUtils::format_duration(live_op.opportunity.avg_duration_ms);
                    ui.label(RichText::new(time_str).small().color(PLOT_CONFIG.color_text_neutral));
            } else {self.display_no_data(ui); }

                if row.opportunity_count_total > 1 {
                     ui.label(RichText::new(format!("{} Opps", row.opportunity_count_total)).small().color(PLOT_CONFIG.color_text_subdued));
            } else {self.display_no_data(ui); }
            });
        });

        // --- COL 5: VOLUME ---
        table_row.col(|ui| {
             let val_str = format_volume_compact(row.quote_volume_24h);
             ui.label(RichText::new(val_str).small().color(PLOT_CONFIG.color_text_subdued));
        });

        // --- COL 6: VARIANTS ---
        table_row.col(|ui| {
            if let Some(live_op) = &row.opportunity {
                let op = &live_op.opportunity;
                     self.render_card_variants(ui, op);
            } else {self.display_no_data(ui); }
        });
    }


    /// Helper: Fetches all rows from engine and applies Scope, Direction, and MWT filters.
    /// Crucially, it ensures the SELECTED PAIR is never filtered out.
    fn get_filtered_rows(&self) -> Vec<TradeFinderRow> {
        let mut rows = if let Some(eng) = &self.engine {
            eng.get_trade_finder_rows(Some(&self.simulated_prices))
        } else {
            vec![]
        };

        // Short path reference
        let profile = &crate::config::ANALYSIS.journey.profile;

        // 1. MWT Demotion (Quality Control)
        // If a trade is "Trash", downgrade the row to "No Opportunity".
        for row in &mut rows {
            // EXEMPTION: Selected Pair always shows its trade (even if trash) so you can see why.
            if self.selected_pair.as_deref() == Some(&row.pair_name) {
                continue;
            }

            if let Some(op) = &row.opportunity {
                if !op.opportunity.is_worthwhile(profile) {
                    row.opportunity = None; // Demote to "No Setup"
                }
            }
        }

        // 2. Scope Filter (Base Asset)
        if self.tf_filter_pair_only {
            let base = self.selected_pair.as_ref()
                .and_then(|p| crate::domain::pair_interval::PairInterval::get_base(p))
                .unwrap_or("");
            
            if !base.is_empty() { 
                rows.retain(|r| {
                    // Always keep selected pair, or matches base
                    self.selected_pair.as_deref() == Some(&r.pair_name) || r.pair_name.starts_with(base)
                }); 
            }
        }

        // 3. Direction Filter
        match self.tf_filter_direction {
            DirectionFilter::All => {
                // Show EVERYTHING: Good Trades + Pairs with No Trades (Market State)
            },
            _ => {
                // STRICT MODE: Only show matching trades (or the selected pair)
                rows.retain(|r| {
                    // Golden Rule: Always keep the selected pair visible
                    if self.selected_pair.as_deref() == Some(&r.pair_name) { return true; }

                    if let Some(op) = &r.opportunity {
                        match self.tf_filter_direction {
                            DirectionFilter::Long => op.opportunity.direction == TradeDirection::Long,
                            DirectionFilter::Short => op.opportunity.direction == TradeDirection::Short,
                            _ => true
                        }
                    } else {
                        // Hide "No Setup" rows when filtering for specific direction
                        false 
                    }
                });
            }
        }
        
        rows
    }


    /// Helper: Heals stale selection when zones change or trades disappear
    fn auto_heal_selection(&mut self, rows: &[TradeFinderRow]) {
        // 1. Do we have a selected pair?
        if let Some(pair) = &self.selected_pair {
            // 2. Is this pair still visible in the list?
            // (Note: rows can contain "No Op" rows, which is good)
            let pair_rows: Vec<&TradeFinderRow> =
                rows.iter().filter(|r| r.pair_name == *pair).collect();
            let pair_is_visible = !pair_rows.is_empty();

            if pair_is_visible {
                // CASE A: Pair is visible. We generally trust the current state.

                if let Some(current_sel) = &self.selected_opportunity {
                    // We have a specific Op selected. Verify it still exists.
                    let exists = pair_rows.iter().any(|r| {
                        r.opportunity
                            .as_ref()
                            .map_or(false, |op| op.opportunity.id == current_sel.id)
                    });

                    if !exists {
                        // The specific Op died (e.g. ROI dropped below MWT, or Zones shifted).
                        // Try to find the NEW best op for this same pair to keep context.
                        if let Some(best_row) = self.find_best_row_for_pair(&pair_rows) {
                            if let Some(op) = &best_row.opportunity {
                                log::error!(
                                    "UI AUTO-HEAL: Swapping stale Op for {}. New ID: {}",
                                    pair,
                                    op.opportunity.id
                                );
                                self.selected_opportunity = Some(op.opportunity.clone());
                            }
                        } else {
                            // No valid trades left for this pair.
                            // Drop to "Market View" (None).
                            log::error!(
                                "UI AUTO-HEAL: Op died for {}. Dropping to Market View.",
                                pair
                            );
                            self.selected_opportunity = None;
                        }
                    }
                }
                // Else: selected_opportunity is None.
                // Since the pair is visible, "None" is a valid state (Market View).
                // DO NOT AUTO-HUNT. This fixes the "Flash" bug.
            } else {
                // CASE B: Selected Pair is GONE (Filtered out entirely).
                // Now we must hunt for a new target from the top of the list.
                if let Some(best_row) = rows.first() {
                    log::error!(
                        "UI HUNTER: Selection {} hidden by filter. Snapping to top: {}",
                        pair,
                        best_row.pair_name
                    );

                    self.handle_pair_selection(best_row.pair_name.clone());
                    // If the top row has an op, select it.
                    if let Some(live_op) = &best_row.opportunity {
                        self.selected_opportunity = Some(live_op.opportunity.clone());
                    } else {
                        self.selected_opportunity = None;
                    }
                } else {
                    // List is empty. Clear selection.
                    self.selected_opportunity = None;
                }
            }
        } else {
            // No pair selected at all? Snap to top if possible.
            if let Some(best_row) = rows.first() {
                self.handle_pair_selection(best_row.pair_name.clone());
                if let Some(live_op) = &best_row.opportunity {
                    self.selected_opportunity = Some(live_op.opportunity.clone());
                }
            }
        }
    }

    /// Helper to find the best row (highest ROI) from a slice of rows.
    fn find_best_row_for_pair<'a>(
        &self,
        rows: &'a [&TradeFinderRow],
    ) -> Option<&'a TradeFinderRow> {
        rows.iter()
            .filter(|r| r.opportunity.is_some())
            .max_by(|a, b| {
                let roi_a = a.opportunity.as_ref().unwrap().live_roi;
                let roi_b = b.opportunity.as_ref().unwrap().live_roi;
                roi_a.total_cmp(&roi_b)
            })
            .map(|r| *r) // FIX: Dereference the double-reference (&&T -> &T)
    }

    /// Renders a single sortable label using the Interactive Button style
    fn render_stable_sort_label(&mut self, ui: &mut Ui, col: SortColumn, text: &str) {
        let is_active = self.tf_sort_col == col;
        let color = if is_active {
            PLOT_CONFIG.color_text_primary
        } else {
            PLOT_CONFIG.color_text_subdued
        };

        // STABILITY: Always include space for the icon.
        let suffix = if is_active {
            match self.tf_sort_dir {
                SortDirection::Ascending => &UI_TEXT.icon_sort_asc,
                SortDirection::Descending => &UI_TEXT.icon_sort_desc,
            }
        } else {
            "  " // Spacer
        };

        let label_text = format!("{} {}", text, suffix);

        // FIX: Pass FontId::proportional(14.0) for Headers
        if ui
            .interactive_label(&label_text, is_active, color, FontId::proportional(14.0))
            .clicked()
        {
            if is_active {
                self.tf_sort_dir = self.tf_sort_dir.toggle();
            } else {
                self.tf_sort_col = col;
                // Intelligent Defaults
                self.tf_sort_dir = match col {
                    SortColumn::PairName | SortColumn::AvgDuration => SortDirection::Ascending,
                    _ => SortDirection::Descending,
                };
            }
        }
    }

    pub(super) fn render_left_panel(&mut self, ctx: &Context) {
        let frame = UI_CONFIG.side_panel_frame();

        SidePanel::left("left_panel")
            .min_width(280.0) // I believe this is irrelevant because items we draw inside have higher total min_width
            .resizable(true)
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
                                let new_decay =
                                    AdaptiveParameters::calculate_time_decay(new_threshold);

                                // Apply only if changed (prevents log spam if dragging within same band)
                                if (self.app_config.time_decay_factor - new_decay).abs()
                                    > f64::EPSILON
                                {
                                    self.app_config.time_decay_factor = new_decay;
                                }
                                self.auto_scale_y = true;
                                // NEW: If we change PH, the list might shuffle. Ensure we keep our selection in view.
                                if let Some(p) = &self.selected_pair {
                                    self.scroll_to_pair = Some(p.clone());
                                }
                                self.invalidate_all_pairs_for_global_change(
                                    "price horizon threshold changed",
                                );
                            }
                        }
                    }
                }

                ui.add_space(10.0);
                ui.separator();
                self.render_trade_finder_content(ui);
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
                    ui.label(
                        RichText::new(&UI_TEXT.tb_time)
                            .size(16.0)
                            .color(PLOT_CONFIG.color_text_neutral),
                    );

                    for res in CandleResolution::iter() {
                        ui.selectable_value(&mut self.candle_resolution, res, res.to_string());
                    }

                    ui.separator();

                    // 2. LAYER VISIBILITY
                    ui.checkbox(&mut self.plot_visibility.sticky, &UI_TEXT.tb_sticky);
                    ui.checkbox(&mut self.plot_visibility.low_wicks, &UI_TEXT.tb_low_wicks);
                    ui.checkbox(&mut self.plot_visibility.high_wicks, &UI_TEXT.tb_high_wicks);
                    ui.checkbox(
                        &mut self.plot_visibility.background,
                        &UI_TEXT.tb_volume_hist,
                    );
                    ui.checkbox(&mut self.plot_visibility.candles, &UI_TEXT.tb_candles);

                    ui.separator();

                    // CONTEXT
                    ui.checkbox(&mut self.plot_visibility.separators, &UI_TEXT.tb_gaps);
                    ui.checkbox(
                        &mut self.plot_visibility.horizon_lines,
                        &UI_TEXT.tb_price_limits,
                    );
                    ui.checkbox(&mut self.plot_visibility.price_line, &UI_TEXT.tb_live_price);
                    ui.checkbox(&mut self.plot_visibility.opportunities, &UI_TEXT.tb_targets);

                    // STATUS INDICATOR (TEMP but very useful)
                    if self.auto_scale_y {
                        ui.label(
                            RichText::new(&UI_TEXT.tb_y_locked)
                                .small()
                                .color(PLOT_CONFIG.color_profit),
                        );
                    } else {
                        ui.label(
                            RichText::new(&UI_TEXT.tb_y_unlocked)
                                .small()
                                .color(PLOT_CONFIG.color_warning),
                        );
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

    fn render_card_variants(&mut self, ui: &mut Ui, op: &TradeOpportunity) {
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            let label_text = format!("{} {} ▾", op.variant_count(), UI_TEXT.label_sl_variants);
            let id_source = format!("var_menu_{}_{}", op.pair_name, op.target_zone_id);

            // CALL THE HELPER
            ui.custom_dropdown(&id_source, &label_text, |ui| {
                // --- CONTENT LOGIC ---
                let active_stop_price = if let Some(sel) = &self.selected_opportunity {
                    if sel.pair_name == op.pair_name && sel.target_zone_id == op.target_zone_id {
                        sel.stop_price
                    } else {
                        op.stop_price
                    }
                } else {
                    op.stop_price
                };

                let mut should_close = false;

                for variant in &op.variants {
                    let risk_pct = calculate_percent_diff(variant.stop_price, op.start_price);
                    let win_rate = variant.simulation.success_rate * 100.0;

                    let text = format!(
                        "ROI {:+.2}%   Win {:.0}%   SL -{:.2}%",
                        variant.roi, win_rate, risk_pct
                    );

                    let is_current = (variant.stop_price - active_stop_price).abs() < f64::EPSILON;

                    if ui.selectable_label(is_current, text).clicked() {
                        self.handle_pair_selection(op.pair_name.clone());
                        self.scroll_to_pair = Some(op.pair_name.clone());

                        let mut new_selected = op.clone();
                        new_selected.stop_price = variant.stop_price;
                        new_selected.simulation = variant.simulation.clone();

                        self.selected_opportunity = Some(new_selected);

                        should_close = true; // Signal the helper to close
                    }
                }

                should_close
            });
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
                ui.label(
                    RichText::new(format!("{}", self.sim_direction))
                        .small()
                        .color(PLOT_CONFIG.color_info),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!("{}: {}", UI_TEXT.sim_step, self.sim_step_size))
                        .small()
                        .color(PLOT_CONFIG.color_profit),
                );
                ui.separator();
                if let Some(sim_price) = self.simulated_prices.get(pair) {
                    ui.label(
                        RichText::new(format!("{} {}", UI_TEXT.sp_price, format_price(*sim_price)))
                            .strong()
                            .color(PLOT_CONFIG.color_short),
                    );
                }
            } else {
                // Live Mode
                ui.label(
                    RichText::new(format!("{} ", &UI_TEXT.sp_live_mode))
                        .small()
                        .color(PLOT_CONFIG.color_profit),
                );
                ui.separator();

                if let Some(price) = self.get_display_price(pair) {
                    ui.label(
                        RichText::new(format!("{} {}", UI_TEXT.sp_price, format_price(price)))
                            .strong()
                            .color(PLOT_CONFIG.color_text_primary),
                    );
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
                    let zone_size = (cva.price_range.end_range - cva.price_range.start_range)
                        / cva.zone_count as f64;
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
                        if pct > 30.0 {
                            PLOT_CONFIG.color_loss
                        }
                        // Red (Too Cluttered)
                        else if pct < 5.0 {
                            PLOT_CONFIG.color_warning
                        }
                        // Yellow (Too sparse)
                        else {
                            PLOT_CONFIG.color_profit
                        } // Green (Good)
                    };
                    ui.label_subdued(&UI_TEXT.sp_coverage);
                    ui.metric(
                        &UI_TEXT.sp_coverage_sticky,
                        &format!("{:.1}%", model.coverage.sticky_pct),
                        cov_color(model.coverage.sticky_pct),
                    );
                    ui.metric(
                        &UI_TEXT.sp_coverage_support,
                        &format!("{:.1}%", model.coverage.support_pct),
                        cov_color(model.coverage.support_pct),
                    );
                    ui.metric(
                        &UI_TEXT.sp_coverage_resistance,
                        &format!("{:.1}%", model.coverage.resistance_pct),
                        cov_color(model.coverage.resistance_pct),
                    );
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
                    let pct = if total > 0 {
                        (relevant as f64 / total as f64) * 100.0
                    } else {
                        0.0
                    };

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
                ui.label(
                    RichText::new(format!("{} {}", UI_TEXT.label_working, msg))
                        .small()
                        .color(PLOT_CONFIG.color_short),
                );
            }

            let q_len = engine.get_queue_len();
            if q_len > 0 {
                ui.separator();
                ui.label(
                    RichText::new(format!("{}: {}", UI_TEXT.label_queue, q_len))
                        .small()
                        .color(PLOT_CONFIG.color_warning),
                );
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
        let (_, profile, actual_count) = if let Some(engine) = &self.engine {
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
            // self.selected_pair.clone(),
            // available_pairs,
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
