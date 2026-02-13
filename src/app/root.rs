use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::{Arc, mpsc, mpsc::Receiver},
};


use eframe::{
    egui::{
        CentralPanel, Context, FontData, FontDefinitions, FontFamily, 
        Key, ProgressBar, Visuals,
    },
    Frame, Storage,
};


#[cfg(not(target_arch = "wasm32"))]
use {crate::data::ledger_io, std::thread, tokio::runtime::Runtime};

use crate::Cli;

use crate::app::{
    phases::phase_view::PhaseView,
    state::{AppState, BootstrapState, RunningState, TuningState},
};
use crate::models::SyncStatus;

use crate::config::{CandleResolution, PhPct, Price, PriceLike, DF};
#[cfg(not(target_arch = "wasm32"))]
use crate::config::Pct;

use crate::data::fetch_pair_data;
use crate::data::timeseries::TimeSeriesCollection;

use crate::engine::SniperEngine;

use crate::models::{restore_engine_ledger, ProgressEvent, TradeOpportunity};

use crate::shared::SharedConfiguration;

use crate::ui::{
    app_simulation::{SimDirection, SimStepSize},
    config::UI_CONFIG,
    screens::bootstrap,
    ticker::TickerState,
    ui_plot_view::{PlotView, PlotVisibility},
    ui_render::{SortColumn, NavigationTarget, NavigationState, ScrollBehavior},
};

use crate::utils::time_utils::AppInstant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    pub fn toggle(&self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Default)]
pub(crate) enum Selection {
    #[default]
    None,
    Pair(String),
    Opportunity(TradeOpportunity),
}

impl fmt::Display for Selection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Selection::None => write!(f, "Selection::None"),
            Selection::Pair(pair_name) => write!(f, "Selection::Pair({})", pair_name),
            Selection::Opportunity(op) => {
                write!(f, "Selection::Opportunity({}, id={})", op.pair_name, op.id)
            }
        }
    }
}

impl Selection {
    /// owned String
    #[inline]
    pub(crate) fn pair_owned(&self) -> Option<String> {
        match self {
            Selection::Pair(p) => Some(p.clone()),
            Selection::Opportunity(op) => Some(op.pair_name.clone()),
            Selection::None => None,
        }
    }

    /// borrowed view
    #[inline]
    pub(crate) fn pair(&self) -> Option<&str> {
        match self {
            Selection::Pair(p) => Some(p),
            Selection::Opportunity(op) => Some(&op.pair_name),
            Selection::None => None,
        }
    }

