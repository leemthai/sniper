use std::collections::BTreeMap;
use std::collections::HashMap;
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

use crate::Cli;

use crate::config::plot::PLOT_CONFIG;

use crate::config::{ANALYSIS, AnalysisConfig, PriceHorizonConfig};

use crate::data::fetch_pair_data;
use crate::data::timeseries::TimeSeriesCollection;

use crate::engine::SniperEngine;

use crate::models::pair_context::PairContext;
use crate::models::trading_view::{DirectionFilter, SortColumn, SortDirection, TradeOpportunity};
use crate::models::{ProgressEvent, SyncStatus};

use crate::ui::app_simulation::{SimDirection, SimStepSize};
use crate::ui::config::UI_TEXT;
use crate::ui::ticker::TickerState;
use crate::ui::ui_plot_view::PlotView;
use crate::ui::utils::setup_custom_visuals;

use crate::utils::TimeUtils;
use crate::utils::time_utils::AppInstant;

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
pub enum AppState {
    Loading(LoadingState),
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

// NEW: Centralized Logic
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
    pub global_price_horizon: PriceHorizonConfig,
    pub price_horizon_overrides: HashMap<String, PriceHorizonConfig>,
    pub plot_visibility: PlotVisibility,
    pub show_debug_help: bool,
    pub show_ph_help: bool,
    pub candle_resolution: CandleResolution,
    // TradeFinder State
    pub tf_filter_pair_only: bool, // True = Current Pair, False = All
    pub tf_filter_direction: DirectionFilter,
    pub show_candle_range: bool,
    pub tf_sort_col: SortColumn,    // TF Sorting State
    pub tf_sort_dir: SortDirection, // TF Sort Direction

    #[serde(skip)]
    pub selected_opportunity: Option<TradeOpportunity>, // Specific TF selection
    #[serde(skip)]
    pub app_config: AnalysisConfig,
    #[serde(skip)]
    pub scroll_to_pair: Option<String>,
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
        Self {
            selected_pair: Some("BTCUSDT".to_string()),
            app_config: ANALYSIS.clone(),
            price_horizon_overrides: HashMap::new(),
            global_price_horizon: ANALYSIS.price_horizon.clone(),
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
            scroll_to_pair: None,
            nav_states: HashMap::new(),
            candle_resolution: CandleResolution::default(),
            auto_scale_y: true,
            ticker_state: TickerState::default(),
            last_frame_time: None,
            show_opportunity_details: false,
            tf_filter_direction: DirectionFilter::All,
            tf_filter_pair_only: false,
            selected_opportunity: None,
            show_candle_range: false,
            tf_sort_col: SortColumn::LiveRoi, // Default to Money
            tf_sort_dir: SortDirection::Descending, // Highest first
        }
    }
}

impl ZoneSniperApp {
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

    pub fn new(cc: &eframe::CreationContext<'_>, args: Cli) -> Self {
        let mut app: ZoneSniperApp = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Self::default()
        };

        // --- 1. SETUP FONTS ---
        Self::configure_fonts(&cc.egui_ctx);

        // RESTORE STATE:
        // Overwrite the default config's PH with the saved user preference.
        // Everything else in app_config remains as defined in 'const ANALYSIS' (code).
        app.app_config.price_horizon = app.global_price_horizon.clone();

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
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
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

    pub fn handle_pair_selection(&mut self, new_pair: String) {
        // log::info!("UI: handle_pair_selection called for {}", new_pair);

        // 1. Save current config for the OLD pair
        if let Some(old_pair) = &self.selected_pair {
            let old_config = self.app_config.price_horizon.clone();
            self.price_horizon_overrides
                .insert(old_pair.clone(), old_config.clone());

            if let Some(engine) = &mut self.engine {
                engine.set_price_horizon_override(old_pair.clone(), old_config.clone());
            }
        }

        // 2. Set New Pair & Reset View Flags
        self.selected_pair = Some(new_pair.clone());
        // self.scroll_to_pair = true;
        self.auto_scale_y = true;

        // 3. Load config for the NEW pair (or default)
        if let Some(saved_config) = self.price_horizon_overrides.get(&new_pair) {
            let mut config = saved_config.clone();
            // Ensure bounds are respected in case global constants changed
            config.min_threshold_pct = ANALYSIS.price_horizon.min_threshold_pct;
            config.max_threshold_pct = ANALYSIS.price_horizon.max_threshold_pct;
            config.threshold_pct = config
                .threshold_pct
                .clamp(config.min_threshold_pct, config.max_threshold_pct);
            self.app_config.price_horizon = config;
        } else {
            self.app_config.price_horizon = ANALYSIS.price_horizon.clone();
        }

        // 4. Intelligent Update Logic
        // Check if we already have data for this pair to avoid unnecessary recalculation
        let needs_calc = if let Some(engine) = &self.engine {
            engine.get_model(&new_pair).is_none()
        } else {
            true
        };

        let price = self.get_display_price(&new_pair);

        if let Some(engine) = &mut self.engine {
            // Always update the engine's config context so it knows the current PH settings
            engine.update_config(self.app_config.clone());
            engine.set_price_horizon_override(
                new_pair.clone(),
                self.app_config.price_horizon.clone(),
            );

            // Only force a heavy recalc if the model is missing
            if needs_calc {
                engine.force_recalc(&new_pair, price, "USER PAIR SELECTION");
            }
        }

        // --- NEW: AUTO-SELECT BEST OPPORTUNITY ---
        // If the new pair has a valid opportunity, select it automatically.
        // This ensures the Plot HUD and TradeFinder highlight it immediately.
        self.selected_opportunity = None; // Reset first

        if let Some(engine) = &self.engine {
            if let Some(model) = engine.get_model(&new_pair) {
                // Find best by Static ROI (same sort logic as TradeFinder)
                let best = model
                    .opportunities
                    .iter()
                    .filter(|op| op.expected_roi() > 0.0)
                    .max_by(|a, b| a.expected_roi().partial_cmp(&b.expected_roi()).unwrap());

                if let Some(op) = best {
                    self.selected_opportunity = Some(op.clone());
                }
            }
        }
    }

