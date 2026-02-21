use {
    crate::{
        app::{App, AutoScaleY, Selection, SortDirection},
        config::{
            CandleResolution, MomentumPct, OptimizationStrategy, PLOT_CONFIG, Pct, Price,
            PriceLike, QuoteVol, TICKER, TUNER_CONFIG, VolatilityPct,
        },
        domain::PairInterval,
        engine::JobMode,
        models::{
            DEFAULT_JOURNEY_SETTINGS, MarketState, ScoreType, TradeDirection, TradeOpportunity,
        },
        ui::{
            CandleRangePanel, DirectionColor, PlotInteraction, TunerAction, UI_CONFIG, UI_TEXT,
            UiStyleExt, get_momentum_color, get_outcome_color, render_time_tuner,
        },
        utils::{format_duration, now_utc},
    },
    chrono::Duration,
    eframe::egui::{
        Align, CentralPanel, Color32, ComboBox, Context, FontId, Frame, Grid, Layout, Order,
        RichText, Sense, SidePanel, TopBottomPanel, Ui, Window,
    },
    egui_extras::{Column, TableBuilder, TableRow},
    serde::{Deserialize, Serialize},
    std::{cmp::Ordering, collections::HashMap},
    strum::IntoEnumIterator,
};

#[cfg(debug_assertions)]
use crate::config::DF;

