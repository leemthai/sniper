use crate::config::{StationId, TimeTunerConfig};
use eframe::egui::{Align, Button, Layout, Ui, vec2};

#[derive(Debug)]
pub(crate) enum TunerAction {
    StationSelected(StationId),
    ConfigureTuner,
}

pub(crate) fn render(
    ui: &mut Ui,
    time_tuner_config: &TimeTunerConfig,
    active_station_id: Option<StationId>,
    pair: Option<String>,
) -> Option<TunerAction> {
    let mut action = None;

    ui.vertical(|ui| {
        if let Some(name) = pair {
            let headline = format!("Generate new {} trades in YOUR style:", name);
            let y_height = 35.0;

            if let Some(station_id) = active_station_id {
                ui.heading(headline);
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

                        let response = ui.add_sized(vec2(90.0, y_height), btn);

                        if response.clicked() {
                            action = Some(TunerAction::StationSelected(station.id));
                        }
                    }

                    // Gear Icon (Right Aligned)
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add_sized(vec2(35.0, y_height), Button::new("⚙"))
                            .on_hover_text("Configure Time Ranges")
                            .clicked()
                        {
                            action = Some(TunerAction::ConfigureTuner);
                        }
                    });
                });
            } else {
                ui.heading(headline);
                ui.add_space(4.0);
                ui.label("No active style selected");
            }
        } else {
            ui.heading("Trading Style");
            ui.add_space(4.0);
            ui.label("UI not available unless a trading pair is selected");
        }

        ui.add_space(4.0);
    });

    action
}
