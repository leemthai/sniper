use chrono::Duration;

use eframe::egui::{
    Align, CentralPanel, Color32, ComboBox, Context, FontId, Frame, Grid, Layout, Order, RichText,
    Sense, SidePanel, TopBottomPanel, Ui, Window,
};

use egui_extras::{Column, TableBuilder, TableRow};

use std::cmp::Ordering;
use std::collections::HashMap;
use strum::IntoEnumIterator;

use crate::config::plot::PLOT_CONFIG;
use crate::config::{MomentumPct, OptimizationStrategy, TICKER, VolatilityPct, constants, Pct};

#[cfg(debug_assertions)]
use crate::config::DF;

use crate::config::{CandleResolution, PriceLike};

use crate::domain::pair_interval::PairInterval;

use crate::engine::messages::JobMode;

use crate::models::cva::ScoreType;
use crate::models::trading_view::{
    NavigationTarget, SortColumn, SortDirection, TradeDirection, TradeFinderRow, TradeOpportunity,
};

use crate::ui::app::ScrollBehavior;
use crate::ui::config::{UI_CONFIG, UI_TEXT};
use crate::ui::styles::{DirectionColor, UiStyleExt, get_momentum_color, get_outcome_color};
use crate::ui::ui_panels::CandleRangePanel;
use crate::ui::ui_plot_view::PlotInteraction;
use crate::ui::{time_tuner, time_tuner::TunerAction};

use crate::utils::TimeUtils;

use super::app::ZoneSniperApp;

