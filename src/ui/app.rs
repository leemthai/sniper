use std::collections::HashMap;
// use std::sync::Arc;

use eframe::egui::Context;
use eframe::{App, Frame, Storage};
use serde::{Deserialize, Serialize};

// use crate::config::ANALYSIS;
use crate::domain::auto_duration;
use crate::engine::SniperEngine;
use crate::models::cva::ScoreType;
use crate::ui::app_simulation::{SimDirection, SimStepSize};
use crate::ui::ui_plot_view::PlotView;
use crate::ui::utils::setup_custom_visuals;

/// Persistent visibility settings for the plot
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PlotVisibility {
    pub sticky: bool,
    pub low_wicks: bool,
    pub high_wicks: bool,
}

impl Default for PlotVisibility {
    fn default() -> Self {
        Self {
            sticky: true,
            low_wicks: true,
            high_wicks: true,
        }
    }
}

fn default_time_horizon_days() -> u64 {
    7
}

/// The Main Application State
#[derive(Deserialize, Serialize)]
#[serde(default)] // Use Default implementation for missing fields during deserialization
pub struct ZoneSniperApp {
    // --- 1. User Interface State (Persisted) ---
    pub selected_pair: Option<String>,
    pub plot_visibility: PlotVisibility,

    // --- 2. Runtime Components (Skipped) ---
    #[serde(skip)]
    pub engine: Option<SniperEngine>, // Option allows deferred initialization if needed

    #[serde(skip)]
    pub plot_view: PlotView,

    #[serde(skip)]
    pub show_debug_help: bool,

    // --- 3. Debug / Simulation State (Skipped) ---
    #[serde(skip)]
    pub debug_background_mode: ScoreType,

    #[serde(skip)]
    pub is_simulation_mode: bool,
    #[serde(skip)]
    pub simulated_prices: HashMap<String, f64>,
    #[serde(skip)]
    pub sim_direction: SimDirection,
    #[serde(skip)]
    pub sim_step_size: SimStepSize,

    // RESTORED: These are UI state for the sliders.
    // They act as "Proposed Config" for the next calculation.
    #[serde(default)]
    pub auto_duration_config: auto_duration::AutoDurationConfig,

    #[serde(default = "default_time_horizon_days")]
    pub time_horizon_days: u64,
}

impl Default for ZoneSniperApp {
    fn default() -> Self {
        Self {
            selected_pair: Some("BTCUSDT".to_string()),
            plot_visibility: PlotVisibility::default(),

            // Initialize Configs
            auto_duration_config: crate::domain::auto_duration::AutoDurationConfig::default(),
            time_horizon_days: default_time_horizon_days(),

            engine: None, // Must be injected after creation
            plot_view: PlotView::new(),
            show_debug_help: false,

            debug_background_mode: ScoreType::FullCandleTVW,
            is_simulation_mode: false,
            simulated_prices: HashMap::new(),
            sim_direction: SimDirection::Up,
            sim_step_size: SimStepSize::Point1,
        }
    }
}

impl ZoneSniperApp {
    /// Create the app and inject the Engine (called from main.rs)
    pub fn new(cc: &eframe::CreationContext<'_>, engine: SniperEngine) -> Self {
        // 1. Load state from disk if available
        let mut app: ZoneSniperApp = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Self::default()
        };

        // 2. Inject the Engine (The Brain)
        app.engine = Some(engine);

        // 3. Re-initialize non-persisted components
        app.plot_view = PlotView::new();
        app.simulated_prices = HashMap::new();

        app
    }

    /// Called when the user selects a new pair in the side panel.
    pub fn handle_pair_selection(&mut self, new_pair: String) {
        self.selected_pair = Some(new_pair.clone());

        // Notify Engine to prioritize this pair (move to front of queue)
        if let Some(engine) = &mut self.engine {
            engine.force_recalc(&new_pair);
        }
    }

    /// Called when a global setting (like Auto-Duration) changes.
    /// Forces a recalculation of ALL pairs.
    pub fn invalidate_all_pairs_for_global_change(&mut self, reason: &str) {
        log::info!("Global invalidation triggered: {}", reason);
        if let Some(engine) = &mut self.engine {
            let all_pairs = engine.get_all_pair_names();
            for pair in all_pairs {
                engine.force_recalc(&pair);
            }
        }
    }

    /// Placeholder for Journey logic (Suspended for now).
    pub fn mark_all_journeys_stale(&mut self, _reason: &str) {
        // No-op until Journey system is ported to Engine architecture
    }

    /// Proxy to get signals from the Engine's Monitor.
    pub fn get_signals(&self) -> Vec<&crate::models::pair_context::PairContext> {
        if let Some(engine) = &self.engine {
            engine.get_signals()
        } else {
            Vec::new()
        }
    }

    /// Helper to get the display price (Simulated or Live)
    pub fn get_display_price(&self, pair: &str) -> Option<f64> {
        if self.is_simulation_mode {
            return self.simulated_prices.get(pair).copied();
        }

        // Ask the Engine for the live price
        if let Some(engine) = &self.engine {
            return engine.get_price(pair);
        }

        None
    }
}

impl App for ZoneSniperApp {
    fn save(&mut self, storage: &mut dyn Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        setup_custom_visuals(ctx);

        // 1. Update the Engine (The Game Loop)
        if let Some(engine) = &mut self.engine {
            engine.update();
        }

        // 2. Handle Inputs
        self.handle_global_shortcuts(ctx);

        // 3. Render
        self.render_side_panel(ctx);
        self.render_central_panel(ctx);
        self.render_status_panel(ctx);

        if self.show_debug_help {
            self.render_help_panel(ctx);
        }
    }
}
