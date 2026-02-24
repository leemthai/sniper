use {
    eframe::{
        Frame, Storage,
        egui::{
            CentralPanel, Context, FontData, FontDefinitions, FontFamily, Key, ProgressBar, Visuals,
        },
    },
    serde::{Deserialize, Serialize},
    std::{
        collections::{HashMap, HashSet},
        mem,
        sync::{Arc, mpsc, mpsc::Receiver},
    },
};

use crate::{
    Cli,
    app::{
        AppState, AutoScaleY, BootstrapState, PersistedSelection, PhaseView, ProgressEvent,
        RunningState, Selection, SortDirection, SyncStatus, TuningState,
    },
    config::{CandleResolution, DF, PhPct},
    data::{TimeSeriesCollection, fetch_pair_data},
    engine::SniperEngine,
    models::{TradeOpportunity, restore_engine_ledger},
    shared::SharedConfiguration,
    ui::{
        NavigationState, NavigationTarget, PlotView, PlotVisibility, ScrollBehavior, SortColumn,
        TickerState, UI_CONFIG, render_bootstrap,
    },
    utils::AppInstant,
};

#[cfg(not(target_arch = "wasm32"))]
use {
    crate::{config::Pct, data::save_ledger},
    std::thread,
    tokio::runtime::Runtime,
};

