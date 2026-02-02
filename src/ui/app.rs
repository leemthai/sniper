use std::collections::{BTreeMap, HashMap, HashSet};
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
    CandleResolution, DF, OptimizationStrategy, PhPct, Price, StationId, constants, PriceLike,
};

use crate::data::fetch_pair_data;
use crate::data::timeseries::TimeSeriesCollection;

use crate::engine::SniperEngine;
use crate::engine::worker;

use crate::models::ledger::OpportunityLedger;
use crate::models::trading_view::{NavigationTarget, SortColumn, SortDirection, TradeOpportunity};
use crate::models::{ProgressEvent, SyncStatus, find_matching_ohlcv};

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NavigationState {
    pub current_segment_idx: Option<usize>, // None = Show All
    pub last_viewed_segment_idx: usize,
}

impl Default for NavigationState {
    fn default() -> Self {
        Self {
            current_segment_idx: None,
            last_viewed_segment_idx: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TuningState {
    pub todo_list: Vec<String>,
    pub total: usize,
    pub completed: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
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
pub struct LoadingState {
    pub pairs: BTreeMap<usize, (String, SyncStatus)>,
    pub total_pairs: usize,
    pub completed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PlotVisibility {
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

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct ZoneSniperApp {
    pub selected_pair: Option<String>,
    pub shared_config: SharedConfiguration,
    pub plot_visibility: PlotVisibility,
    pub show_debug_help: bool,
    pub show_ph_help: bool,
    pub candle_resolution: CandleResolution,
    pub show_candle_range: bool,
    pub startup_tune_done: bool,

    // TradeFinder State
    pub tf_scope_match_base: bool, // True = Current Base Pairs, False = All
    pub tf_sort_col: SortColumn,
    pub tf_sort_dir: SortDirection,

    pub saved_strategy: OptimizationStrategy,
    pub saved_opportunity_id: Option<String>,

    #[serde(skip)]
    pub selected_opportunity: Option<TradeOpportunity>,
    #[serde(skip)]
    pub active_station_id: StationId,
    #[serde(skip)]
    pub active_ph_pct: PhPct,
    #[serde(skip)]
    pub scroll_target: Option<NavigationTarget>,
    #[serde(skip)]
    pub engine: Option<SniperEngine>,
    #[serde(skip)]
    pub plot_view: PlotView,
    #[serde(skip)]
    pub state: AppState,
    #[serde(skip)]
    pub progress_rx: Option<Receiver<ProgressEvent>>,
    #[serde(skip)]
    pub data_rx: Option<Receiver<(TimeSeriesCollection, &'static str)>>,
    #[serde(skip)]
    pub sim_step_size: SimStepSize,
    #[serde(skip)]
    pub sim_direction: SimDirection,
    #[serde(skip)]
    pub simulated_prices: HashMap<String, Price>,
    #[serde(skip)]
    pub nav_states: HashMap<String, NavigationState>,
    #[serde(skip)]
    pub auto_scale_y: bool,
    #[serde(skip)]
    pub ticker_state: TickerState,
    #[serde(skip)]
    pub last_frame_time: Option<AppInstant>,
    #[serde(skip)]
    pub show_opportunity_details: bool,
}

impl Default for ZoneSniperApp {
    fn default() -> Self {
        #[cfg(debug_assertions)]
        if DF.log_selected_pair {
            log::info!("SELECTED PAIR Init to BTCUSDT");
        }

        Self {
            selected_pair: Some("BTCUSDT".to_string()),
            shared_config: SharedConfiguration::new(),
            active_station_id: StationId::default(),
            active_ph_pct: PhPct::default(),
            startup_tune_done: false,
            plot_visibility: PlotVisibility::default(),
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
            last_frame_time: None,
            show_opportunity_details: false,
            tf_scope_match_base: false,
            selected_opportunity: None,
            show_candle_range: false,
            tf_sort_col: SortColumn::LiveRoi, // Default to Money
            tf_sort_dir: SortDirection::Descending, // Highest first
            saved_strategy: OptimizationStrategy::default(),
            saved_opportunity_id: None,
        }
    }
}

impl ZoneSniperApp {
    pub fn new(cc: &eframe::CreationContext<'_>, args: Cli) -> Self {
        let mut app: ZoneSniperApp = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Self::default()
        };

        // --- 1. SETUP FONTS ---
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
    pub fn handle_strategy_change(&mut self) {
        // 1. Guard: Check if the strategy ACTUALLY changed.
        if self
            .engine
            .as_ref()
            .map_or(false, |e| e.engine_strategy != self.saved_strategy)
        {
            return;
        }

        // 2. Prepare Data (Solve Borrow Checker)
        // We must extract the priority pair string BEFORE mutably borrowing the engine.
        // We don't need 'get_display_price' here because 'trigger_global_recalc'
        // handles price resolution internally (using live price or existing overrides).
        let priority_pair = self.selected_pair.clone();

        // 3. Execute Update
        if let Some(e) = &mut self.engine {
            // Global Invalidation
            // Since the "Rules of the Game" changed, every pair needs to be re-judged.
            // We pass the current pair as priority so the user sees the active screen update first.
            e.trigger_global_recalc(priority_pair);
        }
    }

    /// Helper: Runs the Auto-Tune algorithm for a specific pair and station.
    /// Returns Some(new_ph) if successful. Returns None if data/price is missing.
    pub fn run_auto_tune_logic(&self, pair: &str, station_id: StationId) -> Option<PhPct> {
        if let Some(e) = &self.engine {
            // 1. Get Config for the requested Station
            let station = constants::tuner::CONFIG
                .stations
                .iter()
                .find(|s| s.id == station_id)?;

            // 2. Get Price (Strict Check - must be live)
            let price = e.price_stream.get_price(pair)?;

            // 3. Get Data
            let ts_guard = e.timeseries.read().unwrap();
            let ohlcv = find_matching_ohlcv(
                &ts_guard.series_data,
                pair,
                constants::BASE_INTERVAL.as_millis() as i64,
            )
            .ok()?;

            // 4. Run Worker Logic
            return worker::tune_to_station(ohlcv, price, station, self.saved_strategy);
        }
        None
    }

    /// Updates Global State to point to this pair, but does NOT auto-select a trade opportunity
    pub fn handle_pair_selection(&mut self, new_pair_name: String) {
        // This does *NOT* update self.opportunity_selection at all!
        // What it does do: (1) update self.selected_pair (2) update self.active_station_id

        self.auto_scale_y = true; // TEMP completely the wrong place for plot code.

        #[cfg(debug_assertions)]
        {
            let old_pair_name = self.selected_pair.replace(new_pair_name.clone());
            if DF.log_selected_pair {
                log::info!(
                    "SELECTED PAIR: set in handle_pair_selection to {} (from {}) ",
                    new_pair_name,
                    old_pair_name.as_deref().unwrap_or("None"),
                );
            }
        }

        self.active_station_id = self
            .shared_config
            .get_station(&new_pair_name)
            .expect(&format!(
                "handle_pair_selection expects a value for station id for pair {}",
                new_pair_name
            )); // Will crash if None. Deliberate choice
        #[cfg(debug_assertions)]
        if DF.log_active_station_id {
            log::info!(
                "🔧 ACTIVE STATION ID SET FROM OVERRIDES: '{:?}' for [{}] in handle_pair_selection",
                self.active_station_id,
                &new_pair_name,
            );
        }
    }

    /// Sets the scroll target based on the current selection state.
    /// Logic: Prefer Opportunity ID -> Fallback to Pair Name.
    pub fn update_scroll_to_selection(&mut self) {
        self.scroll_target = if let Some(op) = &self.selected_opportunity {
            Some(NavigationTarget::Opportunity(op.id.clone()))
        } else {
            self.selected_pair.clone().map(NavigationTarget::Pair)
        };
    }

    /// Smart navigation via Name Click (Ticker, Lists, Startup).
    /// - Checks Ledger for best Op.
    /// - If found: Selects that specific Op.
    /// - If not: Selects the Pair (Market View).
    pub fn jump_to_pair(&mut self, pair: String) {
        // 1. Same Pair Check (Preserve Context)
        if self.selected_pair.as_deref() == Some(&pair) {
            self.update_scroll_to_selection();
            return;
        }

        // 2. Find Best Op (Smart Lookup)
        // We look for an existing opportunity in the engine's current data
        let mut best_op = None;
        if let Some(e) = &self.engine {
            let rows = e.get_trade_finder_rows(Some(&self.simulated_prices));
            if let Some(row) = rows.into_iter().find(|r| r.pair_name == pair) {
                if let Some(op) = row.opportunity {
                    best_op = Some(op);
                }
            }
        }

        // If we have found an opportunity for this pair, go to it. Otherwise just switchto the pair
        if let Some(op) = best_op {
            // PATH A: Specific Opportunity
            // Delegate to the sniper function to handle context override + highlighting
            self.select_specific_opportunity(op, ScrollBehavior::Center, "jump to pair");
        } else {
            // Path B: No opportunity for this pair. So just switch the UI context to this pair.
            self.handle_pair_selection(pair);
        }

        // 4. Final Polish
        self.update_scroll_to_selection();
    }

    /// Selects a specific opportunity
    pub fn select_specific_opportunity(
        &mut self,
        op: TradeOpportunity,
        scroll: ScrollBehavior,
        _reason: &str,
    ) {
        #[cfg(debug_assertions)]
        if DF.log_selected_opportunity {
            log::info!("Call select_specific_opportunity because {}", _reason);
        }
        // 1. Switch State (Pure UI)
        self.handle_pair_selection(op.pair_name.clone());

        self.active_ph_pct = op.source_ph_pct;
        #[cfg(debug_assertions)]
        if DF.log_tuner {
            log::info!(
                "TUNER OVERRIDE PH: ignore the Station's preference and enforce the Trade's origin PH: {}",
                self.active_ph_pct
            );
        }

        // Update Selection
        self.selected_opportunity = Some(op.clone());
        #[cfg(debug_assertions)]
        if DF.log_selected_opportunity {
            log::info!(
                "SELECTED OPPORTUNITY: SET to {} in select_specific_opportunity",
                op
            );
        }

        // Scroll
        if matches!(scroll, ScrollBehavior::Center) {
            self.scroll_target = Some(NavigationTarget::Opportunity(op.id));
        }
    }

    pub fn get_nav_state(&mut self) -> NavigationState {
        let pair = self.selected_pair.clone().unwrap_or("DEFAULT".to_string());
        *self.nav_states.entry(pair).or_default()
    }

    pub fn set_nav_state(&mut self, state: NavigationState) {
        if let Some(pair) = self.selected_pair.clone() {
            self.nav_states.insert(pair, state);
        }
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

    pub fn is_simulation_mode(&self) -> bool {
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

    pub fn mark_all_journeys_stale(&mut self, _reason: &str) {}

    pub fn get_display_price(&self, pair: &str) -> Option<Price> {
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
        let Some(pair) = self.selected_pair.clone() else {
            return;
        };
        let current_price = self.get_display_price(&pair).unwrap_or(Price::new(0.0));
        if !current_price.is_positive() {
            return;
        }

        let change = current_price.value() * percent;
        let new_price = current_price.value() + change;

        self.simulated_prices
            .insert(pair.clone(), Price::new(new_price));
    }

    pub(super) fn jump_to_next_zone(&mut self, zone_type: &str) {
        if let Some(e) = &self.engine {
            let Some(pair) = self.selected_pair.clone() else {
                return;
            };
            let Some(current_price) = self.get_display_price(&pair) else {
                return;
            };
            let Some(model) = e.get_model(&pair) else {
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
                        .filter(|sz| sz.price_center.value() > current_price.value())
                        .min_by(|a, b| a.price_center.partial_cmp(&b.price_center).unwrap()),
                    SimDirection::Down => superzones
                        .iter()
                        .filter(|sz| sz.price_center.value() < current_price.value())
                        .max_by(|a, b| a.price_center.partial_cmp(&b.price_center).unwrap()),
                };

                if let Some(target_zone) = target {
                    let jump_price = match self.sim_direction {
                        SimDirection::Up => target_zone.price_center.value() * 1.0001,
                        SimDirection::Down => target_zone.price_center.value() * 0.9999,
                    };
                    self.simulated_prices.insert(pair, Price::new(jump_price));
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
                let interval_str =
                    TimeUtils::interval_to_string(constants::BASE_INTERVAL.as_millis() as i64);
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
            while processed < chunk_size && !state.todo_list.is_empty() {
                if let Some(pair) = state.todo_list.pop() {
                    // A. Determine Station for each pair

                    let station_id = self.shared_config.get_station(&pair).expect(&format!(
                        "handle_tuning_phase stations should all be full, especially for pair {}",
                        pair
                    ));
                    #[cfg(debug_assertions)]
                    if DF.log_active_station_id {
                        log::info!(
                            "🔧 Reading station_id from app.shared_config '{:?}' for [{}] in handle_tuning_phase()",
                            station_id,
                            &pair,
                        );
                    }
                    // B. Get Station Definition
                    if let Some(station_def) = constants::tuner::CONFIG
                        .stations
                        .iter()
                        .find(|s| s.id == station_id)
                    {
                        // C. Tune it
                        let best_ph = {
                            let ts_guard = e.timeseries.read().unwrap();
                            if let Ok(ohlcv) = find_matching_ohlcv(
                                &ts_guard.series_data,
                                &pair,
                                constants::BASE_INTERVAL.as_millis() as i64,
                            ) {
                                if let Some(price) = e.price_stream.get_price(&pair) {
                                    worker::tune_to_station(
                                        ohlcv,
                                        price,
                                        station_def,
                                        self.saved_strategy,
                                    )
                                } else {
                                    log::warn!(
                                        "Can't tune {} because we don't have a price for it ",
                                        pair
                                    );
                                    None
                                }
                            } else {
                                if DF.log_ph_vals {
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
                            if DF.log_ph_vals {
                                log::info!(
                                    "PH VALS: For pair {} setting shared_config PH during tuning phase to: {}",
                                    &pair,
                                    ph
                                );
                            }
                            if Some(&pair) == self.selected_pair.as_ref() {
                                #[cfg(debug_assertions)]
                                if DF.log_ph_vals {
                                    log::info!(
                                        "PH VALS: And because pair {} is selected, also set self.active_ph_pct to {}",
                                        &pair,
                                        ph,
                                    );
                                }
                                self.active_ph_pct = ph;
                            }
                        } else {
                            #[cfg(debug_assertions)]
                            if DF.log_ph_vals {
                                log::info!(
                                    "There is no ph value set for {} so can't apply result in handle_tuning_phase. But why?",
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

        #[cfg(debug_assertions)]
        if let Some(e) = &self.engine {
            if DF.log_ph_vals {
                log::info!(
                    "Near bottom of handle_tuning_phase we have shared_config PH overrides of {:?}",
                    e.shared_config.get_all_phs()
                );
            }
        }

        if state.todo_list.is_empty() {
            // 1. Ignite the Engine (Run CVA + Pathfinder for ALL pairs with new settings)
            if let Some(e) = &mut self.engine {
                if DF.log_tuner {
                    log::info!(">> Global Tuning Complete. Igniting Engine.");
                }
                e.trigger_global_recalc(None);
            }

            // 2. EXECUTE SMART SELECTION (don't know what that means anymore)
            if let Some(target_pair) = self.selected_pair.clone() {
                #[cfg(debug_assertions)]
                if DF.log_selected_pair {
                    log::info!(
                        "Weird code in handle_tuning_phase.  now just does jump_to_pair() which might be right. Before it did self.selected_pair = None"
                    );
                }
                self.jump_to_pair(target_pair);
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
        // Access rx without borrowing self for long
        if let Some(rx) = &self.data_rx {
            // Non-blocking check
            if let Ok((timeseries, _sig)) = rx.try_recv() {
                let (available_pairs, valid_set, final_pair) =
                    self.resolve_startup_state(&timeseries);

                // Restore active station for the specific startup pair
                self.active_station_id = self
                    .shared_config
                    .get_station(&final_pair.clone())
                    .expect(&format!("check_loading_completion must have station id set for all pairs, including the final_pair which is {} ", final_pair));
                #[cfg(debug_assertions)]
                if DF.log_active_station_id {
                    log::info!(
                        "🔧 SETTING app.active_station_id to '{:?}' for [{}] in check_loading_completion()",
                        self.active_station_id,
                        final_pair,
                    );
                }

                // 4. Initialize Engine with (pointer to) EXACTLY same SharedConfiguration as we have here. Just two pointers to shared memory.
                let mut e = SniperEngine::new(timeseries, self.shared_config.clone());

                // RESTORE LEDGER
                // If the Nuke Flag is on, we start fresh. Otherwise, we load persistence.
                if DF.wipe_ledger_on_startup {
                    #[cfg(debug_assertions)]
                    log::info!("☢️ LEDGER NUKE: Wiping all historical trades from persistence.");
                    e.engine_ledger = OpportunityLedger::new();
                } else {
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
                                e.engine_ledger = l;
                            }
                            Err(_e) => {
                                #[cfg(debug_assertions)]
                                log::error!("Failed to load ledger (starting fresh): {}", _e);
                                e.engine_ledger = OpportunityLedger::new();
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        e.engine_ledger = OpportunityLedger::new();
                    }
                }

                // Remove all opportunities for pairs that were not loaded in this session.
                let _count_before = e.engine_ledger.opportunities.len();

                e.engine_ledger
                    .retain(|_id, op| valid_set.contains(&op.pair_name));

                #[cfg(debug_assertions)]
                {
                    let count_after = e.engine_ledger.opportunities.len();
                    if _count_before != count_after {
                        if DF.log_ledger {
                            log::warn!(
                                "STARTUP CLEANUP: Culled {} orphan trades (Data not loaded).",
                                _count_before - count_after
                            );
                        }
                    }
                }
                // -------------------------

                self.engine = Some(e);

                if let Some(id) = &self.saved_opportunity_id {
                    if let Some(op) = self
                        .engine
                        .as_ref()
                        .and_then(|e| e.engine_ledger.opportunities.get(id))
                        .cloned()
                    {
                        // We have found the saved opportunity. However, since selected_pair is king, we must decide if this preserved opportunity is from this pair or not.

                        // So first thing decide is this opportunity from selected pair........
                        if self.selected_pair.as_deref() == Some(&op.pair_name) {
                            // Saved opportunity IS from self.selected_pair, we can just slot it in.
                            self.selected_opportunity = Some(op.clone());
                            #[cfg(debug_assertions)]
                            if DF.log_selected_opportunity {
                                log::info!(
                                    "SELECTED OPPORTUNITY SET to {} in check_loading_completion as start-up value because pair_name in opportunity matches selected_pair so straight rehydration is possible.",
                                    op
                                );
                            }
                        } else {
                            #[cfg(debug_assertions)]
                            if DF.log_selected_opportunity {
                                log::info!(
                                    "CAN'T RESTORE OPPORTUNITY FROM STATE because pair name mismatch. Gonna have to pick best opportunity from selected_pair instead (TODO)"
                                );
                            }
                            // If, however, saved opportunity IS NOT from self.selected_pair, we must instead find a new opportunity from self.selected_pair
                            // How to find best opportunity in ledger ? TEMP for start, just pick one quick ..... this code is very rare anyway.
                            if let Some(e) = self.engine.as_ref() {
                                self.selected_opportunity = e
                                    .engine_ledger
                                    .find_first_for_pair(self.selected_pair.clone())
                                    .cloned();
                                #[cfg(debug_assertions)]
                                if DF.log_selected_opportunity {
                                    log::info!(
                                        "SELECTED_OPPORTUNIY SET to {} in check_loading_completion as start-up value because we must always get a opportunity that matches self.selected_pair which is {:?}",
                                        self.selected_opportunity
                                            .as_ref()
                                            .map(|op| op.id.as_str())
                                            .unwrap_or("None"),
                                        &self.selected_pair
                                    );
                                }
                            }
                        }

                        // TEMP no idea what to do with this shitcode yet.
                        self.active_ph_pct = op.source_ph_pct;
                        #[cfg(debug_assertions)]
                        if DF.log_tuner {
                            log::info!(
                                "TUNER: set self.active_ph_pct to {:?} in check_loading_completion as start-up value",
                                op.source_ph_pct
                            );
                        }
                    }
                } else {
                    // Saved_opportunity_id is None. So again, just find best op in ledger, similar to above?
                    if let Some(e) = self.engine.as_ref() {
                        self.selected_opportunity = e
                            .engine_ledger
                            .find_first_for_pair(self.selected_pair.clone())
                            .cloned();
                        #[cfg(debug_assertions)]
                        if DF.log_selected_opportunity {
                            log::info!(
                                "SELECTED_OPPORTUNIY SET to {} in check_loading_completion as start-up value because saved_opportunity_id is ({:?}). Therefore we must get an opportunity that matches self.selected_pair which is {:?}",
                                self.selected_opportunity
                                    .as_ref()
                                    .map(|op| op.id.as_str())
                                    .unwrap_or("None"),
                                self.saved_opportunity_id,
                                &self.selected_pair
                            );
                        }
                    }
                }

                // 5. Reset Navigation
                self.nav_states
                    .insert(final_pair, NavigationState::default());

                // 6. TRANSITION TO TUNING PHASE
                // We create a todo list of ALL pairs to tune them before the app starts.
                let all_pairs: Vec<String> = available_pairs.into_iter().collect();

                return Some(AppState::Tuning(TuningState {
                    total: all_pairs.len(),
                    completed: 0,
                    todo_list: all_pairs,
                }));
            }
        }
        None
    }

    /// Helper: Resolves the startup pair and initializes station overrides.
    fn resolve_startup_state(
        &mut self,
        timeseries: &TimeSeriesCollection,
    ) -> (Vec<String>, HashSet<String>, String) {
        // Get List of ACTUAL loaded pairs
        let available_pairs = timeseries.unique_pair_names();
        let valid_set: HashSet<String> = available_pairs.iter().cloned().collect();

        // 1. Register pairs in shared config - ensures all pairs write
        self.shared_config.register_pairs(available_pairs.clone());

        // Resolve Startup Pair - and check if the saved 'selected_pair' actually exists in the loaded data.
        let valid_startup_pair = self
            .selected_pair
            .as_ref()
            .filter(|p| valid_set.contains(*p))
            .cloned();

        // Ensure self.station_overrides is setup with at least default value for all valid pairs.
        #[cfg(debug_assertions)]
        if DF.log_station_overrides {
            log::info!(
                "Pre initialization of app.shared_config was {:?}",
                self.shared_config
            );
        }
        self.shared_config.ensure_all_stations_initialized();
        self.shared_config
            .ensure_all_phs_initialized(PhPct::default());

        #[cfg(debug_assertions)]
        if DF.log_station_overrides {
            log::info!(
                "Post intiialization app.shared_config is {:?}",
                self.shared_config
            );
        }
        let start_pair = if let Some(p) = valid_startup_pair {
            p // Saved pair is valid
        } else {
            // Saved pair is invalid/missing. Fallback to first available.
            let fallback = available_pairs
                .first()
                .cloned()
                .expect("Can't run app without at least one asset");
            if DF.log_selected_pair {
                #[cfg(debug_assertions)]
                log::warn!(
                    "STARTUP FIX: Saved pair {:?} not found in loaded data. Falling back to {}.",
                    self.selected_pair,
                    fallback
                );
            }
            fallback
        };
        #[cfg(debug_assertions)]
        if DF.log_selected_pair {
            log::info!(
                "SELECTED PAIR: set to [{:?}] in check_loading_completion",
                start_pair
            );
        }
        self.selected_pair = Some(start_pair.clone()); // Guaranteed to have a selected_pair after this

        (available_pairs, valid_set, start_pair)
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
pub fn setup_custom_visuals(ctx: &Context) {
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
        // Update the serializable ID from the runtime "fat" object
        self.saved_opportunity_id = self.selected_opportunity.as_ref().map(|op| op.id.clone());

        #[cfg(debug_assertions)]
        if DF.log_selected_opportunity {
            match &self.saved_opportunity_id {
                Some(id) => log::info!("💾 SAVE [App]: Persisting Opportunity ID [{}]", id),
                None => log::info!("💾 SAVE [App]: No opportunity selected (None)"),
            }
        }

        // 1. Snapshot the Engine Ledger
        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
        if let Some(e) = &self.engine {
            // Save active ledger to separate binary file
            if let Err(e) = ledger_io::save_ledger(&e.engine_ledger) {
                log::error!("Failed to save ledger: {}", e);
            }
        }
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        setup_custom_visuals(ctx);

        let mut next_state = None;

        // --- FIX: GLOBAL CURSOR STRATEGY ---
        // Disable text selection globally.
        // This stops the I-Beam cursor appearing on labels/buttons unless it is a text edit box. THis also prevents text from being selectable
        ctx.style_mut(|s| s.interaction.selectable_labels = false);

        // --- PHASE A: LOADING STATE ---
        // We use a scope block to limit the borrow of 'self.state'
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
