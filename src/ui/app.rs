use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};

use eframe::egui::{
    Align, CentralPanel, Context, FontData, FontDefinitions, FontFamily, Grid, Key, Layout,
    ProgressBar, RichText, ScrollArea, Ui,
};
use eframe::{Frame, Storage};
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

#[cfg(not(target_arch = "wasm32"))]
use {crate::data::ledger_io, std::thread, tokio::runtime::Runtime};

use crate::Cli;

use crate::config::plot::PLOT_CONFIG;

use crate::config::{DF, OptimizationGoal, StationId, TimeTunerConfig, constants};

use crate::data::fetch_pair_data;
use crate::data::timeseries::TimeSeriesCollection;

use crate::engine::SniperEngine;
use crate::engine::worker;

use crate::models::ledger::OpportunityLedger;
use crate::models::trading_view::{NavigationTarget, SortColumn, SortDirection, TradeOpportunity};
use crate::models::{ProgressEvent, SyncStatus, find_matching_ohlcv};

use crate::ui::app_simulation::{SimDirection, SimStepSize};
use crate::ui::config::UI_TEXT;
use crate::ui::ticker::TickerState;
use crate::ui::ui_plot_view::PlotView;
use crate::ui::utils::setup_custom_visuals;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter)]
pub enum CandleResolution {
    M5,
    M15,
    H1,
    H4,
    D1,
    D3,
    W1,
    M1,
}

impl Default for CandleResolution {
    fn default() -> Self {
        Self::D1
    } // Default to 1D candles for plot candles
}

impl fmt::Display for CandleResolution {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::M5 => write!(f, "5m"),
            Self::M15 => write!(f, "15m"),
            Self::H1 => write!(f, "1h"),
            Self::H4 => write!(f, "4h"),
            Self::D1 => write!(f, "1D"),
            Self::D3 => write!(f, "3D"),
            Self::W1 => write!(f, "1W"),
            Self::M1 => write!(f, "1M"),
        }
    }
}

impl CandleResolution {
    pub fn step_size(&self) -> usize {
        match self {
            Self::M5 => 1,
            Self::M15 => 3,  // 3 * 5m
            Self::H1 => 12,  // 12 * 5m
            Self::H4 => 48,  // 48 * 5m
            Self::D1 => 288, // 288 * 5m
            Self::D3 => 288 * 3,
            Self::W1 => 288 * 7,
            Self::M1 => 288 * 30,
        }
    }