#[cfg(feature = "ph_audit")]
use crate::{
    config::BASE_INTERVAL,
    models::find_matching_ohlcv,
    ph_audit::{AUDIT_PAIRS, execute_audit},
};

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct App {
    pub(crate) shared_config: SharedConfiguration, // persists across sessions.
    #[serde(skip)]
    pub(crate) selection: Selection,
    #[serde(skip)]
    pub(crate) valid_session_pairs: HashSet<String>,
    pub(crate) persisted_selection: PersistedSelection,
    pub(crate) plot_visibility: PlotVisibility,
    pub(crate) show_debug_help: bool,
    pub(crate) show_ph_help: bool,
    pub(crate) candle_resolution: CandleResolution,
    pub(crate) show_candle_range: bool,
    pub(crate) tf_scope_match_base: bool,
    pub(crate) tf_sort_col: SortColumn,
    pub(crate) tf_sort_dir: SortDirection,
    #[serde(skip)]
    pub(crate) scroll_target: Option<NavigationTarget>,
    #[serde(skip)]
    pub(crate) engine: Option<SniperEngine>,
    #[serde(skip)]
    pub(crate) plot_view: PlotView,
    #[serde(skip)]
    state: AppState,
    #[serde(skip)]
    pub(crate) progress_rx: Option<Receiver<ProgressEvent>>,
    #[serde(skip)]
    pub(crate) data_rx: Option<Receiver<(TimeSeriesCollection, &'static str)>>,
    #[serde(skip)]
    pub(crate) nav_states: HashMap<String, NavigationState>,
    #[serde(skip)]
    pub(crate) auto_scale_y: AutoScaleY,
    #[serde(skip)]
    pub(crate) ticker_state: TickerState,
}

impl Default for App {
    fn default() -> Self {
        Self {
            selection: Selection::default(),
            persisted_selection: PersistedSelection::None,
            shared_config: SharedConfiguration::new(),
            plot_visibility: PlotVisibility::default(),
            valid_session_pairs: HashSet::new(),
            show_debug_help: false,
            show_ph_help: false,
            engine: None,
            plot_view: PlotView::new(),
            state: AppState::default(),
            progress_rx: None,
            data_rx: None,
            scroll_target: None,
            nav_states: HashMap::new(),
            candle_resolution: CandleResolution::default(),
            auto_scale_y: AutoScaleY::default(),
            ticker_state: TickerState::default(),
            tf_scope_match_base: false,
            show_candle_range: false,
            tf_sort_col: SortColumn::default(),
            tf_sort_dir: SortDirection::default(),
        }
    }
}

impl App {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>, args: Cli) -> Self {
        let mut app: App = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Self::default()
        };

        Self::configure_fonts(&cc.egui_ctx);

        app.plot_view = PlotView::new();
        app.state = AppState::Bootstrapping(BootstrapState::default());

        let (data_tx, data_rx) = mpsc::channel();
        let (prog_tx, prog_rx) = mpsc::channel();

        app.data_rx = Some(data_rx);
        app.progress_rx = Some(prog_rx);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let args_clone = args.clone();
            thread::spawn(move || {
                let rt = Runtime::new().expect("Failed to create runtime");
                rt.block_on(async move {
                    let (data, sig) = fetch_pair_data(300, &args_clone, Some(prog_tx)).await;

                    let _ = data_tx.send((data, sig));
                });
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            let _ = prog_tx;
            let args_clone = args.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let (data, sig) = fetch_pair_data(0, &args_clone, None).await;
                let _ = data_tx.send((data, sig));
            });
        }

        app
    }

    /// Change global strategy
    pub(crate) fn handle_strategy_selection(&mut self) {
        let priority_pair = self.selection.pair_owned();
        if let Some(e) = &mut self.engine {
            e.trigger_global_recalc(priority_pair);
        }
    }

    pub(crate) fn update_scroll_to_selection(&mut self) {
        self.scroll_target = match &self.selection {
            Selection::Opportunity(op) => Some(NavigationTarget::Opportunity(op.id.clone())),
            Selection::Pair(pair) => Some(NavigationTarget::Pair(pair.clone())),
            Selection::None => None,
        };
    }

    pub(crate) fn jump_to_pair(&mut self, pair: String) {
        if matches!(self.selection, Selection::Pair(ref p) if p == &pair) {
            self.update_scroll_to_selection();
            return;
        }

        let best_op = self.engine.as_ref().and_then(|e| {
            e.get_trade_finder_rows()
                .into_iter()
                .find(|r| r.pair_name == pair)
                .and_then(|r| r.opportunity)
        });
        if let Some(op) = best_op {
            self.select_opportunity(op, ScrollBehavior::Center, "jump to pair");
        } else {
            self.selection = Selection::Pair(pair);
        }
        self.update_scroll_to_selection();
    }

    pub(crate) fn select_opportunity(
        &mut self,
        op: TradeOpportunity,
        scroll: ScrollBehavior,
        _reason: &str,
    ) {
        #[cfg(debug_assertions)]
        if DF.log_selection {
            log::info!("Call select_opportunity because {}", _reason);
        }
        self.selection = Selection::Opportunity(op.clone());
        #[cfg(debug_assertions)]
        if DF.log_selection {
            log::info!("SELECTION SET to Opportunity {} in select_opportunity", op);
        }
        if matches!(scroll, ScrollBehavior::Center) {
            self.scroll_target = Some(NavigationTarget::Opportunity(op.id));
        }
    }

    pub(crate) fn get_nav_state(&mut self) -> NavigationState {
        let pair = match &self.selection {
            Selection::Opportunity(op) => op.pair_name.clone(),
            Selection::Pair(pair) => pair.clone(),
            Selection::None => "DEFAULT".to_string(),
        };
        *self.nav_states.entry(pair).or_default()
    }

    pub(crate) fn set_nav_state(&mut self, state: NavigationState) {
        let pair = match &self.selection {
            Selection::Opportunity(op) => op.pair_name.clone(),
            Selection::Pair(pair) => pair.clone(),
            Selection::None => return, // nowhere to store state
        };

        self.nav_states.insert(pair, state);
    }

    pub(crate) fn ensure_nav_state_for_pair(&mut self, pair: &str) {
        self.nav_states.entry(pair.to_owned()).or_default();
    }

    pub(crate) fn handle_global_shortcuts(&mut self, ctx: &Context) {
        if ctx.wants_keyboard_input() {
            // If the user typing intext box, don't trigger global hotkeys.
            return;
        }

        ctx.input(|i| {
            if i.key_pressed(Key::Num1) {
                self.plot_visibility.sticky = !self.plot_visibility.sticky;
            }
            if i.key_pressed(Key::Num2) {
                self.plot_visibility.low_wicks = !self.plot_visibility.low_wicks;
            }
            if i.key_pressed(Key::Num3) {
                self.plot_visibility.high_wicks = !self.plot_visibility.high_wicks;
            }
            if i.key_pressed(Key::Num4) {
                self.plot_visibility.background = !self.plot_visibility.background;
            }
            if i.key_pressed(Key::Num5) {
                self.plot_visibility.candles = !self.plot_visibility.candles;
            }
            if i.key_pressed(Key::Num6) {
                self.plot_visibility.separators = !self.plot_visibility.separators;
            }
            if i.key_pressed(Key::Num7) {
                self.plot_visibility.horizon_lines = !self.plot_visibility.horizon_lines;
            }
            if i.key_pressed(Key::Num8) {
                self.plot_visibility.price_line = !self.plot_visibility.price_line;
            }
            if i.key_pressed(Key::Num9) {
                self.plot_visibility.opportunities = !self.plot_visibility.opportunities;
            }
            if i.key_pressed(Key::K) || i.key_pressed(Key::H) {
                self.show_debug_help = !self.show_debug_help;
            }
            if i.key_pressed(Key::Escape) {
                self.show_debug_help = false;
                self.show_ph_help = false;
            }
            if i.key_pressed(Key::T) {
                self.show_candle_range = !self.show_candle_range;
            }
        });
    }

    pub(crate) fn tick_tuning_state(&mut self, ctx: &Context, state: &mut TuningState) -> AppState {
        CentralPanel::default().show(ctx, |ui| {
            ui.centered_and_justified(|ui| {
                ui.set_max_width(300.0);
                ui.vertical(|ui| {
                    ui.heading("Optimizing Models");
                    ui.add_space(10.0);
                    let progress = state.completed as f32 / state.total.max(1) as f32;
                    let text = format!("Tuning {} / {} pairs...", state.completed, state.total);
                    ui.add(ProgressBar::new(progress).text(text));
                });
            });
        });

        let chunk_size = 5; // keep UI responsive
        let mut processed = 0;

        if let Some(e) = &mut self.engine {
            #[cfg(not(target_arch = "wasm32"))]
            e.price_stream.wait_for_health_threshold(Pct::new(0.5));
            while processed < chunk_size && !state.todo_list.is_empty() {
                if let Some(pair) = state.todo_list.pop() {
                    if let Some(ph) = e.tune_pair_from_config(&pair) {
                        e.shared_config.insert_ph(pair.clone(), ph);

                        #[cfg(debug_assertions)]
                        if DF.log_ph_overrides {
                            log::info!(
                                "WRITING ph value {} for pair {} during tuning phase",
                                ph,
                                pair
                            );
                        }
                    }
                    processed += 1;
                }
            }
        } else {
            log::warn!("No engine. Not good.");
        }

        state.completed += processed;
        if state.todo_list.is_empty() {
            if let Some(e) = &mut self.engine {
                if DF.log_tuner {
                    log::info!(">> Global Tuning Complete. Igniting Engine.");
                }
                e.trigger_global_recalc(None);
            }
            if let Some(target_pair) = self.selection.pair() {
                self.jump_to_pair(target_pair.to_string());
            }

            AppState::Running(RunningState)
        } else {
            ctx.request_repaint();
            AppState::Tuning(state.clone())
        }
    }

    /// RUNNING PHASE MAIN LOOP
    pub(crate) fn tick_running_state(&mut self, ctx: &Context) {
        let start = AppInstant::now();

        if let Some(e) = &mut self.engine {
            let removals = e.update();
            self.clear_selection_if_opportunity_removed(&removals.ids);
        }

        self.ensure_valid_selection();
        let engine_time = start.elapsed().as_micros();
        self.handle_global_shortcuts(ctx);
        self.render_top_panel(ctx);
        let start = AppInstant::now();
        self.render_left_panel(ctx);
        let left_panel_time = start.elapsed().as_micros();
        if self.show_candle_range {
            self.render_right_panel(ctx);
        }
        self.render_ticker_panel(ctx);
        self.render_status_panel(ctx);
        let start = AppInstant::now();
        self.render_central_panel(ctx);
        let plot_time = start.elapsed().as_micros();
        self.render_help_panel(ctx);
        if engine_time + left_panel_time + plot_time > 500_000 {
            if DF.log_performance {
                log::warn!(
                    "🐢 SLOW FRAME: Engine: {}us | LeftPanel(TF): {}us | Plot: {}us",
                    engine_time,
                    left_panel_time,
                    plot_time
                );
            }
        }
    }

    pub(crate) fn finalize_bootstrap_if_ready(&mut self) -> Option<AppState> {
        if let Some(rx) = &self.data_rx {
            if let Ok((timeseries, _sig)) = rx.try_recv() {
                self.build_engine(timeseries);
                self.restore_initial_selection();
                return Some(AppState::Tuning(TuningState {
                    total: self.valid_session_pairs.len(),
                    completed: 0,
                    todo_list: self.valid_session_pairs.iter().cloned().collect(),
                }));
            }
        }
        None
    }

    pub(crate) fn tick_bootstrap_state(
        &mut self,
        ctx: &Context,
        state: &mut BootstrapState,
    ) -> AppState {
        self.update_loading_progress(state);
        ctx.request_repaint();
        if let Some(next_state) = self.finalize_bootstrap_if_ready() {
            return next_state;
        }
        render_bootstrap(ctx, state);
        AppState::Bootstrapping(state.clone())
    }

    pub(crate) fn update_loading_progress(&mut self, state: &mut BootstrapState) {
        if let Some(rx) = &self.progress_rx {
            while let Ok(event) = rx.try_recv() {
                state.pairs.insert(event.index, (event.pair, event.status));
            }
            state.total_pairs = state.pairs.len();
            state.completed = state
                .pairs
                .values()
                .filter(|(_, s)| matches!(s, SyncStatus::Completed(_)))
                .count();
            state.failed = state
                .pairs
                .values()
                .filter(|(_, s)| matches!(s, SyncStatus::Failed(_)))
                .count();
        }
    }

    fn restore_initial_selection(&mut self) {
        self.selection = match &self.persisted_selection {
            PersistedSelection::None => Selection::None,
            PersistedSelection::Pair(pair) => {
                if self.valid_session_pairs.contains(pair) {
                    Selection::Pair(pair.clone())
                } else {
                    Selection::None
                }
            }
            PersistedSelection::Opportunity {
                pair,
                opportunity_id,
            } => {
                if let Some(engine) = &self.engine {
                    if let Some(op) = engine.engine_ledger.opportunities.get(opportunity_id) {
                        Selection::Opportunity(op.clone())
                    } else if self.valid_session_pairs.contains(pair) {
                        // Opportunity expired → fall back to its pair
                        Selection::Pair(pair.clone())
                    } else {
                        Selection::None
                    }
                } else {
                    Selection::None
                }
            }
        };

        if matches!(self.selection, Selection::None) {
            if let Some(pair) = self.valid_session_pairs.iter().next().cloned() {
                self.selection = Selection::Pair(pair);
            }
        }
        if let Some(pair) = self.selection.pair_owned() {
            self.ensure_nav_state_for_pair(&pair);
        }
    }

    fn build_engine(&mut self, timeseries: TimeSeriesCollection) {
        self.initialize_pair_state(&timeseries);
        let mut engine = SniperEngine::new(
            timeseries,
            self.shared_config.clone(),
            self.valid_session_pairs.iter().cloned().collect(),
        );
        engine.engine_ledger = restore_engine_ledger(&self.valid_session_pairs);
        self.engine = Some(engine);
    }

    fn initialize_pair_state(&mut self, timeseries: &TimeSeriesCollection) {
        let available_pairs = timeseries.unique_pair_names();
        self.valid_session_pairs = available_pairs.iter().cloned().collect();
        self.shared_config
            .ensure_all_stations_initialized(&available_pairs);
        self.shared_config
            .ensure_all_phs_initialized(&available_pairs, PhPct::default());
    }

    fn configure_fonts(ctx: &Context) {
        let mut fonts = FontDefinitions::default();
        let mut font_data_mono =
            FontData::from_static(include_bytes!("../../fonts/HackNerdFont-Regular.ttf"));
        font_data_mono.tweak.scale = 0.85;
        let mut font_data_propo =
            FontData::from_static(include_bytes!("../../fonts/HackNerdFontPropo-Regular.ttf"));
        font_data_propo.tweak.scale = 0.85;
        fonts
            .font_data
            .insert("hack_mono".to_owned(), Arc::new(font_data_mono));
        fonts
            .font_data
            .insert("hack_propo".to_owned(), Arc::new(font_data_propo));
        if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
            family.insert(0, "hack_mono".to_owned());
        }
        if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
            family.insert(0, "hack_propo".to_owned());
        }
        ctx.set_fonts(fonts);
    }

    fn clear_selection_if_opportunity_removed(&mut self, removed_ids: &[String]) {
        if let Selection::Opportunity(op) = &self.selection {
            if removed_ids.iter().any(|id| id == &op.id) {
                #[cfg(debug_assertions)]
                if DF.log_selection || DF.log_ledger {
                    log::info!(
                        "\u{f1238} SELECTED OPPORTUNITY CLEARED because it was removed from ledger, as rare as a rare thing!: {}",
                        op.id
                    );
                }
                self.selection = Selection::Pair(op.pair_name.clone());
            }
        }
    }

    fn ensure_valid_selection(&mut self) {
        let selected_pair: Option<&String> = match &self.selection {
            Selection::Pair(pair) => Some(pair),
            Selection::Opportunity(op) => Some(&op.pair_name),
            Selection::None => None,
        };

        if let Some(pair) = selected_pair {
            if self.valid_session_pairs.contains(pair) {
                return;
            }
        }
        if let Some(pair) = self.valid_session_pairs.iter().next() {
            #[cfg(debug_assertions)]
            if DF.log_selection {
                log::info!(
                    "SELECTION HEALED: no valid selection present; falling back to pair {}",
                    pair
                );
            }
            self.selection = Selection::Pair(pair.clone());
        }
    }

    #[cfg(feature = "ph_audit")]
    pub(crate) fn try_run_audit(&self, ctx: &Context) {
        if let Some(e) = &self.engine {
            let ts_guard = e.timeseries.read().unwrap();
            if ts_guard.series_data.is_empty() {
                return;
            }
            let mut waiting_for_price = false;
            for &pair in AUDIT_PAIRS {
                let has_data = find_matching_ohlcv(
                    &ts_guard.series_data,
                    pair,
                    BASE_INTERVAL.as_millis() as i64,
                )
                .is_ok();

                if has_data {
                    if e.get_price(pair).is_none() {
                        waiting_for_price = true;
                        break;
                    }
                }
            }
            if waiting_for_price {
                ctx.request_repaint();
                return;
            }
            drop(ts_guard);
            println!(">> App State is RUNNING. Ticker & Data Ready. Starting Audit...");
            let mut live_prices = HashMap::new();
            for &pair in crate::ph_audit::AUDIT_PAIRS {
                if let Some(p) = e.get_price(pair) {
                    live_prices.insert(pair.to_string(), p);
                }
            }
            execute_audit(&e.timeseries.read().unwrap(), &live_prices);
        } else {
            log::warn!("Engine not init yet in try_run_audit");
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        setup_custom_visuals(ctx);
        let current = mem::take(&mut self.state);
        self.state = match current {
            AppState::Bootstrapping(mut s) => s.tick(self, ctx),
            AppState::Tuning(mut s) => s.tick(self, ctx),
            AppState::Running(mut s) => s.tick(self, ctx),
        };
    }

    fn save(&mut self, storage: &mut dyn Storage) {
        self.persisted_selection = match &self.selection {
            Selection::None => PersistedSelection::None,
            Selection::Pair(p) => PersistedSelection::Pair(p.clone()),
            Selection::Opportunity(op) => PersistedSelection::Opportunity {
                pair: op.pair_name.clone(),
                opportunity_id: op.id.clone(),
            },
        };
        #[cfg(debug_assertions)]
        if DF.log_selection {
            log::info!(
                "💾 SAVE [App]: PersistedSelection = {:?}",
                self.persisted_selection
            );
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(e) = &self.engine {
            if let Err(err) = save_ledger(&e.engine_ledger) {
                log::error!("Failed to save ledger: {}", err);
            }
        }
        eframe::set_value(storage, eframe::APP_KEY, self);
    }
}

fn setup_custom_visuals(ctx: &Context) {
    let mut visuals = Visuals::dark();
    visuals.window_fill = UI_CONFIG.colors.central_panel;
    visuals.panel_fill = UI_CONFIG.colors.side_panel;
    visuals.widgets.noninteractive.fg_stroke.color = UI_CONFIG.colors.label;
    visuals.widgets.inactive.fg_stroke.color = UI_CONFIG.colors.label;
    visuals.widgets.hovered.fg_stroke.color = UI_CONFIG.colors.heading;
    visuals.widgets.active.fg_stroke.color = UI_CONFIG.colors.heading;
    ctx.set_visuals(visuals);
    ctx.style_mut(|s| s.interaction.selectable_labels = false);
}