const CELL_PADDING_Y: f32 = 4.0;

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

                // NEW: Target Price Sort
                SortColumn::TargetPrice => {
                    let val_a = a
                        .opportunity
                        .as_ref()
                        .map(|o| o.target_price)
                        .unwrap_or_default();
                    let val_b = b
                        .opportunity
                        .as_ref()
                        .map(|o| o.target_price)
                        .unwrap_or_default();
                    val_a
                        .value()
                        .total_cmp(&val_b.value())
                        .then_with(|| a.pair_name.cmp(&b.pair_name))
                }

                SortColumn::QuoteVolume24h => a
                    .quote_volume_24h
                    .value()
                    .total_cmp(&b.quote_volume_24h.value())
                    .then_with(|| a.pair_name.cmp(&b.pair_name)),

                SortColumn::Volatility => {
                    let va = a
                        .market_state
                        .as_ref()
                        .map(|m| m.volatility_pct)
                        .unwrap_or(VolatilityPct::new(0.0));
                    let vb = b
                        .market_state
                        .as_ref()
                        .map(|m| m.volatility_pct)
                        .unwrap_or(VolatilityPct::new(0.0));
                    va.value()
                        .total_cmp(&vb.value())
                        .then_with(|| a.pair_name.cmp(&b.pair_name))
                }
                SortColumn::Momentum => {
                    let ma = a
                        .market_state
                        .as_ref()
                        .map(|m| m.momentum_pct)
                        .unwrap_or(MomentumPct::new(0.0));
                    let mb = b
                        .market_state
                        .as_ref()
                        .map(|m| m.momentum_pct)
                        .unwrap_or(MomentumPct::new(0.0));
                    ma.value()
                        .total_cmp(&mb.value())
                        .then_with(|| a.pair_name.cmp(&b.pair_name))
                }

                SortColumn::LiveRoi => {
                    let val_a = a
                        .opportunity
                        .as_ref()
                        .map(|o| o.live_roi(a.current_price).value())
                        .unwrap_or(f64::NEG_INFINITY);
                    let val_b = b
                        .opportunity
                        .as_ref()
                        .map(|o| o.live_roi(b.current_price).value())
                        .unwrap_or(f64::NEG_INFINITY);
                    val_a
                        .total_cmp(&val_b)
                        .then_with(|| a.pair_name.cmp(&b.pair_name))
                }
                SortColumn::AnnualizedRoi => {
                    let val_a = a
                        .opportunity
                        .as_ref()
                        .map(|o| o.live_annualized_roi(a.current_price).value())
                        .unwrap_or(f64::NEG_INFINITY);
                    let val_b = b
                        .opportunity
                        .as_ref()
                        .map(|o| o.live_annualized_roi(b.current_price).value())
                        .unwrap_or(f64::NEG_INFINITY);
                    val_a
                        .total_cmp(&val_b)
                        .then_with(|| a.pair_name.cmp(&b.pair_name))
                }
                SortColumn::AvgDuration => {
                    let val_a = a
                        .opportunity
                        .as_ref()
                        .map(|o| o.avg_duration_ms.value())
                        .unwrap_or(i64::MAX);
                    let val_b = b
                        .opportunity
                        .as_ref()
                        .map(|o| o.avg_duration_ms.value())
                        .unwrap_or(i64::MAX);
                    val_b
                        .cmp(&val_a)
                        .then_with(|| a.pair_name.cmp(&b.pair_name))
                }
                // Sort by Strategy Score (Balanced/ROI/AROI)
                SortColumn::Score => {
                    let val_a = a
                        .opportunity
                        .as_ref()
                        .map(|o| o.calculate_quality_score())
                        .unwrap_or(f64::NEG_INFINITY);
                    let val_b = b
                        .opportunity
                        .as_ref()
                        .map(|o| o.calculate_quality_score())
                        .unwrap_or(f64::NEG_INFINITY);

                    val_a
                        .total_cmp(&val_b)
                        .then_with(|| a.pair_name.cmp(&b.pair_name))
                }
                SortColumn::VariantCount => {
                    let va = a
                        .opportunity
                        .as_ref()
                        .map(|o| o.variant_count())
                        .unwrap_or(0);
                    let vb = b
                        .opportunity
                        .as_ref()
                        .map(|o| o.variant_count())
                        .unwrap_or(0);
                    va.cmp(&vb).then_with(|| a.pair_name.cmp(&b.pair_name))
                }
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

        // 2. Get Data (Directly from Selected Opportunity)
        let Some(op) = self.selected_opportunity.clone() else {
            return;
        };

        // Get live context
        let current_price = self.get_display_price(&op.pair_name).unwrap_or_default();
        let calc_price = if current_price.is_positive() {
            current_price
        } else {
            op.start_price
        };

        // 4. Render Window
        Window::new(format!("Opportunity Explainer: {}", op.pair_name))
            .collapsible(false)
            .resizable(false)
            .order(Order::Tooltip)
            .open(&mut self.show_opportunity_details)
            .default_width(600.)
            .show(ctx, |ui| {
                let sim = &op.simulation;
                let state = &op.market_state;
                ui.heading(format!(
                    "{}: {}",
                    UI_TEXT.opp_exp_current_opp, op.target_price
                ));

                // "Setup Type: LONG" (with encoded color)
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label_subdued(format!("{}", UI_TEXT.opp_exp_setup_type));
                    ui.label(
                        RichText::new(op.direction.to_string().to_uppercase())
                            .strong()
                            .color(op.direction.color()),
                    );
                });
                ui.separator();

                // --- CALCULATIONS ---
                // We use the data captured in the opportunity, not the global config
                let ph_pct = op.source_ph_pct;
                let max_time_ms = op.max_duration_ms;
                let max_time_str = TimeUtils::format_duration(max_time_ms.value());

                // For interval display, we use the global config as a fallback if not in state
                let interval_ms = constants::BASE_INTERVAL.as_millis() as i64;
                let max_candles = if interval_ms > 0 {
                    max_time_ms.value() / interval_ms
                } else {
                    0
                };

                // SECTION 1: THE MATH
                ui.label_subheader(&UI_TEXT.opp_exp_expectancy);
                let roi_pct = op.live_roi(calc_price);
                let aroi_pct = op.live_annualized_roi(calc_price);
                let roi_color = get_outcome_color(roi_pct.value());

                ui.metric(
                    &format!("{}", UI_TEXT.label_roi),
                    &format!("{}", roi_pct),
                    roi_color,
                );
                ui.metric(
                    &UI_TEXT.label_aroi_long,
                    &format!("{}", aroi_pct),
                    roi_color,
                );

                ui.metric(
                    &UI_TEXT.label_avg_duration,
                    &TimeUtils::format_duration(op.avg_duration_ms.value()),
                    PLOT_CONFIG.color_text_neutral,
                );

                ui.metric(
                    &UI_TEXT.label_success_rate,
                    &format!("{}", sim.success_rate),
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

                // Volatility
                ui.metric(
                    &UI_TEXT.label_volatility,
                    &format!("{}", state.volatility_pct),
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
                        RichText::new(format!("{}", state.momentum_pct))
                            .small()
                            .color(get_momentum_color(state.momentum_pct.value())),
                    );

                    ui.label(
                        RichText::new(format!(" ({} {})", UI_TEXT.opp_exp_trend_length, ph_pct,))
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
                    let vol_color = if state.relative_volume.value() > 1.0 {
                        PLOT_CONFIG.color_warning
                    } else {
                        PLOT_CONFIG.color_text_subdued
                    };
                    ui.label(
                        RichText::new(format!("{}", state.relative_volume))
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

                // TRADE SETUP
                ui.label_subheader(&UI_TEXT.opp_exp_trade_setup);

                // Entry / Target can stay standard
                ui.metric(
                    &UI_TEXT.opp_exp_trade_entry,
                    &format!("{}", calc_price),
                    PLOT_CONFIG.color_text_neutral,
                );
                let target_dist = Pct::new(op.target_price.percent_diff_from_0_1(&calc_price));
                let stop_dist = Pct::new(op.stop_price.percent_diff_from_0_1(&calc_price));

                // TARGET ROW
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(
                        RichText::new(UI_TEXT.label_target_text.to_string() + ":")
                            .small()
                            .color(PLOT_CONFIG.color_text_subdued),
                    );
                    ui.label(
                        RichText::new(format!("{}", op.target_price))
                            .small()
                            .color(PLOT_CONFIG.color_profit),
                    );
                    ui.label(
                        RichText::new(format!("({})", target_dist))
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
                        RichText::new(format!("{}", op.stop_price))
                            .small()
                            .color(PLOT_CONFIG.color_stop_loss),
                    );
                    ui.label(
                        RichText::new(format!(
                            "({} {} / {} {})",
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

                // THE STORY
                ui.label_subheader(&UI_TEXT.opp_exp_how_this_works);
                ui.vertical(|ui| {
                    ui.style_mut().spacing.item_spacing.y = 4.0;
                    let story_color = PLOT_CONFIG.color_text_neutral;

                    ui.label(
                        RichText::new(format!(
                            "{} ({} = {}, {} = {}, {} = {})",
                            UI_TEXT.opp_expr_we_fingerprinted,
                            UI_TEXT.label_volatility,
                            state.volatility_pct,
                            UI_TEXT.label_momentum,
                            state.momentum_pct,
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
                        (sim.success_rate.value() * sim.sample_size as f64).round() as usize;

                    ui.label(
                        RichText::new(format!(
                            "{} {} {} {} {} {} {} {} {} {}",
                            UI_TEXT.opp_exp_cases_one,
                            win_count,
                            UI_TEXT.opp_exp_cases_two,
                            sim.sample_size,
                            UI_TEXT.opp_exp_cases_three,
                            UI_TEXT.label_target_text,
                            UI_TEXT.opp_exp_cases_four,
                            sim.success_rate,
                            UI_TEXT.label_success_rate,
                            UI_TEXT.opp_exp_cases_five,
                        ))
                        .small()
                        .color(story_color)
                        .italics(),
                    );
                });
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
    fn render_trade_finder_filters(&mut self, ui: &mut Ui, count: usize) -> bool {
        let mut filter_changed = false;
        ui.add_space(10.0);

        // --- 2. SCOPE TOGGLE ---
        ui.horizontal(|ui| {
            // Item count
            ui.label(
                RichText::new(format!(
                    "{} {}",
                    count,
                    UI_TEXT.label_targets_text.to_lowercase()
                ))
                .strong()
                .color(PLOT_CONFIG.color_text_subdued),
            );

            // Locate Button (Center on Target)
            // Only show if we actually have a target to scroll to
            if self.selected_pair.is_some() {
                ui.add_space(5.0);
                if ui
                    .small_button(
                        RichText::new(UI_TEXT.label_recenter.as_str())
                            .color(PLOT_CONFIG.color_info),
                    )
                    .on_hover_text(&UI_TEXT.hover_scroll_to_selected_target)
                    .clicked()
                {
                    self.update_scroll_to_selection();
                }
            }

            ui.separator();

            // "ALL PAIRS" Button
            if ui
                .selectable_label(!self.tf_scope_match_base, &UI_TEXT.tf_scope_all)
                .clicked()
            {
                self.tf_scope_match_base = false;
                filter_changed = true;
                self.update_scroll_to_selection();
            }

            // "[BASE ASSET] ONLY"
            let base_asset = self
                .selected_pair
                .as_ref()
                .and_then(|p| PairInterval::get_base(p))
                .unwrap_or("SELECTED");

            let pair_label = format!("{} {}", base_asset, UI_TEXT.tf_scope_selected);
            if ui
                .selectable_label(self.tf_scope_match_base, pair_label)
                .clicked()
            {
                self.tf_scope_match_base = true;
                filter_changed = true;
                self.update_scroll_to_selection();
            }
            ui.add_space(10.0);
        });
        ui.separator();

        filter_changed
    }

    /// Helper for Empty Cells (SSOT)
    fn display_no_data(&self, ui: &mut Ui) {
        ui.label("-");
    }

    fn down_from_top(&self, ui: &mut Ui) {
        ui.add_space(CELL_PADDING_Y);
    }

    fn render_strategy_header_grid(&mut self, ui: &mut Ui, sort_changed: &mut bool) {
        let goals: Vec<_> = OptimizationStrategy::iter().collect();
        let count = goals.len();

        // Helper closure to render a specific goal by index
        let mut render_btn = |ui: &mut Ui, idx: usize| {
            if let Some(goal) = goals.get(idx) {
                let col = match goal {
                    &OptimizationStrategy::MaxROI => SortColumn::LiveRoi,
                    &OptimizationStrategy::MaxAROI => SortColumn::AnnualizedRoi,
                    &OptimizationStrategy::Balanced => SortColumn::Score,
                };
                if self.render_sort_icon_button(ui, col, &goal.icon()) {
                    *sort_changed = true;
                }
            }
        };

        ui.vertical_centered(|ui| {
            ui.add_space(2.0);

            if count == 1 {
                // Case 1: Single Center
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    render_btn(ui, 0);
                });
            } else if count == 2 {
                // Case 2: Split Left/Right
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0; // Gap
                    render_btn(ui, 0);
                    render_btn(ui, 1);
                });
            } else if count == 3 {
                // Case 3: Top Center, Bottom Split
                // Top Row
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    render_btn(ui, 0);
                });

                // Bottom Row
                ui.horizontal(|ui| {
                    // Manual padding to center the pair or split them
                    // Since column is 70px wide, and icons are ~20px, we have room.
                    ui.spacing_mut().item_spacing.x = 12.0;
                    render_btn(ui, 1);
                    render_btn(ui, 2);
                });
            } else if count >= 4 {
                // Case 4: 2x2 Grid
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    render_btn(ui, 0);
                    render_btn(ui, 1);
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    render_btn(ui, 2);
                    render_btn(ui, 3);
                });
            }
        });
    }

    fn render_trade_finder_content(&mut self, ui: &mut Ui) {
        let mut rows = self.get_filtered_rows();

        let filter_changed = self.render_trade_finder_filters(ui, rows.len());

        // Simple Validity Check (The Anti-Healer)
        // If the specific selected trade ID is no longer in the list (filtered out or expired),
        // we drop to Market View (None). We do NOT hunt for a replacement.
        if let Some(sel) = &self.selected_opportunity {
            let exists = rows
                .iter()
                .any(|r| r.opportunity.as_ref().map_or(false, |op| op.id == sel.id));
            if !exists {
                #[cfg(debug_assertions)]
                if DF.log_selected_opportunity {
                    log::info!(
                        "ANTI-HEALER: SELECTED OPPORTUNITY CLEARed in render_trade_finder_content because apparently no longer exists. THIS NEEDS INVESTIGATING TO FIND OUT WHY."
                    );
                }
                self.selected_opportunity = None;
            }
        } else {
            #[cfg(debug_assertions)]
            if DF.log_selected_opportunity {
                log::info!(
                    "ANTI-HEALER: FAILED TO FIND the currently selected OPPPORTUNITY {:?} in TF because it is blank. Weird?",
                    &self.selected_opportunity
                );
            }
        }

        // Sort
        self.sort_trade_finder_rows(&mut rows);
        // Empty table check
        if rows.is_empty() {
            ui.centered_and_justified(|ui| ui.label("Loading Market Data..."));
            return;
        }

        // --- SCROLL INDEX LOGIC (PRECISE) ---
        let mut target_index = None;

        if let Some(target) = &self.scroll_target {
            target_index = rows.iter().position(|r| {
                match target {
                    // Case A: Hunting a specific Trade (UUID)
                    NavigationTarget::Opportunity(id) => {
                        // log::info!("render_trade_finder_content() case A hunting id: {} ", id);
                        r.opportunity.as_ref().map_or(false, |op| op.id == *id)
                    }
                    // Case B: Hunting a Pair (Market View)
                    NavigationTarget::Pair(name) => {
                        // Only match if row is the pair AND has no op (Market View row)
                        // Let's stick to "First instance of pair" for fallback.
                        r.pair_name == *name
                    }
                }
            });
        }

        let available_height = ui.available_height();

        let mut sort_changed = false;

        ui.scope(|ui| {
            let visuals = ui.visuals_mut();

            // 1. Set "Selected" color
            visuals.selection.bg_fill = PLOT_CONFIG.color_tf_selected;
            // 2. Set "Stripe" color
            visuals.faint_bg_color = Color32::from_white_alpha(15);

            let mut builder = TableBuilder::new(ui)
                .striped(true)
                .resizable(false)
                .sense(Sense::click()) // Enable row clicks
                .cell_layout(Layout::left_to_right(Align::Min))
                .column(Column::exact(140.0).clip(false)) // Pair
                .column(Column::exact(70.0).clip(true)) // ROI/AROI
                .column(Column::exact(55.0).clip(true)) // Vol/Mom
                .column(Column::exact(55.0).clip(true)) // Time/Ops
                .column(Column::exact(55.0).clip(true)) // Volume
                .column(Column::exact(70.0).clip(true)) // Variant
                .min_scrolled_height(0.0)
                .max_scroll_height(available_height);

            // FIX: Apply Scroll at the Builder Level
            // This works even if the row is virtualized (off-screen)
            if let Some(idx) = target_index {
                builder = builder.scroll_to_row(idx, Some(Align::Center));
            }

            builder
                .header(48.0, |mut header| {
                    self.render_tf_table_header(&mut header, &mut sort_changed);
                })
                .body(|mut body| {
                    for (i, row) in rows.iter().enumerate() {
                        body.row(50.0, |mut table_row| {
                            self.render_tf_table_row(&mut table_row, row, i);
                        });
                    }
                });

            // 6. Sticky Scroll Cleanup
            // If Sort OR Filter changed, we DO NOT clear the request.
            // We wait for the NEXT frame where the rows are regenerated/resorted.
            if !sort_changed && !filter_changed {
                if target_index.is_some() {
                    self.scroll_target = None;
                }
            }
        });
    }

    /// Helper to render a stacked header (Primary Sort Top, Optional Secondary Sort Bottom)
    fn render_header_stack(
        &mut self,
        ui: &mut Ui,
        sort_changed: &mut bool,
        col_top: SortColumn,
        txt_top: &str,
        col_bot: Option<(SortColumn, &str)>,
    ) {
        ui.vertical_centered(|ui| {
            // Top Item
            if self.render_stable_sort_label(ui, col_top, txt_top) {
                *sort_changed = true;
            }

            // Bottom Item (Optional)
            if let Some((c, t)) = col_bot {
                if self.render_stable_sort_label(ui, c, t) {
                    *sort_changed = true;
                }
            }
        });
    }

    fn render_tf_table_header(&mut self, header: &mut TableRow, sort_changed: &mut bool) {
        // Helper to render stacked headers

        // Col 1: Pair (Top) / Target (Bottom)
        header.col(|ui| {
            self.render_header_stack(
                ui,
                sort_changed,
                SortColumn::PairName,
                &UI_TEXT.label_pair,
                Some((SortColumn::TargetPrice, &UI_TEXT.label_target)),
            );
        });

        // Col 2: Strategy Metrics (Grid Layout)
        header.col(|ui| {
            self.render_strategy_header_grid(ui, sort_changed);
        });

        // Col 3: Market
        header.col(|ui| {
            self.render_header_stack(
                ui,
                sort_changed,
                SortColumn::Volatility,
                &UI_TEXT.label_volatility_short,
                Some((SortColumn::Momentum, &UI_TEXT.label_momentum_short)),
            );
        });

        // Col 4: Average Duration / Trade Balance Score
        header.col(|ui| {
            self.render_header_stack(
                ui,
                sort_changed,
                SortColumn::AvgDuration,
                &UI_TEXT.tf_time,
                None,
            );
        });

        // Col 5: Volume
        header.col(|ui| {
            self.render_header_stack(
                ui,
                sort_changed,
                SortColumn::QuoteVolume24h,
                &UI_TEXT.label_volume_24h,
                None,
            );
        });

        // Col 6: Risk
        header.col(|ui| {
            self.render_header_stack(
                ui,
                sort_changed,
                SortColumn::VariantCount,
                &UI_TEXT.label_stop_loss_short,
                None,
            );
        });
    }

    /// Renders the data cells for a single row
    fn render_tf_table_row(
        &mut self,
        table_row: &mut TableRow,
        row: &TradeFinderRow,
        index: usize,
    ) {
        // Selection Logic
        let is_selected = self.selected_pair.as_deref() == Some(&row.pair_name)
            && match (&self.selected_opportunity, &row.opportunity) {
                // Comparison: Match IDs directly
                (Some(sel), Some(op)) => sel.id == op.id,
                (None, None) => true,
                _ => false,
            };

        // This paints the background correctly behind the correct row
        table_row.set_selected(is_selected);

        // We pass the scroll tracker as a local bool to the column helper
        self.col_pair_name(table_row, row, index);
        self.col_strategy_metrics(table_row, row);
        self.col_market_state(table_row, row);
        self.col_time(table_row, row);
        self.col_volume_24h(table_row, row);
        self.col_sl_variants(table_row, row);

        // 2. GET RESPONSE (Safe now)
        let response = table_row.response();

        // // 4. INTERACTION
        if response.clicked() {
            if let Some(op) = &row.opportunity {
                self.select_specific_opportunity(
                    op.clone(),
                    ScrollBehavior::None,
                    "clicked in render_tf_table_row",
                );
            } else {
                // This clicked row has no opportunity attached
                self.handle_pair_selection(row.pair_name.clone());
                #[cfg(debug_assertions)]
                log::info!(
                    "SELECTED OPPORTUNITY CLEARing! in render_tf_table_row because this this row has no opportunity i.e. row.opportunity is None"
                );
                // self.selected_opportunity = None;
            }
        }
    }

    /// Column 1: Pair Name + Direction Icon (Static Text) on top line, Age on 2nd line
    fn col_pair_name(
        &self,
        table_row: &mut egui_extras::TableRow,
        row: &TradeFinderRow,
        index: usize,
    ) {
        table_row.col(|ui| {
            ui.vertical(|ui| {
                // LINE 1: Pair Name + Direction Icon
                ui.horizontal(|ui| {
                    ui.style_mut().spacing.item_spacing.x = 4.0;

                    // Row Number
                    ui.label(
                        RichText::new(format!("{}.", index))
                            .size(10.0)
                            .color(PLOT_CONFIG.color_text_subdued),
                    );

                    // Visual Text (Pair Name)
                    ui.label(
                        RichText::new(&row.pair_name)
                            .strong()
                            .size(14.0)
                            .color(PLOT_CONFIG.color_text_primary),
                    );

                    // // 2. PH Badge (Context)
                    // // Logic: Use Trade Source if available, otherwise use User's Current Setting
                    // let ph_val_raw = if let Some(live_op) = &row.opportunity {
                    //     live_op.opportunity.source_ph
                    // } else {
                    //     #[cfg(debug_assertions)]
                    //     0.15
                    //     // // Fallback: Check overrides -> Global Config
                    //     // self.ph_overrides
                    //     //     .get(&row.pair_name)
                    //     //     .map(|c| c.threshold_pct)
                    //     //     .unwrap_or(self.app_config.price_horizon.threshold_pct)
                    // };

                    // let ph_val = ph_val_raw * 100.0;
                    // if let Some(ph_str) = format_fixed_chars(ph_val, 4) {
                    //     ui.label(
                    //         RichText::new(format!("@{}%", ph_str))
                    //             .size(10.0)
                    //             .color(PLOT_CONFIG.color_text_subdued),
                    //     );
                    // }

                    // Direction Icon
                    if let Some(op) = &row.opportunity {
                        // let op = &live_op.opportunity;
                        let dir_color = op.direction.color();
                        let arrow = match op.direction {
                            TradeDirection::Long => &UI_TEXT.icon_long,
                            TradeDirection::Short => &UI_TEXT.icon_short,
                        };

                        // --- STRATEGY ICON ---
                        let strategy_icon = op.strategy.icon();
                        ui.label(
                            RichText::new(strategy_icon)
                                .size(14.0)
                                .color(PLOT_CONFIG.color_text_neutral),
                        );
                        ui.label(RichText::new(arrow).color(dir_color));
                    }
                });

                if let Some(op) = &row.opportunity {
                    ui.horizontal(|ui| {
                        // Left: Target Price
                        // Use truncation/small font to fit
                        ui.label(
                            RichText::new(format!("T: {}", op.target_price))
                                .size(10.0)
                                .color(PLOT_CONFIG.color_info),
                        );

                        // Right: Age (Pushed to edge)
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let now = TimeUtils::now_utc();
                            let age = now - op.created_at;
                            let age_str = if age < Duration::minutes(1) {
                                "New".to_string()
                            } else {
                                TimeUtils::format_duration(age.num_milliseconds())
                            };

                            ui.label(
                                RichText::new(age_str)
                                    .size(10.0)
                                    .color(PLOT_CONFIG.color_text_subdued),
                            );
                        });
                    });
                    // --- LINE 3: DEBUG UUID ---
                    #[cfg(debug_assertions)]
                    {
                        // Show first 8 chars of UUID
                        let uuid = &op.id;
                        let short_id = if uuid.len() > 8 { &uuid[..8] } else { uuid };
                        ui.label(
                            RichText::new(format!("ID: {}", short_id))
                                .size(9.0)
                                .color(Color32::from_rgb(255, 0, 255)), // Magenta
                        );
                    }
                }
            });
        });
    }

    fn col_strategy_metrics(&self, table_row: &mut TableRow, row: &TradeFinderRow) {
        table_row.col(|ui| {
            if let Some(op) = &row.opportunity {
                let roi_pct = op.live_roi(row.current_price);
                let aroi_pct = op.live_annualized_roi(row.current_price);
                let roi_color = get_outcome_color(roi_pct.value());

                ui.vertical(|ui| {
                    self.down_from_top(ui);

                    // 1. ROI (Primary) + Icon
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(RichText::new(&UI_TEXT.icon_strategy_roi).size(10.0)); // Mountain
                        ui.label(
                            RichText::new(format!("{}", roi_pct))
                                .strong()
                                .color(roi_color),
                        );
                    });

                    // 2. AROI (Secondary) + Icon
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(RichText::new(&UI_TEXT.icon_strategy_aroi).size(10.0)); // Lightning
                        ui.label(
                            RichText::new(format!("{}", aroi_pct))
                                .size(10.0)
                                .color(roi_color.linear_multiply(0.7)),
                        );
                    });

                    // 3. Score (Conditional)
                    // Show if sorting by Score OR if the trade was born from Balance strategy
                    let show_score = self.tf_sort_col == SortColumn::Score
                        || op.strategy == OptimizationStrategy::Balanced;

                    if show_score {
                        let score = op.calculate_quality_score();

                        ui.label(
                            RichText::new(format!(
                                "{}: {:.0}",
                                UI_TEXT.icon_strategy_balanced, score
                            ))
                            .size(9.0)
                            .color(PLOT_CONFIG.color_text_subdued),
                        );
                    }
                });
            } else {
                self.display_no_data(ui);
            }
        });
    }

    fn col_market_state(&self, table_row: &mut TableRow, row: &TradeFinderRow) {
        table_row.col(|ui| {
            if let Some(ms) = &row.market_state {
                ui.vertical(|ui| {
                    self.down_from_top(ui);
                    ui.label(
                        RichText::new(format!("{}", ms.volatility_pct))
                            .small()
                            .color(PLOT_CONFIG.color_info),
                    );
                    let mom_color = get_momentum_color(ms.momentum_pct.value());
                    ui.label(
                        RichText::new(format!("{}", ms.momentum_pct))
                            .small()
                            .color(mom_color),
                    );
                });
            } else {
                self.display_no_data(ui);
            }
        });
    }

    fn col_time(&self, table_row: &mut TableRow, row: &TradeFinderRow) {
        table_row.col(|ui| {
            if let Some(op) = &row.opportunity {
                ui.vertical(|ui| {
                    self.down_from_top(ui);
                    // Avg Duration Only
                    ui.label(
                        RichText::new(TimeUtils::format_duration(op.avg_duration_ms.value()))
                            .small()
                            .color(PLOT_CONFIG.color_text_neutral),
                    );
                });
            } else {
                self.display_no_data(ui);
            }
        });
    }

    fn col_volume_24h(&self, table_row: &mut TableRow, row: &TradeFinderRow) {
        table_row.col(|ui| {
            ui.vertical(|ui| {
                self.down_from_top(ui);
                let val_str = format!("{}", row.quote_volume_24h);
                ui.label(
                    RichText::new(val_str)
                        .small()
                        .color(PLOT_CONFIG.color_text_subdued),
                );
            });
        });
    }

    fn col_sl_variants(&mut self, table_row: &mut TableRow, row: &TradeFinderRow) {
        table_row.col(|ui| {
            ui.vertical(|ui| {
                if let Some(op) = &row.opportunity {
                    self.render_card_variants(ui, op);
                } else {
                    self.display_no_data(ui);
                }
            });
        });
    }

    fn get_filtered_rows(&self) -> Vec<TradeFinderRow> {
        let raw_rows = if let Some(eng) = &self.engine {
            eng.get_trade_finder_rows(Some(&self.simulated_prices))
        } else {
            vec![]
        };

        let selected_op_id = self.selected_opportunity.as_ref().map(|o| &o.id);

        // Scope Helper
        let base_asset = self
            .selected_pair
            .as_ref()
            .and_then(|p| PairInterval::get_base(p))
            .unwrap_or("");

        // Scope Logic
        let is_in_scope = |pair: &str| -> bool {
            if !self.tf_scope_match_base {
                return true;
            }
            // Always keep selected/target pair
            if self.selected_pair.as_deref() == Some(pair) {
                return true;
            }
            // Check Base
            !base_asset.is_empty() && pair.starts_with(base_asset)
        };

        // 1. Group by Pair
        let mut pair_groups: HashMap<String, Vec<TradeFinderRow>> = HashMap::new();
        for row in raw_rows {
            if is_in_scope(&row.pair_name) {
                pair_groups
                    .entry(row.pair_name.clone())
                    .or_default()
                    .push(row);
            }
        }

        let mut final_rows = Vec::new();

        // 2. Process Each Pair
        for (_, mut rows) in pair_groups {
            // Grab metadata from first row for potential placeholder
            let sample = rows[0].clone();

            // Filter down to VALID Opportunities
            rows.retain(|r| {
                if let Some(op) = &r.opportunity {
                    // Rule A: Protection (Selected Op always stays)
                    if selected_op_id == Some(&op.id) {
                        return true;
                    }
                    // Rule B: MWT (Must be worthwhile)
                    if !op.is_worthwhile(&constants::journey::DEFAULT.profile) {
                        return false;
                    }
                    true
                } else {
                    false // Remove existing placeholders (we regenerate below if needed)
                }
            });

            // 3. Result Logic
            if !rows.is_empty() {
                final_rows.append(&mut rows);
            } else {
                // Zero valid trades -> Inject ONE "Market View" row
                final_rows.push(TradeFinderRow {
                    pair_name: sample.pair_name,
                    quote_volume_24h: sample.quote_volume_24h,
                    market_state: sample.market_state,
                    opportunity_count_total: 0,
                    opportunity: None,
                    current_price: sample.current_price,
                });
            }
        }

        final_rows
    }

    /// Renders a small icon-only sort button. Returns true if sort changed.
    fn render_sort_icon_button(&mut self, ui: &mut Ui, col: SortColumn, icon: &str) -> bool {
        let is_active = self.tf_sort_col == col;

        let color = if is_active {
            PLOT_CONFIG.color_text_primary
        } else {
            PLOT_CONFIG.color_text_subdued
        };

        // NEW: Append Sort Arrow if active
        let label_text = if is_active {
            let arrow = match self.tf_sort_dir {
                SortDirection::Ascending => &UI_TEXT.icon_sort_asc,
                SortDirection::Descending => &UI_TEXT.icon_sort_desc,
            };
            // Small gap between icon and arrow
            format!("{}{}", icon, arrow)
        } else {
            icon.to_string()
        };

        // Render
        let response =
            ui.interactive_label(&label_text, is_active, color, FontId::proportional(14.0));

        if response.clicked() {
            if is_active {
                self.tf_sort_dir = self.tf_sort_dir.toggle();
            } else {
                self.tf_sort_col = col;
                self.tf_sort_dir = SortDirection::Descending;
            }
            return true;
        }
        false
    }

    /// Renders a single sortable label using the Interactive Button style
    fn render_stable_sort_label(&mut self, ui: &mut Ui, col: SortColumn, text: &str) -> bool {
        let mut clicked = false;
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

        if ui
            .interactive_label(&label_text, is_active, color, FontId::proportional(14.0))
            .clicked()
        {
            clicked = true;
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
            self.update_scroll_to_selection();
        }
        clicked
    }

    fn render_active_target_panel(&mut self, ui: &mut Ui) {
        let pair_opt = self.selected_pair.clone();
        let opp_opt = self.selected_opportunity.clone();

        Frame::group(ui.style())
            .fill(Color32::from_white_alpha(5))
            .inner_margin(8.0)
            .show(ui, |ui| {
                if let Some(pair) = pair_opt {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{}:", UI_TEXT.label_active_target_text))
                                .size(12.0)
                                .color(PLOT_CONFIG.color_text_subdued),
                        );
                        ui.label(
                            RichText::new(&pair)
                                .size(20.0)
                                .strong()
                                .color(PLOT_CONFIG.color_text_primary),
                        );
                        // Future: Cousins Dropdown here
                    });

                    ui.add_space(5.0);

                    // 2. Context Status
                    if let Some(op) = opp_opt {
                        // A. Trade is Locked
                        ui.horizontal(|ui| {
                            // Direction
                            let dir_color = op.direction.color();
                            let arrow = match op.direction {
                                TradeDirection::Long => &UI_TEXT.icon_long,
                                TradeDirection::Short => &UI_TEXT.icon_short,
                            };
                            ui.label(RichText::new(arrow).size(16.0).color(dir_color));
                            ui.label(
                                RichText::new(op.direction.to_string().to_uppercase())
                                    .strong()
                                    .color(dir_color),
                            );

                            // Live Stats (Mini)
                            // We can safely borrow self here for get_display_price because 'pair_opt' owns the string now
                            let current_price = self.get_display_price(&pair).unwrap_or_default();
                            let roi_pct = op.live_roi(current_price);
                            let color = get_outcome_color(roi_pct.value());

                            ui.label(
                                RichText::new(format!("{} {}", UI_TEXT.label_roi, roi_pct))
                                    .color(color),
                            );
                        });

                        // Source info + ID
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{} {}",
                                    UI_TEXT.label_source_ph, op.source_ph_pct
                                ))
                                .small()
                                .color(PLOT_CONFIG.color_text_subdued),
                            );

                            #[cfg(debug_assertions)]
                            {
                                let short_id = if op.id.len() > 8 { &op.id[..8] } else { &op.id };
                                ui.label(
                                    RichText::new(format!("{}: {}", UI_TEXT.label_id, short_id))
                                        .small()
                                        .color(Color32::from_rgb(255, 0, 255)),
                                );
                            }
                        });
                    } else {
                        // B. Market View (No Trade)
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&UI_TEXT.label_no_targets)
                                    .italics()
                                    .color(PLOT_CONFIG.color_text_neutral),
                            );
                        });
                    }
                } else {
                    // Nothing selected
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new(&UI_TEXT.label_select_pair)
                                .italics()
                                .color(PLOT_CONFIG.color_text_subdued),
                        );
                    });
                }
            });

        ui.add_space(10.0);
    }

    /// Handles events from the Time Tuner UI (Left Panel).
    pub fn handle_tuner_action(&mut self, action: TunerAction) {
        match action {
            TunerAction::StationSelected(station_id) => {
                self.active_station_id = station_id;
                #[cfg(debug_assertions)]
                if DF.log_active_station_id {
                    log::info!(
                        "🔧 ACTIVE STATION SET '{:?}' for ID [{:?}]",
                        self.active_station_id,
                        station_id,
                    );
                }

                // B. Run Auto-Tune for the Active Pair
                if let Some(pair) = &self.selected_pair {
                    let pair_name = pair.clone();

                    // Save the preference immediately
                    self.shared_config
                        .insert_station(pair_name.clone(), station_id);
                    #[cfg(debug_assertions)]
                    if DF.log_station_overrides {
                        log::info!(
                            "🔧 STATION OVERRIDE SET: '{:?}' for [{}] in handle_tuner_action()",
                            station_id,
                            &pair_name,
                        );
                    }

                    if let Some(best_ph_pct) = self.run_auto_tune_logic(&pair_name, station_id) {
                        #[cfg(debug_assertions)]
                        if DF.log_tuner {
                            log::info!(
                                "🎛️ BUTTON TUNE [{}] Station: {:?} -> PH {}",
                                pair_name,
                                station_id,
                                best_ph_pct
                            );
                        }

                        // C. Apply Result to Config
                        self.active_ph_pct = best_ph_pct;

                        // D. Update Engine
                        if let Some(engine) = &mut self.engine {
                            // Create specific config for this pair's override
                            engine
                                .shared_config
                                .insert_ph(pair_name.clone(), best_ph_pct);

                            // Update global context & Fire
                            // engine.update_config(self.app_config.clone());
                            // FIX: Use invalidate_pair_and_recalc to update ONLY this pair
                            engine.invalidate_pair_and_recalc(
                                &pair_name,
                                None,
                                best_ph_pct,
                                self.shared_config.get_strategy(),
                                station_id,
                                JobMode::FullAnalysis,
                                "USER TUNE TIME BUTTON",
                            );
                        }
                    }
                } else {
                }
            }
            TunerAction::ConfigureTuner => {
                #[cfg(debug_assertions)]
                log::info!("TODO: Open Config Modal for Time Tuner");
            }
        }
    }

    pub(super) fn render_left_panel(&mut self, ctx: &Context) {
        let frame = UI_CONFIG.side_panel_frame();

        SidePanel::left("left_panel")
            .min_width(280.0) // I believe this is irrelevant because items we draw inside have higher total min_width
            .resizable(false)
            .frame(frame)
            .show(ctx, |ui| {
                // 1. TIME TUNER
                if let Some(action) = time_tuner::render(
                    ui,
                    &constants::tuner::CONFIG,
                    self.active_station_id,
                    self.selected_pair.clone(),
                ) {
                    self.handle_tuner_action(action);
                }

                ui.add_space(10.0);
                ui.separator();

                // 2. ACTIVE TARGET (New Context Panel)
                self.render_active_target_panel(ui);

                ui.separator();

                // 3. MARKET SCANNER (Formerly Trade Finder)
                // We will rename/update this function next
                self.render_trade_finder_content(ui);
            });
    }

    fn ui_optimization_strategy(&mut self, ui: &mut Ui) {
        ui.label(format!(
            "{} {}",
            UI_TEXT.label_goal,
            UI_TEXT.icon_strategy
        ));

        // Read current value
        let current_strategy = self.shared_config.get_strategy(); // egui mutates a temporary UI variable (to show if updated or not)
        let mut selected_strategy = current_strategy;

        // Selected text (icon + display)
        let selected_text =
            format!("{} {}", selected_strategy.icon(), selected_strategy);

        // ComboBox
        ComboBox::from_label("Optimization strategy")
            .selected_text(selected_text)
            .width(100.0)
            .show_ui(ui, |ui| {
                for strategy in OptimizationStrategy::iter() {
                    let text = format!("{} {}", strategy.icon(), strategy);
                    ui.selectable_value(
                        &mut selected_strategy,
                        strategy,
                        text,
                    );
                }
            });

        // Commit if changed
        if selected_strategy != current_strategy {
            #[cfg(debug_assertions)]
            if DF.log_strategy_selection {
                log::info!(
                    "Changing strategy from {} to {}",
                    current_strategy,
                    selected_strategy
                );
            }

            self.shared_config.set_strategy(selected_strategy);
            self.handle_strategy_selection();
        }

        ui.separator();
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

                    ui.add_space(10.0);
                    ui.separator();

                    self.ui_optimization_strategy(ui);

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
                        self.jump_to_pair(pair);
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        let _ = pair;
                    }
                }
            });
    }

    fn render_card_variants(&mut self, ui: &mut Ui, op: &TradeOpportunity) {
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            // 1. Determine which variant is currently active
            let active_stop_price = if let Some(sel) = &self.selected_opportunity {
                // FIX: Check exact UUID match.
                // Previous logic checked target_zone_id, which is now often 0 for generated trades,
                // causing all trades to think they were selected.
                if sel.id == op.id {
                    sel.stop_price
                } else {
                    op.stop_price
                }
            } else {
                op.stop_price
            };

            // 2. Find the index (1-based)
            let current_index = op
                .variants
                .iter()
                .position(|v| v.stop_price == active_stop_price)
                .unwrap_or(0)
                + 1;

            // 3. Generate Label "#/# Vrts"
            let label_text = format!(
                "{}/{} {} ▾",
                current_index,
                op.variant_count(),
                UI_TEXT.label_sl_variants_short
            );

            // FIX: Use Unique UUID for the UI ID source.
            // Using target_zone_id (which is 0) caused ID collisions in egui.
            let id_source = format!("var_menu_{}", op.id);

            // CALL THE HELPER
            ui.custom_dropdown(&id_source, &label_text, |ui| {
                let mut should_close = false;

                for (i, variant) in op.variants.iter().enumerate() {
                    let risk_pct = Pct::new(variant.stop_price.percent_diff_from_0_1(&op.start_price));
                    let win_rate = variant.simulation.success_rate;

                    let text = format!(
                        "{}. {} {}   {} {}   {} -{}",
                        i + 1,
                        UI_TEXT.label_roi,
                        variant.roi_pct,
                        UI_TEXT.label_success_rate_short,
                        win_rate,
                        UI_TEXT.label_stop_loss_short,
                        risk_pct
                    );

                    let is_current = variant.stop_price == active_stop_price;

                    if ui.selectable_label(is_current, text).clicked() {
                        // 1. Construct the specific variant opportunity
                        let mut new_selected = op.clone();
                        new_selected.stop_price = variant.stop_price;
                        new_selected.simulation = variant.simulation.clone();

                        // 2. Use the Helper
                        self.select_specific_opportunity(
                            new_selected,
                            ScrollBehavior::None,
                            "render_card_variants",
                        );

                        should_close = true;
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
                        RichText::new(format!("{} {}", UI_TEXT.sp_price, sim_price))
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
                        RichText::new(format!("{} {}", UI_TEXT.sp_price, price))
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
                    let zone_size =
                        (cva.price_range.end - cva.price_range.start) / cva.zone_count as f64;
                    ui.metric(
                        &UI_TEXT.sp_zone_size,
                        &format!("{}", zone_size),
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
                        &format!("{}", model.cva.volatility_pct),
                        PLOT_CONFIG.color_warning,
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
                    ("3", UI_TEXT.kbs_toolbar_shortcut_high_wick.as_str()),
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
