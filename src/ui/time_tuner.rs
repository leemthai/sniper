use eframe::egui::{Ui, Layout, Align, Button};
use crate::config::{TimeTunerConfig, StationId};

#[derive(Debug)]
pub enum TunerAction {
    StationSelected(StationId),
    ConfigureTuner, 
}

pub fn render(
    ui: &mut Ui,
    time_tuner_config: &TimeTunerConfig,
    active_station_id: Option<StationId>,
    pair: Option<String>,
) -> Option<TunerAction> {
    let mut action = None;

    ui.vertical(|ui| {
        if let Some(name) = pair {
            if let Some(station_id) = active_station_id {
                ui.heading(format!("Time Tuner for {}", name));
                ui.add_space(4.0);

                // --- ROW 1: STATION BUTTONS ---
                ui.horizontal(|ui| {
                    ui.style_mut().spacing.item_spacing.x = 2.0;

                    for station in time_tuner_config.stations {
                        let is_active = station_id == station.id;

                        let btn = if is_active {
                            Button::new(station.name)
                                .fill(ui.visuals().selection.bg_fill)
                                .stroke(ui.visuals().selection.stroke)
                        } else {
                            Button::new(station.name)
                        };

                        if ui.add(btn).clicked() {
                            action = Some(TunerAction::StationSelected(station.id));
                        }
                    }

                    // Gear Icon (Right Aligned)
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("⚙").on_hover_text("Configure Time Ranges").clicked() {
                            action = Some(TunerAction::ConfigureTuner);
                        }
                    });
                });
            } else {
                ui.heading(format!("Time Tuner for {}", name));
                ui.add_space(4.0);
                ui.label("No active station selected");
            }
        } else {
            ui.heading("Time Tuner");
            ui.add_space(4.0);
            ui.label("UI not available unless a pair is selected");
        }

        ui.add_space(4.0);
    });

    action
}