    pub fn invalidate_all_pairs_for_global_change(&mut self, _reason: &str) {
        if let Some(pair) = self.selected_pair.clone() {
            let price = self.get_display_price(&pair);
            let new_config = self.app_config.price_horizon.clone();
            self.price_horizon_overrides
                .insert(pair.clone(), new_config.clone());

            if let Some(engine) = &mut self.engine {
                engine.update_config(self.app_config.clone());
                engine.set_price_horizon_override(pair.clone(), new_config);
                engine.force_recalc(
                    &pair,
                    price,
                    "INVALIDATE ALL PAIRS -> PRICE HORIZON CHANGED",
                );
            }
        }
    }

    pub fn mark_all_journeys_stale(&mut self, _reason: &str) {}

    pub fn get_signals(&self) -> Vec<&PairContext> {
        if let Some(engine) = &self.engine {
            engine.get_signals()
        } else {
            Vec::new()
        }
    }

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
                let interval_str = TimeUtils::interval_to_string(ANALYSIS.interval_width_ms);
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
        if let Some(engine) = &mut self.engine {
            engine.update();
        }
        self.handle_global_shortcuts(ctx);

        self.render_top_panel(ctx); // Render before left/right if we want to occupy full app screen space
        self.render_left_panel(ctx);
        if self.show_candle_range {
            self.render_right_panel(ctx);
        }
        // self.render_trade_finder_panel(ctx);

        self.render_ticker_panel(ctx);
        self.render_status_panel(ctx);
        self.render_central_panel(ctx);

        // Modals
        self.render_help_panel(ctx);
        self.render_opportunity_details_modal(ctx);
    }

    fn update_loading_progress(
        state: &mut crate::ui::app::LoadingState,
        rx_opt: &Option<Receiver<ProgressEvent>>,
    ) {
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

    /// Helper: Checks if the background thread has finished.
    /// Returns Some(NewState) if ready to transition.
    fn check_loading_completion(&mut self) -> Option<AppState> {
        // Access rx without borrowing self for long
        // We need to use 'if let' on the field directly
        if let Some(rx) = &self.data_rx {
            // Non-blocking check
            if let Ok((timeseries, _sig)) = rx.try_recv() {
                // 1. Validate Pair
                let available_pairs = timeseries.unique_pair_names();
                let current_is_valid = self
                    .selected_pair
                    .as_ref()
                    .map(|p| available_pairs.contains(p))
                    .unwrap_or(false);

                if !current_is_valid {
                    if let Some(first) = available_pairs.first() {
                        #[cfg(debug_assertions)]
                        log::warn!(
                            "Startup: Persisted pair {:?} not found. Switching to {}",
                            self.selected_pair,
                            first
                        );
                        self.selected_pair = Some(first.clone());
                    }
                }

                // 2. Initialize Engine
                let mut engine = SniperEngine::new(timeseries);
                engine.update_config(self.app_config.clone());
                engine.set_all_overrides(self.price_horizon_overrides.clone());
                engine.trigger_global_recalc(self.selected_pair.clone());

                self.engine = Some(engine);
                self.scroll_to_pair = self.selected_pair.clone();

                // 3. Reset Navigation
                if let Some(pair) = &self.selected_pair {
                    self.nav_states
                        .insert(pair.clone(), NavigationState::default());
                }

                return Some(AppState::Running);
            }
        }
        None
    }
}

impl eframe::App for ZoneSniperApp {
    fn save(&mut self, storage: &mut dyn Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        // self.check_performance_monitor();
        setup_custom_visuals(ctx);

        let mut next_state = None;

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

        // --- PHASE C: RENDER ---
        match &self.state {
            AppState::Loading(state) => {
                Self::render_loading_screen(ctx, state);
            }
            AppState::Running => {
                self.render_running_state(ctx);
            }
        }
    }
}