const CELL_PADDING_Y: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum SortColumn {
    PairName,
    TargetPrice,
    #[default]
    LiveRoi,
    AnnualizedRoi,
    AvgDuration,
    QuoteVolume24h,
    Volatility,
    Momentum,
    VariantCount,
    Score,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NavigationTarget {
    Opportunity(String), // UUID (Primary)
    Pair(String),        // Fallback (Market View / No Op)
}

#[derive(PartialEq, Eq)]
pub enum ScrollBehavior {
    Center,
    None,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub(crate) struct NavigationState {
    pub current_segment_idx: Option<usize>, // None = Show All
    pub last_viewed_segment_idx: usize,
}

/// A row in the Trade Finder list.
#[derive(Debug, Clone)]
pub(crate) struct TradeFinderRow {
    pub pair_name: String,
    pub quote_volume_24h: QuoteVol, // 24h Quote Volume (e.g. USDT volume). Filtering "Dead" coins.
    pub market_state: Option<MarketState>, // Market State (Volatility, Momentum)
    pub opportunity: Option<TradeOpportunity>,
    pub current_price: Price,
}

impl App {
    pub(crate) fn render_right_panel(&mut self, ctx: &Context) {
        let frame = UI_CONFIG.side_panel_frame();

        SidePanel::right("right_panel")
            .min_width(160.0)
            .resizable(false)
            .frame(frame)
            .show(ctx, |ui| {
                ui.add_space(5.0);

                if let Some(engine) = &self.engine {
                    if let Some(pair) = &self.selection.pair_owned() {
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
                                self.auto_scale_y = AutoScaleY(true);
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

    pub(crate) fn render_help_panel(&mut self, ctx: &Context) {
        Window::new(&UI_TEXT.kbs_name_long)
            .open(&mut self.show_debug_help)
            .resizable(false)
            .order(Order::Tooltip) // Need coz Plot draws elements on Order::Foreground (and redraws them every second) so need be a higher-level
            .collapsible(false)
            .default_width(400.0)
            .show(ctx, |ui| {
                ui.heading("Press keys to execute commands");
                ui.add_space(10.0);

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
                    ("T", UI_TEXT.kbs_view_time_machine.as_str()),
                ];

                Grid::new("general_shortcuts_grid")
                    .num_columns(2)
                    .spacing([20.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        Self::render_shortcut_rows(ui, &_general_shortcuts);
                    });

                #[cfg(debug_assertions)]
                {
                    // Also add keys here to handle_global_shortcuts
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

    pub(crate) fn render_left_panel(&mut self, ctx: &Context) {
        let frame = UI_CONFIG.side_panel_frame();

        SidePanel::left("left_panel")
            .min_width(280.0)
            .resizable(false)
            .frame(frame)
            .show(ctx, |ui| {
                if let Some(pair) = self.selection.pair_owned() {
                    if let Some(action) = render_time_tuner(
                        ui,
                        &TUNER_CONFIG,
                        self.shared_config.get_station_opt(Some(pair.clone())),
                        Some(pair),
                    ) {
                        self.handle_tuner_action(action);
                    }
                }

                ui.add_space(10.0);
                ui.separator();
                self.render_active_target_panel(ui);
                ui.separator();
                self.render_trade_finder_content(ui);
            });
    }

    pub(crate) fn render_top_panel(&mut self, ctx: &Context) {
        let frame = UI_CONFIG.top_panel_frame();

        TopBottomPanel::top("top_toolbar")
            .frame(frame)
            .min_height(30.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
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

                    self.render_optimization_strategy(ui);

                    ui.checkbox(&mut self.plot_visibility.sticky, &UI_TEXT.tb_sticky);
                    ui.checkbox(&mut self.plot_visibility.low_wicks, &UI_TEXT.tb_low_wicks);
                    ui.checkbox(&mut self.plot_visibility.high_wicks, &UI_TEXT.tb_high_wicks);
                    ui.checkbox(
                        &mut self.plot_visibility.background,
                        &UI_TEXT.tb_volume_hist,
                    );
                    ui.checkbox(&mut self.plot_visibility.candles, &UI_TEXT.tb_candles);

                    ui.separator();

                    ui.checkbox(&mut self.plot_visibility.separators, &UI_TEXT.tb_gaps);
                    ui.checkbox(
                        &mut self.plot_visibility.horizon_lines,
                        &UI_TEXT.tb_price_limits,
                    );
                    ui.checkbox(&mut self.plot_visibility.price_line, &UI_TEXT.tb_live_price);
                    ui.checkbox(&mut self.plot_visibility.opportunities, &UI_TEXT.tb_targets);

                    if self.auto_scale_y.value() {
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

    pub(crate) fn render_ticker_panel(&mut self, ctx: &Context) {
        let panel_frame = UI_CONFIG.bottom_panel_frame();

        TopBottomPanel::bottom("ticker_panel")
            .frame(panel_frame)
            .min_height(TICKER.height)
            .resizable(false)
            .show(ctx, |ui| {
                if let Some(engine) = &self.engine {
                    self.ticker_state.update_data(engine);
                }
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

    pub(crate) fn render_central_panel(&mut self, ctx: &Context) {
        let central_panel_frame = UI_CONFIG.central_panel_frame();

        CentralPanel::default()
            .frame(central_panel_frame)
            .show(ctx, |ui| {
                let nav_state = self.get_nav_state();

                let Some(engine) = &self.engine else {
                    render_fullscreen_message(
                        ui,
                        &UI_TEXT.cp_system_starting,
                        &UI_TEXT.cp_init_engine,
                        false,
                    );
                    return;
                };

                let Some(pair) = self.selection.pair_owned() else {
                    render_fullscreen_message(
                        ui,
                        &UI_TEXT.error_no_pair_selected,
                        &UI_TEXT.cp_please_select_pair,
                        false,
                    );
                    return;
                };

                let current_price = engine.get_price(&pair);

                let (is_calculating, last_error) = engine.get_pair_status(&pair);

                if let Some(err_msg) = last_error {
                    let body = if err_msg.contains("Insufficient data") {
                        format!("{}\n\n{}", UI_TEXT.error_insufficient_data_body, err_msg)
                    } else {
                        err_msg.to_string()
                    };
                    render_fullscreen_message(ui, &UI_TEXT.error_analysis_failed, &body, true);
                } else if let Some(model) = engine.get_model(&pair) {
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
                        self.auto_scale_y.value(),
                        self.selection.opportunity().cloned(),
                    );

                    match interaction {
                        PlotInteraction::UserInteracted => {
                            // User wants control. Disable auto-scale.
                            self.auto_scale_y = AutoScaleY(false);
                        }
                        PlotInteraction::RequestReset => {
                            // User requested reset. Re-enable auto-scale.
                            self.auto_scale_y = AutoScaleY(true);
                        }
                        PlotInteraction::None => {}
                    }
                } else if is_calculating {
                    render_fullscreen_message(
                        ui,
                        &format!("{} {}...", UI_TEXT.cp_analyzing, pair),
                        &UI_TEXT.cp_calculating_zones,
                        false,
                    );
                } else if current_price.is_some() {
                    render_fullscreen_message(
                        ui,
                        &format!("{}: {}...", UI_TEXT.cp_queued, pair),
                        &UI_TEXT.cp_wait_thread,
                        false,
                    );
                } else {
                    render_fullscreen_message(
                        ui,
                        &UI_TEXT.cp_wait_prices,
                        &UI_TEXT.cp_listen_binance_stream,
                        false,
                    );
                }
            });
    }

    pub(crate) fn render_status_panel(&mut self, ctx: &Context) {
        let frame = UI_CONFIG.bottom_panel_frame();
        TopBottomPanel::bottom("status_panel")
            .frame(frame)
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        self.render_price(ui);
                        self.render_status_zone_info(ui);
                        ui.separator();
                        self.render_status_coverage(ui);
                        self.render_status_candles(ui);
                        self.render_status_system(ui);
                        ui.separator();
                        self.render_status_network(ui);
                    });
                });
            });
    }

    fn render_trade_finder_filters(&mut self, ui: &mut Ui, count: usize) -> bool {
        // Renders the Header, Scope, and Direction controls
        let mut filter_changed = false;
        ui.add_space(10.0);

        // SCOPE TOGGLE
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

            // Center on Target button - Only show if target available
            if self.selection.pair_owned().is_some() {
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

            if ui
                .selectable_label(!self.tf_scope_match_base, &UI_TEXT.tf_scope_all)
                .clicked()
            {
                self.tf_scope_match_base = false;
                filter_changed = true;
                self.update_scroll_to_selection();
            }

            let base_asset = self
                .selection
                .pair()
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

    fn display_no_data(&self, ui: &mut Ui) {
        // Helper for Empty Cells (SSOT)
        ui.label("-");
    }

    fn down_from_top(&self, ui: &mut Ui) {
        ui.add_space(CELL_PADDING_Y);
    }

    fn render_strategy_header_grid(&mut self, ui: &mut Ui, sort_changed: &mut bool) {
        let goals: Vec<_> = OptimizationStrategy::iter().collect();
        let count = goals.len();

        let mut render_btn = |ui: &mut Ui, idx: usize| {
            if let Some(goal) = goals.get(idx) {
                let col = match goal {
                    OptimizationStrategy::MaxROI => SortColumn::LiveRoi,
                    OptimizationStrategy::MaxAROI => SortColumn::AnnualizedRoi,
                    OptimizationStrategy::Balanced => SortColumn::Score,
                    OptimizationStrategy::LogGrowthConfidence => SortColumn::Score, // TEMP shares score column right no
                };
                if self.render_sort_icon_button(ui, col, &goal.icon()) {
                    *sort_changed = true;
                }
            }
        };

        ui.vertical_centered(|ui| {
            ui.add_space(2.0);

            if count == 1 {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    render_btn(ui, 0);
                });
            } else if count == 2 {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0; // Gap
                    render_btn(ui, 0);
                    render_btn(ui, 1);
                });
            } else if count == 3 {
                // Case 3: Top Center, Bottom Split
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    render_btn(ui, 0);
                });
                ui.horizontal(|ui| {
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

        #[cfg(debug_assertions)]
        if let Selection::Opportunity(sel) = &self.selection {
            let exists = rows
                .iter()
                .any(|r| r.opportunity.as_ref().is_some_and(|op| op.id == sel.id));

            if !exists && DF.log_selection {
                log::warn!(
                    "UI invariant violation: selected opportunity {} not present in rendered rows",
                    sel.id
                );
            }
        }

        // Sortx
        self.sort_trade_finder_rows(&mut rows);
        if rows.is_empty() {
            ui.centered_and_justified(|ui| ui.label("Loading Market Data..."));
            return;
        }

        let mut target_index = None;
        if let Some(target) = &self.scroll_target {
            target_index = rows.iter().position(|r| {
                match target {
                    // Hunting a specific Trade (UUID)
                    NavigationTarget::Opportunity(id) => {
                        r.opportunity.as_ref().is_some_and(|op| op.id == *id)
                    }
                    // Hunting a Pair (Market View)
                    NavigationTarget::Pair(name) => r.pair_name == *name,
                }
            });
        }

        let available_height = ui.available_height();

        let mut sort_changed = false;

        ui.scope(|ui| {
            let visuals = ui.visuals_mut();

            visuals.selection.bg_fill = PLOT_CONFIG.color_tf_selected;
            // "Stripe" color
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

            // Apply Scroll at Builder Level
            // Works even if the row is virtualized (off-screen)
            if let Some(idx) = target_index {
                builder = builder.scroll_to_row(idx, Some(Align::Center));
            }

            builder
                .header(48.0, |mut header| {
                    self.render_tf_table_header(&mut header, &mut sort_changed);
                })
                .body(|mut body| {
                    for (i, row) in rows.iter().enumerate() {
                        body.row(55.0, |mut table_row| {
                            self.render_tf_table_row(&mut table_row, row, i);
                        });
                    }
                });

            // 6. Sticky Scroll Cleanup
            // If Sort OR Filter changed, we DO NOT clear the request.
            // Wait for NEXT frame where rows are regenerated/resorted.
            if !sort_changed && !filter_changed {
                if target_index.is_some() {
                    self.scroll_target = None;
                }
            }
        });
    }

    fn render_header_stack(
        // Helper to render a stacked header (Primary Sort Top, Optional Secondary Sort Bottom)
        &mut self,
        ui: &mut Ui,
        sort_changed: &mut bool,
        col_top: SortColumn,
        txt_top: &str,
        col_bot: Option<(SortColumn, &str)>,
    ) {
        ui.vertical_centered(|ui| {
            if self.render_stable_sort_label(ui, col_top, txt_top) {
                *sort_changed = true;
            }
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

        header.col(|ui| {
            self.render_strategy_header_grid(ui, sort_changed);
        });

        // Market
        header.col(|ui| {
            self.render_header_stack(
                ui,
                sort_changed,
                SortColumn::Volatility,
                &UI_TEXT.label_volatility_short,
                Some((SortColumn::Momentum, &UI_TEXT.label_momentum_short)),
            );
        });

        // Average Duration / Trade Balance Score
        header.col(|ui| {
            self.render_header_stack(
                ui,
                sort_changed,
                SortColumn::AvgDuration,
                &UI_TEXT.tf_time,
                None,
            );
        });

        // Volume
        header.col(|ui| {
            self.render_header_stack(
                ui,
                sort_changed,
                SortColumn::QuoteVolume24h,
                &UI_TEXT.label_volume_24h,
                None,
            );
        });

        // Risk
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

    fn render_tf_table_row(
        // Renders the data cells for a single row
        &mut self,
        table_row: &mut TableRow,
        row: &TradeFinderRow,
        index: usize,
    ) {
        // Selection Logic (Selection-only)
        let is_selected = match (&self.selection, &row.opportunity) {
            (Selection::Opportunity(sel), Some(op)) => sel.id == op.id,
            (Selection::Pair(pair), None) => pair == &row.pair_name,
            (Selection::None, _) => false,
            _ => false,
        };

        table_row.set_selected(is_selected);

        self.col_pair_name(table_row, row, index);
        self.col_strategy_metrics(table_row, row);
        self.col_market_state(table_row, row);
        self.col_time(table_row, row);
        self.col_volume_24h(table_row, row);
        self.col_sl_variants(table_row, row);

        let response = table_row.response();

        if response.clicked() {
            match &row.opportunity {
                Some(op) => {
                    self.select_opportunity(
                        op.clone(),
                        ScrollBehavior::None,
                        "clicked in render_tf_table_row",
                    );
                }
                None => {
                    // Case where we start as NOPP
                    #[cfg(debug_assertions)]
                    if DF.log_selection {
                        log::info!(
                            "Started as NOPP. So need make a selection based on new pair name "
                        );
                    }
                    self.selection = Selection::Pair(row.pair_name.clone());
                }
            }
        }
    }

    fn col_pair_name(
        // Column 1: Pair Name + Direction Icon (Static Text) on top line, Age on 2nd line
        &self,
        table_row: &mut egui_extras::TableRow,
        row: &TradeFinderRow,
        index: usize,
    ) {
        table_row.col(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.style_mut().spacing.item_spacing.x = 4.0;
                    ui.label(
                        RichText::new(format!("{}.", index))
                            .size(10.0)
                            .color(PLOT_CONFIG.color_text_subdued),
                    );
                    ui.label(
                        RichText::new(&row.pair_name)
                            .strong()
                            .size(14.0)
                            .color(PLOT_CONFIG.color_text_primary),
                    );
                    if let Some(op) = &row.opportunity {
                        ui.label(
                            RichText::new(op.station_id.short_name())
                                .size(14.0)
                                .color(PLOT_CONFIG.color_text_neutral),
                        );
                        ui.label(
                            RichText::new(op.strategy.icon())
                                .size(14.0)
                                .color(PLOT_CONFIG.color_text_neutral),
                        );
                        let arrow = match op.direction {
                            TradeDirection::Long => &UI_TEXT.icon_long,
                            TradeDirection::Short => &UI_TEXT.icon_short,
                        };
                        ui.label(RichText::new(arrow).color(op.direction.color()));
                    }
                });

                if let Some(op) = &row.opportunity {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("T: {}", op.target_price))
                                .size(10.0)
                                .color(PLOT_CONFIG.color_info),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let now = now_utc();
                            let age = now - op.created_at;
                            let age_str = if age < Duration::minutes(1) {
                                "New".to_string()
                            } else {
                                format_duration(age.num_milliseconds())
                            };

                            ui.label(
                                RichText::new(age_str)
                                    .size(10.0)
                                    .color(PLOT_CONFIG.color_text_subdued),
                            );
                        });
                    });
                    #[cfg(debug_assertions)]
                    {
                        let uuid = &op.id;
                        let short_id = if uuid.len() > 8 { &uuid[..8] } else { uuid };
                        ui.label(
                            RichText::new(format!("ID: {} (PH: {})", short_id, op.ph_pct))
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

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(RichText::new(&UI_TEXT.icon_strategy_roi).size(10.0)); // Mountain
                        ui.label(
                            RichText::new(format!("{}", roi_pct))
                                .strong()
                                .color(roi_color),
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(RichText::new(&UI_TEXT.icon_strategy_aroi).size(10.0)); // Lightning
                        ui.label(
                            RichText::new(format!("{}", aroi_pct))
                                .size(10.0)
                                .color(roi_color.linear_multiply(0.7)),
                        );
                    });

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
                        RichText::new(format_duration(op.avg_duration.value()))
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
            eng.get_trade_finder_rows()
        } else {
            vec![]
        };

        let selected_op_id = self.selection.opportunity().map(|o| &o.id);

        let base_asset = self
            .selection
            .pair()
            .and_then(|p| PairInterval::get_base(p))
            .unwrap_or("");

        let is_in_scope = |pair: &str| -> bool {
            if !self.tf_scope_match_base {
                return true;
            }
            if self.selection.pair() == Some(pair) {
                return true;
            }
            !base_asset.is_empty() && pair.starts_with(base_asset)
        };

        // Group by Pair
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

        // Process Each Pair
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
                    if !op.is_worthwhile(&DEFAULT_JOURNEY_SETTINGS.profile) {
                        return false;
                    }
                    true
                } else {
                    false // Remove existing placeholders (regenerate below if needed)
                }
            });

            // Result Logic
            if !rows.is_empty() {
                final_rows.append(&mut rows);
            } else {
                // Zero valid trades -> Inject ONE "Market View" row
                final_rows.push(TradeFinderRow {
                    pair_name: sample.pair_name,
                    quote_volume_24h: sample.quote_volume_24h,
                    market_state: sample.market_state,
                    opportunity: None,
                    current_price: sample.current_price,
                });
            }
        }

        final_rows
    }

    fn render_sort_icon_button(&mut self, ui: &mut Ui, col: SortColumn, icon: &str) -> bool {
        // Renders a small icon-only sort button. Returns true if sort changed.
        let is_active = self.tf_sort_col == col;

        let color = if is_active {
            PLOT_CONFIG.color_text_primary
        } else {
            PLOT_CONFIG.color_text_subdued
        };

        let label_text = if is_active {
            let arrow = match self.tf_sort_dir {
                SortDirection::Ascending => &UI_TEXT.icon_sort_asc,
                SortDirection::Descending => &UI_TEXT.icon_sort_desc,
            };
            format!("{}{}", icon, arrow)
        } else {
            icon.to_string()
        };

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

    fn render_stable_sort_label(&mut self, ui: &mut Ui, col: SortColumn, text: &str) -> bool {
        // Renders a single sortable label using the Interactive Button style
        let mut clicked = false;
        let is_active = self.tf_sort_col == col;
        let color = if is_active {
            PLOT_CONFIG.color_text_primary
        } else {
            PLOT_CONFIG.color_text_subdued
        };

        let suffix = if is_active {
            match self.tf_sort_dir {
                SortDirection::Ascending => &UI_TEXT.icon_sort_asc,
                SortDirection::Descending => &UI_TEXT.icon_sort_desc,
            }
        } else {
            "  "
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
        let pair_opt = self.selection.pair_owned();
        let opp_opt = self.selection.opportunity();

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
                    });

                    ui.add_space(5.0);

                    if let Some(op) = opp_opt {
                        // A. Trade is Locked
                        ui.horizontal(|ui| {
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

                            if let Some(engine) = &mut self.engine {
                                if let Some(current_price) = engine.get_price(&pair) {
                                    let roi_pct = op.live_roi(current_price);
                                    let color = get_outcome_color(roi_pct.value());
                                    ui.label(
                                        RichText::new(format!("{} {}", UI_TEXT.label_roi, roi_pct))
                                            .color(color),
                                    );
                                } else {
                                    log::info!("No price available for {}", pair);
                                }
                            } else {
                                log::info!("No engine available for {}", pair);
                            }
                        });

                        // Source info + ID
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{} {}", UI_TEXT.label_source_ph, op.ph_pct))
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

    fn handle_tuner_action(&mut self, action: TunerAction) {
        // Handles events from the Time Tuner UI (Left Panel).
        match action {
            TunerAction::StationSelected(station_id) => {
                if let Some(pair) = self.selection.pair_owned() {
                    let pair_name = pair.clone();
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

                    if let Some(engine) = &mut self.engine {
                        if let Some(best_ph_pct) =
                            engine.tune_pair_with_station(&pair_name, station_id)
                        {
                            #[cfg(debug_assertions)]
                            if DF.log_tuner {
                                log::info!(
                                    "🎛️ BUTTON TUNE [{}] Station: {:?} -> PH {}",
                                    pair_name,
                                    station_id,
                                    best_ph_pct
                                );
                            }

                            engine
                                .shared_config
                                .insert_ph(pair_name.clone(), best_ph_pct);

                            #[cfg(debug_assertions)]
                            if DF.log_ph_overrides {
                                log::info!(
                                    "SETTING PH_OVERRIDES for {} to be {} in handle_tuner_action",
                                    pair_name,
                                    best_ph_pct
                                );
                            }

                            // Recalculate this pair
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

    fn render_price(&self, ui: &mut Ui) {
        if let Some(pair) = &self.selection.pair_owned() {
            // Live Mode only now (no simulated price mode)
            ui.label(
                RichText::new(format!("{} ", &UI_TEXT.sp_live_mode))
                    .small()
                    .color(PLOT_CONFIG.color_profit),
            );
            ui.separator();

            if let Some(engine) = &self.engine {
                // if let Some(price) = self.get_display_price(pair) {
                ui.label(
                    RichText::new(format!("{} {:?}", UI_TEXT.sp_price, engine.get_price(pair)))
                        .strong()
                        .color(PLOT_CONFIG.color_text_primary),
                );
            } else {
                ui.label(format!("{} ...", UI_TEXT.label_connecting));
            }
        }
    }

    fn render_status_zone_info(&self, ui: &mut Ui) {
        if let Some(engine) = &self.engine {
            if let Some(pair) = &self.selection.pair_owned() {
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
            if let Some(pair) = &self.selection.pair_owned() {
                if let Some(model) = engine.get_model(pair) {
                    let cov_color = |pct: f64| {
                        if pct > 30.0 {
                            PLOT_CONFIG.color_loss
                        } else if pct < 5.0 {
                            PLOT_CONFIG.color_warning
                        } else {
                            PLOT_CONFIG.color_profit
                        }
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
            if let Some(pair) = &self.selection.pair_owned() {
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
            let health: Pct = engine.price_stream.connection_health();
            let color = if health >= Pct::new(0.9) {
                PLOT_CONFIG.color_profit
            } else if health >= Pct::new(0.5) {
                PLOT_CONFIG.color_warning
            } else {
                PLOT_CONFIG.color_loss
            };
            ui.metric(
                &UI_TEXT.sp_stream_status,
                &format!("{} {}", health, UI_TEXT.label_connected),
                color,
            );
        }
    }

    fn render_card_variants(&mut self, ui: &mut Ui, op: &TradeOpportunity) {
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            let active_stop_price = if let Some(sel) = &self.selection.opportunity() {
                if sel.id == op.id {
                    sel.stop_price
                } else {
                    op.stop_price
                }
            } else {
                op.stop_price
            };

            let current_index = op
                .variants
                .iter()
                .position(|v| v.stop_price == active_stop_price)
                .unwrap_or(0)
                + 1;

            // Generate Label "#/# Vrts"
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
                    let risk_pct =
                        Pct::new(variant.stop_price.percent_diff_from_0_1(&op.start_price));
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
                        let mut new_selected = op.clone();
                        new_selected.stop_price = variant.stop_price;
                        new_selected.simulation = variant.simulation.clone();

                        self.select_opportunity(
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

    fn render_optimization_strategy(&mut self, ui: &mut Ui) {
        ui.label(&UI_TEXT.label_goal);

        let current_strategy = self.shared_config.get_strategy();
        let mut selected_strategy = current_strategy;
        let selected_text = format!("{} {}", selected_strategy.icon(), selected_strategy);

        ComboBox::from_id_salt("Optimization strategy")
            .selected_text(selected_text)
            .width(100.0)
            .show_ui(ui, |ui| {
                for strategy in OptimizationStrategy::iter() {
                    let text = format!("{} {}", strategy.icon(), strategy);
                    ui.selectable_value(&mut selected_strategy, strategy, text);
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

    fn render_shortcut_rows(ui: &mut Ui, rows: &[(&str, &str)]) {
        for (key, description) in rows {
            ui.label(RichText::new(*key).monospace().strong());
            ui.label(*description);
            ui.end_row();
        }
    }

    fn sort_trade_finder_rows(&self, rows: &mut [TradeFinderRow]) {
        rows.sort_by(|a, b| {
            // Always push "No Opportunity" rows to the bottom
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

            // Standard Sort
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
                        .map(|o| o.avg_duration.value())
                        .unwrap_or(i64::MAX);
                    let val_b = b
                        .opportunity
                        .as_ref()
                        .map(|o| o.avg_duration.value())
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
