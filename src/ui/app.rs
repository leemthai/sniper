use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};

use eframe::egui::{
    Align, CentralPanel, Context, FontData, FontDefinitions, FontFamily, Grid, Key, Layout,
    ProgressBar, RichText, ScrollArea, Ui, Visuals,
};
use eframe::{Frame, Storage};
use serde::{Deserialize, Serialize};
// use strum_macros::EnumIter;

#[cfg(not(target_arch = "wasm32"))]
use {crate::data::ledger_io, std::thread, tokio::runtime::Runtime};

use crate::Cli;

use crate::config::plot::PLOT_CONFIG;

use crate::config::{
    BASE_INTERVAL, CandleResolution, PhPct, Price, PriceLike, StationId, TUNER_CONFIG,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::config::Pct;

use crate::config::DF;

use crate::data::fetch_pair_data;
use crate::data::timeseries::TimeSeriesCollection;

use crate::engine::SniperEngine;
use crate::engine::worker;

use crate::models::TradeOpportunity;
use crate::models::ledger::OpportunityLedger;
use crate::models::{
    NavigationTarget, ProgressEvent, SortColumn, SortDirection, SyncStatus, find_matching_ohlcv,
};

use crate::shared::SharedConfiguration;

use crate::ui::app_simulation::{SimDirection, SimStepSize};
use crate::ui::config::{UI_CONFIG, UI_TEXT};
use crate::ui::ticker::TickerState;
use crate::ui::ui_plot_view::PlotView;

use crate::utils::TimeUtils;
use crate::utils::time_utils::AppInstant;

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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TuningState {
    pub todo_list: Vec<String>,
    pub total: usize,
    pub completed: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AppState {
    Loading(LoadingState),
    Tuning(TuningState),
    Running,
}

impl Default for AppState {
    fn default() -> Self {
        Self::Loading(LoadingState::default())
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct LoadingState {
    pub pairs: BTreeMap<usize, (String, SyncStatus)>,
    pub total_pairs: usize,
    pub completed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct PlotVisibility {
    pub sticky: bool,
    pub low_wicks: bool,
    pub high_wicks: bool,
    pub background: bool,
    pub price_line: bool,
    pub candles: bool,
    pub opportunities: bool,

    pub horizon_lines: bool,
    pub separators: bool,
}

impl Default for PlotVisibility {
    fn default() -> Self {
        Self {
            sticky: true,
            low_wicks: false,
            high_wicks: false,
            background: true,
            price_line: true,
            candles: true,
            opportunities: true,
            horizon_lines: true,
            separators: true,
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
pub enum PersistedSelection {
    None,
    Pair(String),
    Opportunity {
        pair: String,
        opportunity_id: String,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct ZoneSniperApp {
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
    pub(crate) state: AppState,
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

impl Default for ZoneSniperApp {
    fn default() -> Self {
        #[cfg(debug_assertions)]
        if DF.log_selected_pair {
            log::info!("SELECTED PAIR Init to BTCUSDT");
        }

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

impl ZoneSniperApp {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>, args: Cli) -> Self {
        let mut app: ZoneSniperApp = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Self::default()
        };

        Self::configure_fonts(&cc.egui_ctx);

        app.plot_view = PlotView::new();
        app.simulated_prices = HashMap::new();
        app.state = AppState::Loading(LoadingState::default());

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

    /// Runs the Auto-Tune algorithm for a specific pair and station.
    /// Returns Some(new_ph) if successful. Returns None if data/price is missing.
    pub(crate) fn run_auto_tune_logic(&self, pair: &str, station_id: StationId) -> Option<PhPct> {
        if let Some(e) = &self.engine {
            // 1. Get Config for the requested Station
            let station = TUNER_CONFIG.stations.iter().find(|s| s.id == station_id)?;

            // 2. Get Price (Strict Check - must be live)
            let price = e.price_stream.get_price(pair)?;

            // 3. Get Data
            let ts_guard = e.timeseries.read().unwrap();
            let ohlcv = find_matching_ohlcv(
                &ts_guard.series_data,
                pair,
                BASE_INTERVAL.as_millis() as i64,
            )
            .ok()?;

            // 4. Run Worker Logic
            return worker::tune_to_station(
                ohlcv,
                price,
                station,
                self.shared_config.get_strategy(),
            );
        }
        None
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
        if DF.log_selected_opportunity {
            log::info!("Call select_opportunity because {}", _reason);
        }

        // Single source of truth
        self.selection = Selection::Opportunity(op.clone());

        #[cfg(debug_assertions)]
        if DF.log_selected_opportunity {
            log::info!("SELECTION SET to Opportunity {} in select_opportunity", op);
        }

        // Scroll
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
    pub(super) fn toggle_simulation_mode(&mut self) {
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

    pub(super) fn adjust_simulated_price_by_percent(&mut self, percent: f64) {
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

    pub(super) fn jump_to_next_zone(&mut self, zone_type: &str) {
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

    pub(super) fn handle_global_shortcuts(&mut self, ctx: &Context) {
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

    fn render_loading_grid(ui: &mut Ui, state: &LoadingState) {
        ScrollArea::vertical().show(ui, |ui| {
            Grid::new("loading_grid_multi_col")
                .striped(true)
                .spacing([20.0, 10.0])
                .min_col_width(250.0)
                .show(ui, |ui| {
                    for (i, (_idx, (pair, status))) in state.pairs.iter().enumerate() {
                        // Determine Color/Text based on Status
                        let (color, status_text, status_color) = match status {
                            SyncStatus::Pending => (
                                PLOT_CONFIG.color_text_subdued,
                                "-".to_string(),
                                PLOT_CONFIG.color_text_subdued,
                            ),
                            SyncStatus::Syncing => (
                                PLOT_CONFIG.color_warning,
                                UI_TEXT.ls_syncing.to_string(),
                                PLOT_CONFIG.color_warning,
                            ),
                            SyncStatus::Completed(n) => (
                                PLOT_CONFIG.color_text_primary,
                                format!("+{}", n),
                                PLOT_CONFIG.color_profit,
                            ),
                            SyncStatus::Failed(_) => (
                                PLOT_CONFIG.color_loss,
                                UI_TEXT.ls_failed.to_string(),
                                PLOT_CONFIG.color_loss,
                            ),
                        };

                        // Render Cell
                        ui.horizontal(|ui| {
                            ui.set_min_width(240.0);
                            ui.label(RichText::new(pair).strong().color(color));

                            // Clean Layout syntax using imports
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                match status {
                                    SyncStatus::Syncing => {
                                        ui.spinner();
                                    }
                                    SyncStatus::Completed(_) => {
                                        // We use status_text ("+500") here
                                        ui.label(RichText::new(status_text).color(status_color));
                                    }
                                    _ => {
                                        ui.label(RichText::new(status_text).color(status_color));
                                    }
                                }
                            });
                        });

                        // New row every 3 items
                        if (i + 1) % 3 == 0 {
                            ui.end_row();
                        }
                    }
                });
        });
    }

    fn render_loading_screen(ctx: &Context, state: &LoadingState) {
        CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);

                // Title
                ui.heading(
                    RichText::new(&UI_TEXT.ls_title)
                        .size(24.0)
                        .strong()
                        .color(PLOT_CONFIG.color_warning),
                );

                // Subtitle / Info
                let interval_str = TimeUtils::interval_to_string(BASE_INTERVAL.as_millis() as i64);
                ui.label(
                    RichText::new(format!(
                        "{} {} {}",
                        UI_TEXT.ls_syncing, interval_str, UI_TEXT.ls_main,
                    ))
                    .italics()
                    .color(PLOT_CONFIG.color_text_neutral),
                );

                ui.add_space(20.0);

                // Progress Bar Logic
                let total = state.total_pairs;
                let done = state.completed + state.failed;
                let progress = if total > 0 {
                    done as f32 / total as f32
                } else {
                    0.0
                };

                ui.add_space(20.0);
                ui.add(
                    ProgressBar::new(progress)
                        .show_percentage()
                        .animate(true)
                        .text(format!("Processed {}/{}", done, total)),
                );

                // Failure Warning
                if state.failed > 0 {
                    ui.add_space(5.0);
                    ui.label(
                        RichText::new(format!(
                            "{} {} {}",
                            UI_TEXT.label_warning, state.failed, UI_TEXT.label_failures
                        ))
                        .color(PLOT_CONFIG.color_loss),
                    );
                }

                ui.add_space(20.0);
            });

            // Call the Grid Helper we made earlier
            Self::render_loading_grid(ui, state);
        });
    }

    fn render_running_state(&mut self, ctx: &Context) {
        let start = AppInstant::now();
        if let Some(e) = &mut self.engine {
            e.update();
        }
        let engine_time = start.elapsed().as_micros();

        self.handle_global_shortcuts(ctx);

        self.render_top_panel(ctx); // Render before left/right if we want to occupy full app screen space

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

        // LOGGING results. Adjust threshold to catch the slowdown
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

    fn update_loading_progress(state: &mut LoadingState, rx_opt: &Option<Receiver<ProgressEvent>>) {
        if let Some(rx) = rx_opt {
            while let Ok(event) = rx.try_recv() {
                // FIX: Insert using Index as Key (usize), and Tuple as Value
                state.pairs.insert(event.index, (event.pair, event.status));
            }

            state.total_pairs = state.pairs.len();

            // FIX: Destructure the tuple |(_, s)| to access the Status
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

    /// Writes a value into each self.shared_config.ph_overrides for each pair (for current station_id)
    /// HUGE issue for this function is that tuning is *not* done unless we have a price for the pair. And often that fails
    /// So we get through without doing the tuning and just use default (i.e untuned) PH values to run the initial phase (not good)
    fn handle_tuning_phase(&mut self, ctx: &Context, mut state: TuningState) -> AppState {
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

        // 2. Process a Chunk (Batch of 5)
        let chunk_size = 5;
        let mut processed = 0;

        if let Some(e) = &mut self.engine {
            // Wait for connection health to be good enough (50%) before continuing
            #[cfg(not(target_arch = "wasm32"))]
            e.price_stream.wait_for_health_threshold(Pct::new(0.5));

            while processed < chunk_size && !state.todo_list.is_empty() {
                if let Some(pair) = state.todo_list.pop() {
                    // A. Determine Station for each pair
                    let station_id = self.shared_config.get_station(&pair).unwrap_or_else(|| panic!("handle_tuning_phase stations should all be full, especially for pair {}", pair));
                    #[cfg(debug_assertions)]
                    if DF.log_station_overrides {
                        log::info!(
                            "🔧 READING station_overrides from app.shared_config '{:?}' for [{}] in handle_tuning_phase()",
                            station_id,
                            &pair,
                        );
                    }
                    // B. Get Station Definition
                    if let Some(station_def) =
                        TUNER_CONFIG.stations.iter().find(|s| s.id == station_id)
                    {
                        // C. Tune it
                        let best_ph = {
                            let ts_guard = e.timeseries.read().unwrap();
                            if let Ok(ohlcv) = find_matching_ohlcv(
                                &ts_guard.series_data,
                                &pair,
                                BASE_INTERVAL.as_millis() as i64,
                            ) {
                                if let Some(price) = e.price_stream.get_price(&pair) {
                                    worker::tune_to_station(
                                        ohlcv,
                                        price,
                                        station_def,
                                        e.shared_config.get_strategy(),
                                    )
                                } else {
                                    log::warn!(
                                        "Can't tune {} because we don't have a price for it ",
                                        pair
                                    );
                                    None
                                }
                            } else {
                                if DF.log_ph_overrides {
                                    log::warn!(
                                        "Can't tune because we have no matching_ohlcv for {}",
                                        &pair
                                    );
                                }
                                None
                            }
                        };

                        // D. Apply Result
                        if let Some(ph) = best_ph {
                            e.shared_config.insert_ph(pair.clone(), ph);
                            #[cfg(debug_assertions)]
                            if DF.log_ph_overrides {
                                log::info!(
                                    "WRITING ph value {} for pair {} during tuning phase",
                                    &pair,
                                    ph
                                );
                            }
                        } else {
                            #[cfg(debug_assertions)]
                            if DF.log_ph_overrides {
                                log::info!(
                                    "No ph value set yet for {} so can't apply result in handle_tuning_phase. But why? Just means best_ph never set.
                                    Can happen if phase above fails for any reason  e.g. no price obtained. So this means we can get through this function without having tuned",
                                    pair
                                )
                            }
                        }
                    }
                    processed += 1;
                }
            }
        } else {
            log::warn!("No engine. Not good");
        }

        // 3. Update State or Finish
        state.completed += processed;

        if state.todo_list.is_empty() {
            // Ignite the Engine (Run CVA + Pathfinder for ALL pairs with new settings)
            if let Some(e) = &mut self.engine {
                if DF.log_tuner {
                    log::info!(">> Global Tuning Complete. Igniting Engine.");
                }
                e.trigger_global_recalc(None);
            }

            // Re-run jump_to_pair() for current selection to sync UI with fresh engine state
            if let Some(target_pair) = self.selection.pair() {
                self.jump_to_pair(target_pair.to_string());
            }
            AppState::Running
        } else {
            ctx.request_repaint();
            AppState::Tuning(state)
        }
    }

    /// Helper: Checks if the background thread has finished.
    /// Returns Some(NewState) if ready to transition.
    fn check_loading_completion(&mut self) -> Option<AppState> {
        if let Some(rx) = &self.data_rx {
            if let Ok((timeseries, _sig)) = rx.try_recv() {
                // Phase 1: derive valid pairs + shared config
                self.initialize_pair_state(&timeseries);

                // Phase 2: engine + ledger
                let mut e = SniperEngine::new(
                    timeseries,
                    self.shared_config.clone(),
                    self.valid_session_pairs.iter().cloned().collect(),
                );

                e.engine_ledger = Self::restore_engine_ledger(&self.valid_session_pairs);
                self.engine = Some(e);

                // Phase 3: restore persisted selection EXACTLY
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
                        if let Some(e) = &self.engine {
                            if let Some(op) = e.engine_ledger.opportunities.get(opportunity_id) {
                                Selection::Opportunity(op.clone())
                            } else if self.valid_session_pairs.contains(pair) {
                                // Opportunity expired → fall back to its pair ONLY
                                Selection::Pair(pair.clone())
                            } else {
                                Selection::None
                            }
                        } else {
                            Selection::None
                        }
                    }
                };

                // Phase 4: final fallback if NOTHING is selected
                if matches!(self.selection, Selection::None) {
                    if let Some(pair) = self.valid_session_pairs.iter().next().cloned() {
                        self.selection = Selection::Pair(pair);
                    }
                }

                // Reset Navigation to use selected Pair
                if let Some(pair) = self.selection.pair_owned() {
                    self.nav_states.insert(pair, NavigationState::default());
                }

                // TRANSITION TO TUNING PHASE
                return Some(AppState::Tuning(TuningState {
                    total: self.valid_session_pairs.len(),
                    completed: 0,
                    todo_list: self
                        .valid_session_pairs
                        .iter()
                        .cloned()
                        .collect::<Vec<String>>(),
                }));
            }
        }
        None
    }

    // fn check_loading_completion_old(&mut self) -> Option<AppState> {
    //     // Access rx without borrowing self for long
    //     if let Some(rx) = &self.data_rx {
    //         // Non-blocking check
    //         if let Ok((timeseries, _sig)) = rx.try_recv() {
    //             self.initialize_pair_state(&timeseries);

    //             // Initialize Engine
    //             let mut e = SniperEngine::new(
    //                 timeseries,
    //                 self.shared_config.clone(),
    //                 self.valid_session_pairs.iter().cloned().collect(),
    //             );

    //             e.engine_ledger = Self::restore_engine_ledger(&self.valid_session_pairs);
    //             self.engine = Some(e);

    //             if let Some(id) = &self.persisted_opportunity_id {
    //                 if let Some(op) = self
    //                     .engine
    //                     .as_ref()
    //                     .and_then(|e| e.engine_ledger.opportunities.get(id))
    //                     .cloned()
    //                 {
    //                     // Persisted opportunity still exists → restore it directly.
    //                     // Its pair becomes the effective selected pair.
    //                     self.selection = Selection::Opportunity(op.clone());

    //                     #[cfg(debug_assertions)]
    //                     if DF.log_selected_opportunity {
    //                         log::info!(
    //                             "SELECTED OPPORTUNITY RESTORED to {} in check_loading_completion",
    //                             op.id
    //                         );
    //                     }
    //                 } else {
    //                     // Persisted opportunity ID no longer valid → fall back to best opportunity
    //                     // for the currently resolved selected pair.
    //                     let selection = self.selection.clone();

    //                     if let (Some(e), Selection::Pair(pair)) = (&self.engine, selection) {
    //                         if let Some(op) =
    //                             e.engine_ledger.find_first_for_pair(Some(pair)).cloned()
    //                         {
    //                             self.selection = Selection::Opportunity(op.clone());

    //                             #[cfg(debug_assertions)]
    //                             if DF.log_selected_opportunity {
    //                                 log::info!(
    //                                     "SELECTED OPPORTUNITY SET to {} in check_loading_completion (persisted ID invalid)",
    //                                     op.id
    //                                 );
    //                             }
    //                         }
    //                     }
    //                 }
    //             } else {
    //                 // NOTE: If persisted_opportunity_id is None → intentionally do nothing.
    //                 // We remain in Selection::Pair(_)

    //                 // // No persisted opportunity → pick best opportunity for current selection (if any)
    //                 // let selection = self.selection.clone();

    //                 // if let (Some(e), Selection::Pair(pair)) = (&self.engine, selection) {
    //                 //     if let Some(op) = e.engine_ledger.find_first_for_pair(Some(pair)).cloned() {
    //                 //         self.selection = Selection::Opportunity(op.clone());

    //                 //         #[cfg(debug_assertions)]
    //                 //         if DF.log_selected_opportunity {
    //                 //             log::info!(
    //                 //                 "SELECTED OPPORTUNITY SET to {} in check_loading_completion because persisted_opportunity_id is None",
    //                 //                 op.id
    //                 //             );
    //                 //         }
    //                 //     }
    //                 // }
    //             }

    //             // Reset Navigation to use selected Pair
    //             if let Some(pair) = self.selection.pair_owned() {
    //                 self.nav_states.insert(pair, NavigationState::default());
    //             }

    //             // TRANSITION TO TUNING PHASE
    //             return Some(AppState::Tuning(TuningState {
    //                 total: self.valid_session_pairs.len(),
    //                 completed: 0,
    //                 todo_list: self
    //                     .valid_session_pairs
    //                     .iter()
    //                     .cloned()
    //                     .collect::<Vec<String>>(),
    //             }));
    //         }
    //     }

    //     None
    // }

    /// Returns a fully-initialized OpprtuntyLedger (including startup-culling against valid_session_pairs)
    fn restore_engine_ledger(valid_session_pairs: &HashSet<String>) -> OpportunityLedger {
        // If the Nuke Flag is on, we start fresh.
        if DF.wipe_ledger_on_startup {
            #[cfg(debug_assertions)]
            log::info!("☢️ LEDGER NUKE: Wiping all historical trades from persistence.");
            return OpportunityLedger::new();
        }

        // Otherwise attempt to load persistence
        let mut ledger = {
            #[cfg(not(target_arch = "wasm32"))]
            {
                match ledger_io::load_ledger() {
                    Ok(l) => {
                        #[cfg(debug_assertions)]
                        if DF.log_ledger {
                            log::info!(
                                "Loaded ledger with {} opportunities",
                                l.opportunities.len()
                            );
                        }
                        l
                    }
                    Err(_e) => {
                        #[cfg(debug_assertions)]
                        log::error!("Failed to load ledger (starting fresh): {}", _e);
                        OpportunityLedger::new()
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                OpportunityLedger::new()
            }
        };

        // Remove all opportunities for pairs that were not loaded in this session.
        let _count_before = ledger.opportunities.len();

        #[cfg(debug_assertions)]
        if DF.log_ledger {
            log::info!("The valid start-up set is {:?}", valid_session_pairs);
        }

        ledger.retain(|_id, op| valid_session_pairs.contains(&op.pair_name));

        #[cfg(debug_assertions)]
        {
            if DF.log_ledger {
                for op in ledger.opportunities.values() {
                    debug_assert!(
                        valid_session_pairs.contains(&op.pair_name),
                        "Ledger contains invalid pair AFTER retain: {}",
                        op.pair_name
                    );
                }
            }
        }

        #[cfg(debug_assertions)]
        {
            let count_after = ledger.opportunities.len();
            if _count_before != count_after && DF.log_ledger {
                log::info!(
                    "START-UP CLEANUP: Culled {} orphan trades (Data not loaded).",
                    _count_before - count_after
                );
            }
        }

        ledger
    }

    /// Helper: Resolves
    /// valid_session_pairs
    /// shared_config station + PH initialization
    fn initialize_pair_state(&mut self, timeseries: &TimeSeriesCollection) {
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
}

impl eframe::App for ZoneSniperApp {
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
        if DF.log_selected_opportunity {
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

    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        setup_custom_visuals(ctx);

        let mut next_state = None;

        // --- FIX: GLOBAL CURSOR STRATEGY ---
        // Disable text selection globally. This stops the I-Beam cursor appearing on labels/buttons unless it is a text edit box. THis also prevents text from being selectable
        ctx.style_mut(|s| s.interaction.selectable_labels = false);

        // --- PHASE A: LOADING STATE --- We use a scope block to limit the borrow of 'self.state'
        {
            if let AppState::Loading(state) = &mut self.state {
                // 1. Update Progress (Pass fields individually to avoid conflict)
                Self::update_loading_progress(state, &self.progress_rx);
                ctx.request_repaint(); // Keep animating progress bar
            }
        }

        // 2. Check Completion (requires &mut self)
        // Only run this check if we are currently loading
        if matches!(self.state, AppState::Loading(_)) {
            next_state = self.check_loading_completion();
            // log::warn!("WHERE DOES CHECK_LOADING_COMPLETION ARRIVE IN THE PROCESS?");
        }

        // --- PHASE B: TRANSITION ---
        if let Some(new_state) = next_state {
            self.state = new_state;
            ctx.request_repaint();
            return;
        }

        match &self.state {
            AppState::Loading(state) => {
                Self::render_loading_screen(ctx, state);
            }
            AppState::Tuning(tuning_state) => {
                self.state = self.handle_tuning_phase(ctx, tuning_state.clone());
            }
            AppState::Running => {
                #[cfg(feature = "ph_audit")]
                self.try_run_audit(ctx);
                self.render_running_state(ctx);
            }
        }
    }
}