    pub(crate) fn opportunity(&self) -> Option<&TradeOpportunity> {
        match self {
            Selection::Opportunity(op) => Some(op),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum PersistedSelection {
    None,
    Pair(String),
    Opportunity {
        pair: String,
        opportunity_id: String,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct App {
    pub(crate) shared_config: SharedConfiguration, // This persists across sessions. Contains details of all pairs analysed

    #[serde(skip)]
    pub(crate) selection: Selection,

    #[serde(skip)]
    pub(crate) valid_session_pairs: HashSet<String>, // Valid pairs for this session only - this is passed to the engine

    // Persisted user intent (thin, serializable)
    pub(crate) persisted_selection: PersistedSelection,

    pub(crate) plot_visibility: PlotVisibility,
    pub(crate) show_debug_help: bool,
    pub(crate) show_ph_help: bool,
    pub(crate) candle_resolution: CandleResolution,
    pub(crate) show_candle_range: bool,

    // TradeFinder State
    pub(crate) tf_scope_match_base: bool, // True = Current Base Pairs, False = All
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
    pub(crate) sim_step_size: SimStepSize,
    #[serde(skip)]
    pub(crate) sim_direction: SimDirection,
    #[serde(skip)]
    pub(crate) simulated_prices: HashMap<String, Price>,
    #[serde(skip)]
    pub(crate) nav_states: HashMap<String, NavigationState>,
    #[serde(skip)]
    pub(crate) auto_scale_y: bool,
    #[serde(skip)]
    pub(crate) ticker_state: TickerState,
    #[serde(skip)]
    pub(crate) show_opportunity_details: bool,
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
            sim_step_size: SimStepSize::default(),
            sim_direction: SimDirection::default(),
            simulated_prices: HashMap::new(),
            scroll_target: None,
            nav_states: HashMap::new(),
            candle_resolution: CandleResolution::default(),
            auto_scale_y: true,
            ticker_state: TickerState::default(),
            show_opportunity_details: false,
            tf_scope_match_base: false,
            show_candle_range: false,
            tf_sort_col: SortColumn::LiveRoi, // Default to Money
            tf_sort_dir: SortDirection::Descending, // Highest first
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
        app.simulated_prices = HashMap::new();
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

    /// Handles a change in global strategy (Optimization Goal).
    pub(crate) fn handle_strategy_selection(&mut self) {
        let priority_pair = self.selection.pair_owned();
        if let Some(e) = &mut self.engine {
            // Global Invalidation. Since strategy has changed, every pair needs to be re-judged. We pass the current pair as priority so the user sees the active screen update first.
            e.trigger_global_recalc(priority_pair);
        }
    }

    /// Sets the scroll target based on the current selection state.
    /// Logic: Prefer Opportunity ID -> Fallback to Pair Name.
    pub(crate) fn update_scroll_to_selection(&mut self) {
        self.scroll_target = match &self.selection {
            Selection::Opportunity(op) => Some(NavigationTarget::Opportunity(op.id.clone())),
            Selection::Pair(pair) => Some(NavigationTarget::Pair(pair.clone())),
            Selection::None => None,
        };
    }

    /// Smart navigation via Pair Name (not Opportunity) Click
    /// - Checks Ledger for best Op.
    /// - If found: Selects that specific Op.
    /// - If not: Selects the Pair (Market View).
    pub(crate) fn jump_to_pair(&mut self, pair: String) {
        // 1. Same Pair Check (Preserve Context)
        if matches!(self.selection, Selection::Pair(ref p) if p == &pair) {
            self.update_scroll_to_selection();
            return;
        }

        // 2. Find Best Op (Smart Lookup)
        let best_op = self.engine.as_ref().and_then(|e| {
            e.get_trade_finder_rows(Some(&self.simulated_prices))
                .into_iter()
                .find(|r| r.pair_name == pair)
                .and_then(|r| r.opportunity)
        });

        // 3. Apply Selection
        if let Some(op) = best_op {
            self.select_opportunity(op, ScrollBehavior::Center, "jump to pair");
        } else {
            self.selection = Selection::Pair(pair);
        }

        // 4. Final Polish
        self.update_scroll_to_selection();
    }

    /// Selects a specific opportunity
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

        // Single source of truth
        self.selection = Selection::Opportunity(op.clone());

        #[cfg(debug_assertions)]
        if DF.log_selection {
            log::info!("SELECTION SET to Opportunity {} in select_opportunity", op);
        }

        // Scroll
        if matches!(scroll, ScrollBehavior::Center) {
            self.scroll_target = Some(NavigationTarget::Opportunity(op.id));
        }
    }

    /// Returns the navigation state associated with the currently selected pair, inserting a default if none exists.
    pub(crate) fn get_nav_state(&mut self) -> NavigationState {
        let pair = match &self.selection {
            Selection::Opportunity(op) => op.pair_name.clone(),
            Selection::Pair(pair) => pair.clone(),
            Selection::None => "DEFAULT".to_string(),
        };

        *self.nav_states.entry(pair).or_default()
    }

    /// Stores the given navigation state for the currently selected pair, if one is selected.
    pub(crate) fn set_nav_state(&mut self, state: NavigationState) {
        let pair = match &self.selection {
            Selection::Opportunity(op) => op.pair_name.clone(),
            Selection::Pair(pair) => pair.clone(),
            Selection::None => return, // nowhere to store state
        };

        self.nav_states.insert(pair, state);
    }
    
    /// Ensures a navigation state entry exists for the specified pair, inserting a default if absent.
    pub(crate) fn ensure_nav_state_for_pair(&mut self, pair: &str) {
        self.nav_states.entry(pair.to_owned()).or_default();
    }
    
    pub(crate) fn is_simulation_mode(&self) -> bool {
        // WASM is always offline/simulation
        #[cfg(target_arch = "wasm32")]
        return true;

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(e) = &self.engine {
            e.price_stream.is_suspended()
        } else {
            false
        }
    }

    pub(crate) fn get_display_price(&self, pair: &str) -> Option<Price> {
        if let Some(sim_price) = self.simulated_prices.get(pair) {
            return Some(*sim_price);
        }
        if let Some(e) = &self.engine {
            return e.get_price(pair);
        }
        None
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn toggle_simulation_mode(&mut self) {
        if let Some(e) = &self.engine {
            let is_sim = !e.price_stream.is_suspended();
            e.set_stream_suspended(is_sim);

            if is_sim {
                let all_pairs = e.get_all_pair_names();
                for pair in all_pairs {
                    if let Some(live_price) = e.get_price(&pair) {
                        self.simulated_prices.insert(pair, live_price);
                    }
                }
            } else {
                self.simulated_prices.clear();
            }
        }
    }

    pub(crate) fn adjust_simulated_price_by_percent(&mut self, percent: f64) {
        let Some(pair) = self.selection.pair_owned() else {
            return;
        };
        let current_price = self.get_display_price(&pair).unwrap_or(Price::new(0.0));
        if !current_price.is_positive() {
            return;
        }

        let change = current_price * percent;
        let new_price = current_price + change;

        self.simulated_prices
            .insert(pair.clone(), Price::new(new_price));
    }

    pub(crate) fn jump_to_next_zone(&mut self, zone_type: &str) {
        if let Some(e) = &self.engine {
            let Some(pair) = self.selection.pair() else {
                return;
            };
            let Some(current_price) = self.get_display_price(pair) else {
                return;
            };
            let Some(model) = e.get_model(pair) else {
                return;
            };

            let superzones = match zone_type {
                "sticky" => Some(&model.zones.sticky_superzones),
                "low-wick" => Some(&model.zones.low_wicks_superzones),
                "high-wick" => Some(&model.zones.high_wicks_superzones),
                _ => None,
            };

            if let Some(superzones) = superzones {
                if superzones.is_empty() {
                    return;
                }

                let target = match self.sim_direction {
                    SimDirection::Up => superzones
                        .iter()
                        .filter(|sz| sz.price_center > current_price)
                        .min_by(|a, b| a.price_center.partial_cmp(&b.price_center).unwrap()),
                    SimDirection::Down => superzones
                        .iter()
                        .filter(|sz| sz.price_center < current_price)
                        .max_by(|a, b| a.price_center.partial_cmp(&b.price_center).unwrap()),
                };

                if let Some(target_zone) = target {
                    let jump_price = match self.sim_direction {
                        SimDirection::Up => target_zone.price_center * 1.0001,
                        SimDirection::Down => target_zone.price_center * 0.9999,
                    };
                    self.simulated_prices.insert(pair.to_string(), jump_price);
                }
            }
        }
    }

    pub(crate) fn handle_global_shortcuts(&mut self, ctx: &Context) {
        // FIX: If the user is typing in a text box (wants_keyboard_input),
        // do NOT trigger global hotkeys.
        if ctx.wants_keyboard_input() {
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
                self.show_opportunity_details = false
            }

            // Gate the 'S' key so it only works on Native
            #[cfg(not(target_arch = "wasm32"))]
            if i.key_pressed(Key::S) {
                self.toggle_simulation_mode();
            }

            // Toggle 'O'pportunity Explainer
            if i.key_pressed(Key::O) {
                self.show_opportunity_details = !self.show_opportunity_details;
            }

            // Toggle 'T'ime Machine Panel
            if i.key_pressed(Key::T) {
                self.show_candle_range = !self.show_candle_range;
            }

            if self.is_simulation_mode() {
                if i.key_pressed(Key::Y) {
                    self.jump_to_next_zone("sticky");
                }
                if i.key_pressed(Key::L) {
                    self.jump_to_next_zone("low-wick");
                }
                if i.key_pressed(Key::W) {
                    self.jump_to_next_zone("high-wick");
                }
                if i.key_pressed(Key::D) {
                    self.sim_direction = match self.sim_direction {
                        SimDirection::Up => SimDirection::Down,
                        SimDirection::Down => SimDirection::Up,
                    };
                }
                if i.key_pressed(Key::X) {
                    self.sim_step_size.cycle();
                }
                if i.key_pressed(Key::A) {
                    let percent = self.sim_step_size.as_percentage();
                    let adj = match self.sim_direction {
                        SimDirection::Up => percent,
                        SimDirection::Down => -percent,
                    };
                    self.adjust_simulated_price_by_percent(adj);
                }
            }
        });
    }

    /// Writes a value into each self.shared_config.ph_overrides for each pair (for current station_id)
    pub(crate) fn tick_tuning_state(&mut self, ctx: &Context, state: &mut TuningState) -> AppState {
        // Render tuning progress UI
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

        // Process a small chunk each frame to keep UI responsive
        let chunk_size = 5;
        let mut processed = 0;

        if let Some(e) = &mut self.engine {
            // Temporary: wait for stream health before tuning
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

        // Advance progress
        state.completed += processed;

        if state.todo_list.is_empty() {
            // Trigger full engine recalculation with new PH values
            if let Some(e) = &mut self.engine {
                if DF.log_tuner {
                    log::info!(">> Global Tuning Complete. Igniting Engine.");
                }
                e.trigger_global_recalc(None);
            }

            // Resync UI selection with refreshed engine state
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

        // Modals
        self.render_help_panel(ctx);
        self.render_opportunity_details_modal(ctx);

        // Performance logging
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

    /// Helper: Checks if the background thread has finished.
    /// Returns Some(NewState) if ready to transition.
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

        // 2. Check if backfill completed
        if let Some(next_state) = self.finalize_bootstrap_if_ready() {
            return next_state;
        }

        // 3. Still bootstrapping → render loading UI
        bootstrap::render(ctx, state);

        // Remain in Bootstrapping
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

        // Final fallback: ensure at least one pair is selected
        if matches!(self.selection, Selection::None) {
            if let Some(pair) = self.valid_session_pairs.iter().next().cloned() {
                self.selection = Selection::Pair(pair);
            }
        }

        // Initialize navigation for selected pair
        if let Some(pair) = self.selection.pair_owned() {
            self.ensure_nav_state_for_pair(&pair);
        }
    }

    fn build_engine(&mut self, timeseries: TimeSeriesCollection) {
        // Derive valid pairs + shared config
        self.initialize_pair_state(&timeseries);

        // Construct engine
        let mut engine = SniperEngine::new(
            timeseries,
            self.shared_config.clone(),
            self.valid_session_pairs.iter().cloned().collect(),
        );

        // Restore ledger from persistence
        engine.engine_ledger = restore_engine_ledger(&self.valid_session_pairs);

        self.engine = Some(engine);
    }

    fn initialize_pair_state(&mut self, timeseries: &TimeSeriesCollection) {
        // Helper: Resolves
        // valid_session_pairs
        // shared_config station + PH initialization
        // Actual loaded pairs
        let available_pairs = timeseries.unique_pair_names();
        self.valid_session_pairs = available_pairs.iter().cloned().collect();

        // Register all pairs in shared config
        self.shared_config
            .ensure_all_stations_initialized(&available_pairs);
        self.shared_config
            .ensure_all_phs_initialized(&available_pairs, PhPct::default());
    }

    /// Helper to load and configure custom fonts (Nerd Fonts)
    fn configure_fonts(ctx: &Context) {
        let mut fonts = FontDefinitions::default();

        // 1. Load the MONO Font (For Data/Tables)
        // Keep scale at 0.85 or tweak as needed
        let mut font_data_mono =
            FontData::from_static(include_bytes!("../fonts/HackNerdFont-Regular.ttf"));
        font_data_mono.tweak.scale = 0.85;

        // 2. Load the PROPO Font (For General UI)
        // This is the new file you downloaded
        let mut font_data_propo =
            FontData::from_static(include_bytes!("../fonts/HackNerdFontPropo-Regular.ttf"));
        font_data_propo.tweak.scale = 0.85; // Match scale so they look consistent

        // 3. Register them
        fonts
            .font_data
            .insert("hack_mono".to_owned(), Arc::new(font_data_mono));
        fonts
            .font_data
            .insert("hack_propo".to_owned(), Arc::new(font_data_propo));

        // 4. Prioritize!
        // A. MONOSPACE Family -> Use "hack_mono"
        if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
            family.insert(0, "hack_mono".to_owned());
        }

        // B. PROPORTIONAL Family -> Use "hack_propo"
        if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
            family.insert(0, "hack_propo".to_owned());
        }

        // 5. Apply
        ctx.set_fonts(fonts);
    }

    fn clear_selection_if_opportunity_removed(&mut self, removed_ids: &[String]) {
        // If we have just removed the selected opportunity from the ledger, clear the selection!!
        // TEMP in future, we may announce this via a notification or something "Selected Opportunity has just expired....."
        if let Selection::Opportunity(op) = &self.selection {
            if removed_ids.iter().any(|id| id == &op.id) {
                #[cfg(debug_assertions)]
                if DF.log_selection || DF.log_ledger {
                    log::info!(
                        "\u{f1238} SELECTED OPPORTUNITY CLEARED because it was removed from ledger, as rare as a rare thing!: {}",
                        op.id
                    );
                }
                // Revert to bare pair rather than the opportunity!
                self.selection = Selection::Pair(op.pair_name.clone());
            }
        }
    }

    /// Ensure the UI always has a valid *pair-level* selection while the app is running.
    ///
    /// Invariant:
    /// - While in `Running` state, the UI must never be left without a selected pair.
    /// - A missing or invalid selection causes large parts of the UI to disappear.
    ///
    /// This function is a *healer*, not normal control flow:
    /// - It should do nothing 99.999% of the time.
    /// - It only intervenes if state has become invalid due to startup gaps,
    ///   stale persisted state, or ledger-driven removals.
    fn ensure_valid_selection(&mut self) {
        // FAST PATH:
        // If we already have a selection that refers to a valid session pair,
        // we do nothing.
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

        // HEALING PATH:
        // At this point, either:
        // - Selection is None
        // - Selection refers to a pair that is NOT valid for this session
        //   (e.g. persisted pair missing, or opportunity was pruned)
        //
        // We must recover by selecting *some* valid pair so the UI remains usable.
        if let Some(pair) = self.valid_session_pairs.iter().next() {
            #[cfg(debug_assertions)]
            if DF.log_selection {
                log::info!(
                    "SELECTION HEALED: no valid selection present; falling back to pair {}",
                    pair
                );
            }

            // IMPORTANT:
            // We deliberately fall back to a *pair* selection, NOT an opportunity.
            // Opportunity selection is transient and ledger-driven; pair selection
            // is the stable baseline that keeps the UI alive.
            self.selection = Selection::Pair(pair.clone());
        }
    }

    // --- AUDIT HELPER ---
    #[cfg(feature = "ph_audit")]
    fn try_run_audit(&self, ctx: &Context) {
        if let Some(e) = &self.engine {
            // 1. ACCESS DATA FIRST
            // We need to know what pairs we actually HAVE before we decide what to wait for.
            let ts_guard = e.timeseries.read().unwrap();

            // If data hasn't loaded yet, keep waiting.
            if ts_guard.series_data.is_empty() {
                // println!("Waiting for KLines...");
                return;
            }

            // 2. CHECK TICKER (Smart Wait)
            // Only wait for prices on pairs that actually exist in our KLine data.
            let mut waiting_for_price = false;

            for &pair in crate::ph_audit::config::AUDIT_PAIRS {
                // Check if we have KLines for this pair
                let has_data =
                    find_matching_ohlcv(&ts_guard.series_data, pair, CONSTANTS.interval_width_ms)
                        .is_ok();

                if has_data {
                    // If we have data, we MUST wait for a live price
                    if e.price_stream.get_price(pair).is_none() {
                        waiting_for_price = true;
                        break;
                    }
                }
            }

            // If we are missing a price for a loaded pair, keep pumping the loop
            if waiting_for_price {
                ctx.request_repaint();
                return;
            }

            // 3. EXECUTE
            // We hold the lock from step 1, so we drop it now to allow the runner to use it if needed
            // (though we pass a ref, so dropping is just good hygiene here)
            drop(ts_guard);

            println!(">> App State is RUNNING. Ticker & Data Ready. Starting Audit...");

            // Gather Live Prices (Only for the ones we found)
            let mut live_prices = std::collections::HashMap::new();
            for &pair in crate::ph_audit::config::AUDIT_PAIRS {
                if let Some(p) = e.price_stream.get_price(pair) {
                    live_prices.insert(pair.to_string(), p);
                }
            }

            let config = self.app_config.clone();
            let ts = e.timeseries.read().unwrap();

            // Run & Exit
            crate::ph_audit::runner::execute_audit(&ts, &config, &live_prices);
        } else {
            // Engine not initialized yet
            log::warn!("Engine not init yet in try_run_audit");
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        setup_custom_visuals(ctx);

        // Take ownership of state cleanly
        let current = std::mem::take(&mut self.state);

        self.state = match current {
            AppState::Bootstrapping(mut s) => s.tick(self, ctx),
            AppState::Tuning(mut s) => s.tick(self, ctx),
            AppState::Running(mut s) => s.tick(self, ctx),
        };
    }

    fn save(&mut self, storage: &mut dyn Storage) {
        // Persist user intent, not runtime guesses
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

        // Snapshot the Engine Ledger
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(e) = &self.engine {
            if let Err(err) = ledger_io::save_ledger(&e.engine_ledger) {
                log::error!("Failed to save ledger: {}", err);
            }
        }

        eframe::set_value(storage, eframe::APP_KEY, self);
    }
}

/// Sets up custom visuals for the entire application
fn setup_custom_visuals(ctx: &Context) {
    let mut visuals = Visuals::dark();

    // Customize the dark theme
    visuals.window_fill = UI_CONFIG.colors.central_panel;
    visuals.panel_fill = UI_CONFIG.colors.side_panel;

    // Make the widgets stand out a bit more
    visuals.widgets.noninteractive.fg_stroke.color = UI_CONFIG.colors.label;
    visuals.widgets.inactive.fg_stroke.color = UI_CONFIG.colors.label;
    visuals.widgets.hovered.fg_stroke.color = UI_CONFIG.colors.heading;
    visuals.widgets.active.fg_stroke.color = UI_CONFIG.colors.heading;

    // Set the custom visuals
    ctx.set_visuals(visuals);
    // Disable text selection globally. This stops the I-Beam cursor appearing on labels/buttons unless it is a text edit box.
    // Also prevents any text from being selectable
    ctx.style_mut(|s| s.interaction.selectable_labels = false);
}
