use super::ZoneSniperApp;
#[cfg(debug_assertions)]
use crate::config::debug::PRINT_SIMULATION_EVENTS;
use crate::models::TradingModel;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum SimDirection {
    #[default]
    Up,
    Down,
}

impl std::fmt::Display for SimDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimDirection::Up => write!(f, "▲ PRICE UP"),
            SimDirection::Down => write!(f, "▼ PRICE DOWN"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) enum SimStepSize {
    Point1, // 0.1%
    #[default]
    One, // 1%
    Five,   // 5%
    Ten,    // 10%
}

impl SimStepSize {
    pub(super) fn as_percentage(&self) -> f64 {
        match self {
            SimStepSize::Point1 => 0.1,
            SimStepSize::One => 1.0,
            SimStepSize::Five => 5.0,
            SimStepSize::Ten => 10.0,
        }
    }

    pub(super) fn cycle(&mut self) {
        *self = match self {
            SimStepSize::Point1 => SimStepSize::One,
            SimStepSize::One => SimStepSize::Five,
            SimStepSize::Five => SimStepSize::Ten,
            SimStepSize::Ten => SimStepSize::Point1,
        };
    }
}

impl std::fmt::Display for SimStepSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}%", self.as_percentage())
    }
}

impl ZoneSniperApp {
    pub(super) fn get_display_price(&mut self, pair: &str) -> Option<f64> {
        if self.is_simulation_mode {
            if let Some(live_price) = self
                .price_stream
                .as_ref()
                .and_then(|stream| stream.get_price(pair))
                .filter(|_| !self.simulated_prices.contains_key(pair))
            {
                self.simulated_prices.insert(pair.to_string(), live_price);
                #[cfg(debug_assertions)]
                if PRINT_SIMULATION_EVENTS {
                    log::info!(
                        "🎮 Initialized simulation price for {}: ${:.2}",
                        pair,
                        live_price
                    );
                }
            }
            self.simulated_prices.get(pair).copied()
        } else if let Some(ref stream) = self.price_stream {
            stream.get_price(pair)
        } else {
            None
        }
    }

    pub(super) fn toggle_simulation_mode(&mut self) {
        self.is_simulation_mode = !self.is_simulation_mode;

        if self.is_simulation_mode {
            if let Some(ref stream) = self.price_stream {
                stream.suspend();

                if let Some(ref pair) = self.selected_pair {
                    if !self.simulated_prices.contains_key(pair) {
                        if let Some(live_price) = stream.get_price(pair) {
                            self.simulated_prices.insert(pair.clone(), live_price);
                            #[cfg(debug_assertions)]
                            if PRINT_SIMULATION_EVENTS {
                                log::info!(
                                    "🎮 Entered SIMULATION MODE for {} at ${:.2}",
                                    pair,
                                    live_price
                                );
                            }
                        }
                    } else if let Some(_sim_price) = self.simulated_prices.get(pair) {
                        #[cfg(debug_assertions)]
                        if PRINT_SIMULATION_EVENTS {
                            log::info!(
                                "🎮 Entered SIMULATION MODE for {} at ${:.2} (restored)",
                                pair,
                                _sim_price
                            );
                        }
                    }
                }
            }
        } else if let Some(ref stream) = self.price_stream {
            stream.resume();
            #[cfg(debug_assertions)]
            if PRINT_SIMULATION_EVENTS {
                log::info!("📡 Exited to LIVE MODE (simulated prices preserved)");
            }
        }
    }

    pub(super) fn adjust_simulated_price_by_percent(&mut self, percent: f64) {
        if !self.is_simulation_mode {
            return;
        }

        if let Some((pair, current_price)) = self
            .selected_pair
            .as_ref()
            .and_then(|p| self.simulated_prices.get(p).copied().map(|c| (p, c)))
        {
            let adjustment = current_price * (percent / 100.0);
            let new_price = current_price + adjustment;
            self.simulated_prices.insert(pair.clone(), new_price);

            #[cfg(debug_assertions)]
            if PRINT_SIMULATION_EVENTS {
                log::info!(
                    "💰 {} price: ${:.2} → ${:.2} ({:+.1}%)",
                    pair,
                    current_price,
                    new_price,
                    percent
                );
            }

            self.multi_pair_monitor
                .process_price_update(pair, new_price);
        }
    }

    pub(super) fn jump_to_next_zone(&mut self, zone_type: &str) {
        if !self.is_simulation_mode {
            return;
        }

        let Some(ref pair) = self.selected_pair else {
            return;
        };
        let Some(current_price) = self.simulated_prices.get(pair).copied() else {
            return;
        };
        let Some(ref cva_results) = self.data_state.cva_results else {
            return;
        };

        let trading_model = TradingModel::from_cva(Arc::clone(cva_results), Some(current_price));

        let superzones = match zone_type {
            "sticky" => Some(&trading_model.zones.sticky_superzones),
            "slippy" => Some(&trading_model.zones.slippy_superzones),
            "low-wick" => Some(&trading_model.zones.low_wicks_superzones),
            "high-wick" => Some(&trading_model.zones.high_wicks_superzones),
            _ => None,
        };

        let Some(superzones) = superzones else {
            return;
        };

        if superzones.is_empty() {
            return;
        }

        let target_superzone = match self.sim_direction {
            SimDirection::Up => superzones
                .iter()
                .filter(|sz| sz.price_center > current_price)
                .min_by(|a, b| a.price_center.partial_cmp(&b.price_center).unwrap()),
            SimDirection::Down => superzones
                .iter()
                .filter(|sz| sz.price_center < current_price)
                .max_by(|a, b| a.price_center.partial_cmp(&b.price_center).unwrap()),
        };

        if let Some(superzone) = target_superzone {
            let new_price = superzone.price_center;
            self.simulated_prices.insert(pair.clone(), new_price);

            self.multi_pair_monitor
                .process_price_update(pair, new_price);
        }
    }
}
