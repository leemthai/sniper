use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::collections::BTreeMap;

use eframe::egui::{Context, CentralPanel, RichText, Key};
use eframe::{Frame, Storage};
use serde::{Deserialize, Serialize};

use crate::Cli;
use crate::config::ANALYSIS;
use crate::config::AnalysisConfig;
use crate::config::PriceHorizonConfig;
use crate::engine::SniperEngine;
use crate::models::pair_context::PairContext;
use crate::data::timeseries::TimeSeriesCollection;
use crate::ui::ui_plot_view::PlotView;
use crate::ui::utils::setup_custom_visuals;
use crate::ui::app_simulation::SimStepSize;
use crate::ui::app_simulation::SimDirection;
use crate::models::cva::ScoreType;
use crate::models::{SyncStatus, ProgressEvent};


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
    pub support: bool,
    pub resistance: bool,
    pub low_wicks: bool,
    pub high_wicks: bool,
    pub pivot_lines: bool,
    pub background: bool,
    pub price_line: bool,
}

impl Default for PlotVisibility {
    fn default() -> Self {
        Self {
            sticky: true,
            support: true,
            resistance: true,
            low_wicks: false,
            high_wicks: false,
            pivot_lines: false,
            background: true,
            price_line: true,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(default)] 
pub struct ZoneSniperApp {
    pub selected_pair: Option<String>,
    pub app_config: AnalysisConfig,
    pub price_horizon_overrides: HashMap<String, PriceHorizonConfig>,
    pub plot_visibility: PlotVisibility,
    pub show_debug_help: bool,
    pub show_ph_help: bool,
    pub debug_background_mode: ScoreType,

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
}

impl Default for ZoneSniperApp {
    fn default() -> Self {
        Self {
            selected_pair: Some("BTCUSDT".to_string()),
            app_config: ANALYSIS.clone(),
            price_horizon_overrides: HashMap::new(),
            plot_visibility: PlotVisibility::default(),
            show_debug_help: false,
            show_ph_help: false,
            debug_background_mode: ScoreType::FullCandleTVW, 
            engine: None,
            plot_view: PlotView::new(),
            state: AppState::default(),
            progress_rx: None,
            data_rx: None,
            sim_step_size: SimStepSize::default(),
            sim_direction: SimDirection::default(),
            simulated_prices: HashMap::new(),
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

        app.plot_view = PlotView::new();
        app.simulated_prices = HashMap::new();
        app.state = AppState::Loading(LoadingState::default());

        let (data_tx, data_rx) = std::sync::mpsc::channel();
        let (prog_tx, prog_rx) = std::sync::mpsc::channel();
        
        app.data_rx = Some(data_rx);
        app.progress_rx = Some(prog_rx);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let args_clone = args.clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
                rt.block_on(async move {
                    let (data, sig) = crate::data::fetch_pair_data(
                        300, 
                        &args_clone, 
                        Some(prog_tx)
                    ).await;

                    let _ = data_tx.send((data, sig));
                });
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            let _ = prog_tx;
            let args_clone = args.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let (data, sig) = crate::data::fetch_pair_data(0, &args_clone, None).await;
                let _ = data_tx.send((data, sig));
            });
        }

        app
    }

    pub fn is_simulation_mode(&self) -> bool {
        if let Some(engine) = &self.engine {
            engine.price_stream.is_suspended()
        } else {
            false
        }
    }

    pub fn handle_pair_selection(&mut self, new_pair: String) {
        if let Some(old_pair) = &self.selected_pair {
            let old_config = self.app_config.price_horizon.clone();
            self.price_horizon_overrides.insert(old_pair.clone(), old_config.clone());

            if let Some(engine) = &mut self.engine {
                engine.set_price_horizon_override(old_pair.clone(), old_config.clone());
            }
        }

        self.selected_pair = Some(new_pair.clone());

        if let Some(saved_config) = self.price_horizon_overrides.get(&new_pair) {
            let mut config = saved_config.clone();
            config.min_threshold_pct = ANALYSIS.price_horizon.min_threshold_pct;
            config.max_threshold_pct = ANALYSIS.price_horizon.max_threshold_pct;
            config.threshold_pct = config.threshold_pct.clamp(config.min_threshold_pct, config.max_threshold_pct);
            self.app_config.price_horizon = config;
        } else {
            self.app_config.price_horizon = ANALYSIS.price_horizon.clone();
        }

        let price = self.get_display_price(&new_pair);
        if let Some(engine) = &mut self.engine {
            engine.update_config(self.app_config.clone());
            engine.set_price_horizon_override(
                new_pair.clone(),
                self.app_config.price_horizon.clone(),
            );
            engine.force_recalc(&new_pair, price);
        }
    }