    pub fn interval_ms(&self) -> i64 {
        match self {
            Self::M5 => TimeUtils::MS_IN_5_MIN,
            Self::M15 => TimeUtils::MS_IN_15_MIN,
            Self::H1 => TimeUtils::MS_IN_H,
            Self::H4 => TimeUtils::MS_IN_4_H,
            Self::D1 => TimeUtils::MS_IN_D,
            Self::D3 => TimeUtils::MS_IN_3_D,
            Self::W1 => TimeUtils::MS_IN_W,
            Self::M1 => TimeUtils::MS_IN_1_M,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct ZoneSniperApp {
    pub selected_pair: Option<String>,
    pub global_tuner_config: TimeTunerConfig,
    pub station_overrides: HashMap<String, StationId>,
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

    pub saved_strategy: OptimizationGoal,
    pub saved_opportunity_id: Option<String>,

    #[serde(skip)]
    pub selected_opportunity: Option<TradeOpportunity>,
    #[serde(skip)]
    pub active_station_id: StationId,
    #[serde(skip)]
    pub active_ph_pct: f64,
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
    pub simulated_prices: HashMap<String, f64>,
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
        log::info!("when do we init default ZoneSniperApp?");
        #[cfg(debug_assertions)]
        if DF.log_selected_pair {
            log::info!("SELECTED PAIR Init to BTCUSDT");
        }

        Self {
            selected_pair: Some("BTCUSDT".to_string()),
            global_tuner_config: TimeTunerConfig::default(),
            station_overrides: HashMap::new(),
            active_station_id: StationId::default(),
            active_ph_pct: 0.15,
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
            saved_strategy: OptimizationGoal::default(),
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

        // Restore Active Station for the specific startup pair
        if let Some(pair) = &app.selected_pair {
            // Use .copied() to turn &StationId into StationId
            if let Some(saved_station) = app.station_overrides.get(pair).copied() {
                app.active_station_id = saved_station;

                #[cfg(debug_assertions)]
                if DF.log_tuner {
                    log::info!(
                        "🔧 TUNER INIT: Restored saved station '{:?}' for [{}]",
                        saved_station,
                        pair
                    );
                }
            } else {
                // Default if no override exists
                app.active_station_id = StationId::default();
                if DF.log_tuner {
                    #[cfg(debug_assertions)]
                    log::info!(
                        "🔧 TUNER INIT: No save found for [{}]. Using StationId::default() instead",
                        pair
                    );
                }
            }
        } else {
            app.active_station_id = StationId::default();
        }

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
            .map_or(false, |e| e.current_strategy != self.saved_strategy)
        {
            return;
        }

        // 2. Prepare Data (Solve Borrow Checker)
        // We must extract the priority pair string BEFORE mutably borrowing the engine.
        // We don't need 'get_display_price' here because 'trigger_global_recalc'
        // handles price resolution internally (using live price or existing overrides).
        let priority_pair = self.selected_pair.clone();

        // 3. Execute Update
        if let Some(engine) = &mut self.engine {
            // Global Invalidation
            // Since the "Rules of the Game" changed, every pair needs to be re-judged.
            // We pass the current pair as priority so the user sees the active screen update first.
            engine.trigger_global_recalc(priority_pair);
        }
    }

    /// Helper: Runs the Auto-Tune algorithm for a specific pair and station.
    /// Returns Some(new_ph) if successful. Returns None if data/price is missing.
    pub fn run_auto_tune_logic(&self, pair: &str, station_id: StationId) -> Option<f64> {
        if let Some(engine) = &self.engine {
            // 1. Get Config for the requested Station
            let station = self
                .global_tuner_config
                .stations
                .iter()
                .find(|s| s.id == station_id)?;

            // 2. Get Price (Strict Check - must be live)
            let price = engine.price_stream.get_price(pair)?;

            // 3. Get Data
            let ts_guard = engine.timeseries.read().unwrap();
            let ohlcv =
                find_matching_ohlcv(&ts_guard.series_data, pair, constants::INTERVAL_WIDTH_MS)
                    .ok()?;

            // 4. Run Worker Logic
            return worker::tune_to_station(ohlcv, price, station, self.saved_strategy);
        }
        None
    }

    /// Standard Pair Switch (Manual or programmatic).
    /// Updates Global State to point to this pair, but does NOT auto-select a trade.
    pub fn handle_pair_selection(&mut self, new_pair: String) {
        // --- 1. SAVE STATE (Old Pair) ---
        // We only remember which Station (Button) the user was on.
        if let Some(old_pair) = &self.selected_pair {
            // Default to Swing if for some reason nothing is set (Safety)
            let current_station = self.active_station_id;

            self.station_overrides
                .insert(old_pair.clone(), current_station);

            #[cfg(debug_assertions)]
            if DF.log_tuner {
                log::info!(
                    "💾 TUNER  SAVE [{}]: Saved Station '{:?}'",
                    old_pair,
                    current_station
                );
            }
        }

        self.auto_scale_y = true;

        self.selected_pair = Some(new_pair.clone());
        #[cfg(debug_assertions)]
        if DF.log_selected_pair {
            log::info!(
                "SELECTED PAIR: set in handle_pair_selection to {}",
                new_pair
            );
        }

        self.selected_opportunity = None;
        #[cfg(debug_assertions)]
        if DF.log_selected_opportunity {
            log::info!("SELECTED OPPORTUNITY: clear in handle_pair_selection");
        }

        // LOAD STATE for new pair
        let target_station = self
            .station_overrides
            .get(&new_pair)
            .copied() // Converts &StationId to StationId
            .unwrap_or_default();
        // Apply it to the config
        #[cfg(debug_assertions)]
        if DF.log_tuner {
            log::info!(" TUNER: LOAD station [{:?}]", target_station);
        }
        self.active_station_id = target_station;
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
        if let Some(eng) = &self.engine {
            let rows = eng.get_trade_finder_rows(Some(&self.simulated_prices));
            if let Some(row) = rows.into_iter().find(|r| r.pair_name == pair) {
                if let Some(op) = row.opportunity {
                    best_op = Some(op);
                }
            }
        }

        // 3. Routing (The Fork)
        if let Some(op) = best_op {
            // PATH A: Specific Opportunity
            // Delegate to the sniper function to handle context override + highlighting
            self.select_specific_opportunity(op, ScrollBehavior::Center);
        } else {
            // PATH B: Market View
            // Just switch the UI context to this pair.
            // We assume the Engine is already running/tuned for this pair via Startup/Background loop.
            self.handle_pair_selection(pair);
        }

        // 4. Final Polish
        self.update_scroll_to_selection();
    }

    /// Selects a specific opportunity
    pub fn select_specific_opportunity(&mut self, op: TradeOpportunity, scroll: ScrollBehavior) {
        // 1. Switch State (Pure UI)
        self.handle_pair_selection(op.pair_name.clone());

        self.active_ph_pct = op.source_ph;
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
                "SELECTED OPPORTUNITY: set in select_specific_opportunity to {:?}",
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
        if let Some(engine) = &self.engine {
            engine.price_stream.is_suspended()
        } else {
            false
        }
    }

    pub fn mark_all_journeys_stale(&mut self, _reason: &str) {}

    pub fn get_display_price(&self, pair: &str) -> Option<f64> {
        if let Some(sim_price) = self.simulated_prices.get(pair) {
            return Some(*sim_price);
        }
        if let Some(engine) = &self.engine {
            return engine.get_price(pair);
        }
        None
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn toggle_simulation_mode(&mut self) {
        if let Some(engine) = &self.engine {
            let is_sim = !engine.price_stream.is_suspended();
            engine.set_stream_suspended(is_sim);

            if is_sim {
                let all_pairs = engine.get_all_pair_names();
                for pair in all_pairs {
                    if let Some(live_price) = engine.get_price(&pair) {
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
        let current_price = self.get_display_price(&pair).unwrap_or(0.0);
        if current_price <= 0.0 {
            return;
        }

        let change = current_price * percent;
        let new_price = current_price + change;

        self.simulated_prices.insert(pair.clone(), new_price);
    }

    pub(super) fn jump_to_next_zone(&mut self, zone_type: &str) {
        if let Some(engine) = &self.engine {
            let Some(pair) = self.selected_pair.clone() else {
                return;
            };
            let current_price = self.get_display_price(&pair).unwrap_or(0.0);
            let Some(model) = engine.get_model(&pair) else {
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
                    self.simulated_prices.insert(pair, jump_price);
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
                let interval_str = TimeUtils::interval_to_string(constants::INTERVAL_WIDTH_MS);
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
        // Recently added logging to see which panes are slow.

        let start = AppInstant::now();
        if let Some(engine) = &mut self.engine {
            engine.update();
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

        if let Some(engine) = &mut self.engine {
            while processed < chunk_size && !state.todo_list.is_empty() {
                if let Some(pair) = state.todo_list.pop() {
                    // A. Determine Station
                    let station_id = self
                        .station_overrides
                        .get(&pair)
                        .copied()
                        .unwrap_or_default();

                    // B. Get Station Definition
                    if let Some(station_def) = self
                        .global_tuner_config
                        .stations
                        .iter()
                        .find(|s| s.id == station_id)
                    {
                        // C. Tune it
                        let best_ph = {
                            let ts_guard = engine.timeseries.read().unwrap();
                            if let Ok(ohlcv) = find_matching_ohlcv(
                                &ts_guard.series_data,
                                &pair,
                                constants::INTERVAL_WIDTH_MS,
                            ) {
                                if let Some(price) = engine.price_stream.get_price(&pair) {
                                    if price > f64::EPSILON {
                                        worker::tune_to_station(
                                            ohlcv,
                                            price,
                                            station_def,
                                            self.saved_strategy,
                                        )
                                    } else {
                                        if DF.log_tuner {
                                            log::warn!(
                                                "Can't tune because price is {} for {}",
                                                price,
                                                &pair
                                            );
                                        }
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                if DF.log_tuner {
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
                            // let mut pair_config = self.active_ph_pct;
                            // pair_config.threshold_pct = ph;

                            engine.set_price_horizon_override(pair.clone(), self.active_ph_pct);
                            #[cfg(debug_assertions)]
                            if DF.log_tuner {
                                log::info!(
                                    "TUNER: For pair {} setting ph during tuning phase to: {}",
                                    &pair,
                                    ph
                                );
                            }
                            if Some(&pair) == self.selected_pair.as_ref() {
                                #[cfg(debug_assertions)]
                                if DF.log_tuner {
                                    log::info!(
                                        "TUNER: And because pair {} is selected, also setting self.app_config.price_horizon.threshold pct for some reason as well",
                                        &pair
                                    );
                                }
                                self.active_ph_pct = ph;
                            }
                        }
                    }
                    processed += 1;
                }
            }
        }

        // 3. Update State or Finish
        state.completed += processed;

        if state.todo_list.is_empty() {
            // 1. Ignite the Engine (Run CVA + Pathfinder for ALL pairs with new settings)
            if let Some(engine) = &mut self.engine {
                if DF.log_tuner {
                    log::info!(">> Global Tuning Complete. Igniting Engine.");
                }
                engine.trigger_global_recalc(None);
            }

            // 2. EXECUTE SMART SELECTION (Restored Logic)
            // We retrieve the startup pair we stored in check_loading_completion
            if let Some(target_pair) = self.selected_pair.clone() {
                // Force a "New Switch" by clearing selection first (even though we just set it above,
                // this ensures jump_to_pair treats it as a fresh navigation event).
                #[cfg(debug_assertions)]
                if DF.log_selected_pair {
                    log::info!("SELECTED PAIR CLEARing: handle_tuning_phase");
                }
                self.selected_pair = None;
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
                // 1. Get List of ACTUAL loaded pairs
                let available_pairs = timeseries.unique_pair_names();
                let valid_set: HashSet<String> = available_pairs.iter().cloned().collect();

                // 2. Resolve Startup Pair
                // Check if the saved 'selected_pair' actually exists in the loaded data.
                let valid_startup_pair = self
                    .selected_pair
                    .as_ref()
                    .filter(|p| valid_set.contains(*p))
                    .cloned();

                let final_pair = if let Some(p) = valid_startup_pair {
                    p // Saved pair is valid
                } else {
                    // Saved pair is invalid/missing. Fallback to first available.
                    let fallback = available_pairs.first().cloned().unwrap_or_default();
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

                // 3. FORCE UPDATE STATE
                #[cfg(debug_assertions)]
                if DF.log_selected_pair {
                    log::info!("SELECTED PAIR: set to [{:?}] in check_loading_completion", final_pair);
                }
                self.selected_pair = Some(final_pair.clone());

                // 4. Initialize Engine
                let mut engine = SniperEngine::new(timeseries);

                // RESTORE LEDGER
                // If the Nuke Flag is on, we start fresh. Otherwise, we load persistence.
                if DF.wipe_ledger_on_startup {
                    #[cfg(debug_assertions)]
                    log::info!("☢️ LEDGER NUKE: Wiping all historical trades from persistence.");
                    engine.ledger = OpportunityLedger::new();
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
                                engine.ledger = l;
                            }
                            Err(_e) => {
                                #[cfg(debug_assertions)]
                                log::error!("Failed to load ledger (starting fresh): {}", _e);
                                engine.ledger = OpportunityLedger::new();
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        engine.ledger = OpportunityLedger::new();
                    }
                }

                // --- CULL ORPHANS ---
                // Remove opportunities for pairs that were not loaded in this session.
                #[cfg(debug_assertions)]
                let count_before = engine.ledger.opportunities.len();

                engine
                    .ledger
                    .retain(|_id, op| valid_set.contains(&op.pair_name));

                #[cfg(debug_assertions)]
                {
                    let count_after = engine.ledger.opportunities.len();
                    if count_before != count_after {
                        if DF.log_ledger {
                            log::warn!(
                                "STARTUP CLEANUP: Culled {} orphan trades (Data not loaded).",
                                count_before - count_after
                            );
                        }
                    }
                }
                // -------------------------

                self.engine = Some(engine);

                // TEMP this restoration code works but something later appears to overwrite self.selected_opportunity. Not surprising....
                if let Some(id) = &self.saved_opportunity_id {
                    if let Some(op) = self
                        .engine
                        .as_ref()
                        .and_then(|e| e.ledger.opportunities.get(id))
                        .cloned()
                    {
                        self.selected_opportunity = Some(op.clone());
                        #[cfg(debug_assertions)]
                        if DF.log_selected_opportunity {
                            log::info!(
                                "SELECTED OPPORTUNITY: set to {:?} in check_loading_completion",
                                op
                            );
                        }
                        self.selected_pair = Some(op.pair_name.clone());
                        #[cfg(debug_assertions)]
                        if DF.log_selected_pair {
                            log::info!("SELECTED PAIR: set to {:?} in check_loading_completion ", op.pair_name);
                        }

                        self.active_ph_pct = op.source_ph;
                        #[cfg(debug_assertions)]
                        if DF.log_tuner {
                            log::info!("TUNER: set self.active_ph_pct to {:?} in check_loading_completion", op.source_ph);
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

    // --- AUDIT HELPER ---
    #[cfg(feature = "ph_audit")]
    fn try_run_audit(&self, ctx: &Context) {
        if let Some(engine) = &self.engine {
            // 1. ACCESS DATA FIRST
            // We need to know what pairs we actually HAVE before we decide what to wait for.
            let ts_guard = engine.timeseries.read().unwrap();

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
                    if engine.price_stream.get_price(pair).is_none() {
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
                if let Some(p) = engine.price_stream.get_price(pair) {
                    live_prices.insert(pair.to_string(), p);
                }
            }

            let config = self.app_config.clone();
            let ts = engine.timeseries.read().unwrap();

            // Run & Exit
            crate::ph_audit::runner::execute_audit(&ts, &config, &live_prices);
        } else {
            // Engine not initialized yet
        }
    }
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
        if let Some(engine) = &self.engine {
            // Save active ledger to separate binary file
            if let Err(e) = ledger_io::save_ledger(&engine.ledger) {
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
