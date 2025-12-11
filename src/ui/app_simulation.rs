use super::app::ZoneSniperApp;
use crate::config::DEBUG_FLAGS;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SimDirection {
    Up,
    Down,
}

impl fmt::Display for SimDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimDirection::Up => write!(f, "▲ PRICE UP"),
            SimDirection::Down => write!(f, "▼ PRICE DOWN"),
        }
    }
}

impl Default for SimDirection {
    fn default() -> Self {
        Self::Up
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SimStepSize {
    Point1, // 0.1%
    Point5, // 0.5%
    One,    // 1%
    Five,   // 5%
    Ten,    // 10%
}

impl SimStepSize {
    pub(super) fn as_percentage(&self) -> f64 {
        match self {
            SimStepSize::Point1 => 0.001,
            SimStepSize::Point5 => 0.005,
            SimStepSize::One => 0.01,
            SimStepSize::Five => 0.05,
            SimStepSize::Ten => 0.10,
        }
    }

    pub(super) fn cycle(&mut self) {
        *self = match self {
            SimStepSize::Point1 => SimStepSize::Point5,
            SimStepSize::Point5 => SimStepSize::One,
            SimStepSize::One => SimStepSize::Five,
            SimStepSize::Five => SimStepSize::Ten,
            SimStepSize::Ten => SimStepSize::Point1,
        };
    }
}

impl Default for SimStepSize {
    fn default() -> Self {
        Self::Point1
    }
}

impl fmt::Display for SimStepSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1}%", self.as_percentage() * 100.0)
    }
}

impl ZoneSniperApp {
    pub(super) fn toggle_simulation_mode(&mut self) {
        self.is_simulation_mode = !self.is_simulation_mode;
        let is_sim = self.is_simulation_mode;

        // 1. Tell Engine to Suspend/Resume live updates
        if let Some(engine) = &self.engine {
            engine.set_stream_suspended(is_sim);

            // 2. If Entering Sim Mode, Snapshot current price
            if is_sim {
                let all_pairs = engine.get_all_pair_names();

                for pair in all_pairs {
                    if let Some(live_price) = engine.get_price(&pair) {
                            self.simulated_prices.insert(pair, live_price);
                        }
                    }
            }
        }

        if self.is_simulation_mode {
            if cfg!(debug_assertions) && DEBUG_FLAGS.print_simulation_events {
                log::info!("Entered Simulation Mode");
            }
        } else {
            // Clearing simulated prices effectively resets them to live on next fetch
            self.simulated_prices.clear();
            if cfg!(debug_assertions) && DEBUG_FLAGS.print_simulation_events {
                log::info!("Exited Simulation Mode");
            }
        }
    }

    pub(super) fn adjust_simulated_price_by_percent(&mut self, percent: f64) {
        let Some(pair) = self.selected_pair.clone() else {
            return;
        };

        // 1. Calculate new price
        // We must rely on get_display_price to handle the fallback logic
        let current_price = self.get_display_price(&pair).unwrap_or(0.0);
        if current_price == 0.0 {
            return;
        }

        let new_price = current_price * (1.0 + percent);
        self.simulated_prices.insert(pair.clone(), new_price);

        // 2. Notify Monitor (Optional: if we want signals to update in Sim mode)
        // Since we removed direct access to multi_pair_monitor, we skip this for now.
        // The PlotView will update automatically because it reads `current_pair_price`.

        if cfg!(debug_assertions) && DEBUG_FLAGS.print_simulation_events {
            log::info!(
                "Simulated Price Change: {} -> {:.2} ({:+.2}%)",
                pair,
                new_price,
                percent * 100.0
            );
        }
    }

    pub(super) fn jump_to_next_zone(&mut self, zone_type: &str) {
        let Some(pair) = self.selected_pair.clone() else {
            return;
        };

        // 1. Get Model from Engine
        let Some(engine) = &self.engine else {
            return;
        };
        let Some(model) = engine.get_model(&pair) else {
            return;
        };

        let current_price = self.get_display_price(&pair).unwrap_or(0.0);

        // 2. Select Zone List
        // Note: Using Arc<TradingModel> fields directly
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

            // 3. Find Next Target
            let target_zone = match self.sim_direction {
                SimDirection::Up => superzones
                    .iter()
                    .filter(|sz| sz.price_center > current_price)
                    .min_by(|a, b| a.price_center.partial_cmp(&b.price_center).unwrap()),
                SimDirection::Down => superzones
                    .iter()
                    .filter(|sz| sz.price_center < current_price)
                    .max_by(|a, b| a.price_center.partial_cmp(&b.price_center).unwrap()),
            };

            // 4. Move Price
            if let Some(zone) = target_zone {
                // Jump slightly past the center to ensure we are "in" it or clearly past previous
                let new_price = zone.price_center;
                self.simulated_prices.insert(pair.clone(), new_price);

                if cfg!(debug_assertions) && DEBUG_FLAGS.print_simulation_events {
                    log::info!("Jumped to {} zone at {:.2}", zone_type, new_price);
                }
            }
        }
    }
}