    pub fn invalidate_all_pairs_for_global_change(&mut self, _reason: &str) {
        if let Some(pair) = self.selected_pair.clone() {
            let price = self.get_display_price(&pair);
            let new_config = self.app_config.price_horizon.clone();
            self.price_horizon_overrides.insert(pair.clone(), new_config.clone());
            
            if let Some(engine) = &mut self.engine {
                engine.update_config(self.app_config.clone());
                engine.set_price_horizon_override(pair.clone(), new_config);
                engine.force_recalc(&pair, price);
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
        let Some(pair) = self.selected_pair.clone() else { return; };
        let current_price = self.get_display_price(&pair).unwrap_or(0.0);
        if current_price <= 0.0 { return; }
        
        let change = current_price * percent;
        let new_price = current_price + change;
        
        self.simulated_prices.insert(pair.clone(), new_price);
    }
    
    pub(super) fn jump_to_next_zone(&mut self, zone_type: &str) {
        if let Some(engine) = &self.engine {
             let Some(pair) = self.selected_pair.clone() else { return; };
             let current_price = self.get_display_price(&pair).unwrap_or(0.0);
             let Some(model) = engine.get_model(&pair) else { return; };
             
             let superzones = match zone_type {
                "sticky" => Some(&model.zones.sticky_superzones),
                "low-wick" => Some(&model.zones.low_wicks_superzones),
                "high-wick" => Some(&model.zones.high_wicks_superzones),
                _ => None,
             };
             
             if let Some(superzones) = superzones {
                 if superzones.is_empty() { return; }
                 
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
        ctx.input(|i| {
            if i.key_pressed(Key::Num1) { self.plot_visibility.sticky = !self.plot_visibility.sticky; }
            if i.key_pressed(Key::Num2) { self.plot_visibility.support = !self.plot_visibility.support; self.plot_visibility.resistance = !self.plot_visibility.resistance; }
            if i.key_pressed(Key::Num3) { self.plot_visibility.low_wicks = !self.plot_visibility.low_wicks; self.plot_visibility.high_wicks = !self.plot_visibility.high_wicks; }
            if i.key_pressed(Key::H) { self.show_debug_help = !self.show_debug_help; }
            if i.key_pressed(Key::Escape) { self.show_debug_help = false; self.show_ph_help = false; }
            if i.key_pressed(Key::B) {
                if !self.plot_visibility.background {
                    self.plot_visibility.background = true;
                    self.debug_background_mode = ScoreType::FullCandleTVW;
                } else {
                    // Was ON -> Cycle Modes
                    match self.debug_background_mode {
                        ScoreType::FullCandleTVW => self.debug_background_mode = ScoreType::LowWickCount,
                        ScoreType::LowWickCount => self.debug_background_mode = ScoreType::HighWickCount,
                        ScoreType::HighWickCount => {
                            // End of cycle -> Turn OFF
                            self.plot_visibility.background = false;
                        }
                        _ => self.debug_background_mode = ScoreType::FullCandleTVW,
                    }
                }
            }
            if i.key_pressed(Key::S) { self.toggle_simulation_mode(); }
            if i.key_pressed(Key::Num4) { self.jump_to_next_zone("sticky"); }
            if i.key_pressed(Key::Num5) { self.jump_to_next_zone("low-wick"); }
            if i.key_pressed(Key::Num6) { self.jump_to_next_zone("high-wick"); }
            if i.key_pressed(Key::D) { 
                 self.sim_direction = match self.sim_direction { SimDirection::Up => SimDirection::Down, SimDirection::Down => SimDirection::Up };
            }
            if i.key_pressed(Key::X) { self.sim_step_size.cycle(); }
            if i.key_pressed(Key::A) { 
                 let percent = self.sim_step_size.as_percentage();
                 let adj = match self.sim_direction { SimDirection::Up => percent, SimDirection::Down => -percent };
                 self.adjust_simulated_price_by_percent(adj);
            }
        });
    }

}

impl eframe::App for ZoneSniperApp {
    fn save(&mut self, storage: &mut dyn Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        setup_custom_visuals(ctx);

        // --- 1. HANDLE LOADING UPDATES ---
        if let AppState::Loading(state) = &mut self.state {
            // Check Progress
            if let Some(rx) = &self.progress_rx {
                while let Ok(event) = rx.try_recv() {
                    // Store by INDEX to preserve order
                    state.pairs.insert(event.index, (event.pair, event.status));
                    
                    state.total_pairs = state.pairs.len();
                    state.completed = state.pairs.values().filter(|(_, s)| matches!(s, SyncStatus::Completed(_))).count();
                    state.failed = state.pairs.values().filter(|(_, s)| matches!(s, SyncStatus::Failed(_))).count();
                    
                    ctx.request_repaint();
                }
            }

            // Check Completion
            if let Some(rx) = &self.data_rx {
                if let Ok((timeseries, _sig)) = rx.try_recv() {
                    let mut engine = SniperEngine::new(timeseries);
                    engine.update_config(self.app_config.clone());
                    engine.set_all_overrides(self.price_horizon_overrides.clone());
                    engine.trigger_global_recalc(self.selected_pair.clone());
                    self.engine = Some(engine);
                    self.state = AppState::Running;
                    ctx.request_repaint();
                    return; // Transition immediately
                }
            }
        }

        // --- 2. RENDER LOADING SCREEN ---
        if let AppState::Loading(state) = &self.state {
            CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.heading(RichText::new("ZONE SNIPER INITIALIZATION").size(24.0).strong().color(eframe::egui::Color32::from_rgb(255, 215, 0))); // Gold title
                    // NEW: Sub-header with Interval Context
                    let interval_str = crate::utils::TimeUtils::interval_to_string(ANALYSIS.interval_width_ms);
                    ui.label(RichText::new(format!(
                        "Syncing {} klines from Binance Public API. Please be patient. This may take some time if it hasn't been run for a while or you are collecting many pairs. Subsequent runs will complete much quicker.", 
                        interval_str
                    )).italics().color(eframe::egui::Color32::LIGHT_GRAY));

                    ui.add_space(20.0);


                    // Progress Bar
                    let total = state.pairs.len();
                    let done = state.completed + state.failed;
                    let progress = if total > 0 { done as f32 / total as f32 } else { 0.0 };
                    
                    ui.add_space(20.0);
                    ui.add(eframe::egui::ProgressBar::new(progress)
                        .show_percentage()
                        .animate(true)
                        .text(format!("Processed {}/{}", done, total))
                    );
                    
                    if state.failed > 0 {
                        ui.add_space(5.0);
                        ui.label(RichText::new(format!("⚠ {} Failures", state.failed)).color(eframe::egui::Color32::RED));
                    }
                    
                    ui.add_space(20.0);
                });

                // 3-COLUMN GRID LAYOUT
// 3-COLUMN GRID LAYOUT
                eframe::egui::ScrollArea::vertical().show(ui, |ui| {
                    eframe::egui::Grid::new("loading_grid_multi_col")
                        .striped(true)
                        .spacing([20.0, 10.0])
                        .min_col_width(250.0) 
                        .show(ui, |ui| {
                            
                            for (i, (_idx, (pair, status))) in state.pairs.iter().enumerate() {
                                
                                // Determine Color based on Status
                                let (color, status_text, status_color) = match status {
                                    crate::models::SyncStatus::Pending => (
                                        eframe::egui::Color32::from_gray(80), // Dimmed Gray for Queue
                                        "-".to_string(), 
                                        eframe::egui::Color32::from_gray(80)
                                    ),
                                    crate::models::SyncStatus::Syncing => (
                                        eframe::egui::Color32::YELLOW, // Bright for Active
                                        "Syncing".to_string(), 
                                        eframe::egui::Color32::YELLOW
                                    ),
                                    crate::models::SyncStatus::Completed(n) => (
                                        eframe::egui::Color32::WHITE, // Normal for Done
                                        format!("+{}", n),
                                        eframe::egui::Color32::GREEN
                                    ),
                                    crate::models::SyncStatus::Failed(_) => (
                                        eframe::egui::Color32::RED, 
                                        "FAILED".to_string(), 
                                        eframe::egui::Color32::RED
                                    ),
                                };

                                // Render Cell
                                ui.horizontal(|ui| {
                                    ui.set_min_width(240.0);
                                    ui.label(RichText::new(pair).monospace().strong().color(color));
                                    
                                    ui.with_layout(eframe::egui::Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                                        match status {
                                            crate::models::SyncStatus::Syncing => { ui.spinner(); },
                                            crate::models::SyncStatus::Completed(n) => { 
                                                ui.label(RichText::new(format!("✔ (+{})", n)).color(status_color)); 
                                            },
                                            _ => { 
                                                ui.label(RichText::new(status_text).color(status_color)); 
                                            }
                                        }
                                    });
                                });

                                if (i + 1) % 3 == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                });
            });
            return;
        }

        // --- 3. RUNNING STATE ---
        if let Some(engine) = &mut self.engine {
            engine.update();
        }

        self.handle_global_shortcuts(ctx);
        self.render_side_panel(ctx);
        self.render_central_panel(ctx);
        self.render_status_panel(ctx);
        self.render_help_panel(ctx);
        
        ctx.request_repaint();
    }
}