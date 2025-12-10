use eframe::egui::{ComboBox, ScrollArea, Ui, Slider, RichText, Color32};
use strum::IntoEnumIterator;

use crate::config::ANALYSIS;
use crate::config::plot::PLOT_CONFIG;
use crate::domain::pair_interval::PairInterval;

use crate::models::cva::ScoreType;
use crate::models::{PairContext, ZoneType};
use crate::ui::config::UI_TEXT;
use crate::ui::utils::{colored_subsection_heading, section_heading, spaced_separator};

#[cfg(debug_assertions)]
use crate::config::DEBUG_FLAGS;

/// Trait for UI panels that can be rendered
pub trait Panel {
    type Event;
    fn render(&mut self, ui: &mut Ui) -> Vec<Self::Event>;
}

/// Panel for data generation options
pub struct DataGenerationPanel<'a> {
    #[allow(dead_code)]
    zone_count: usize,
    selected_pair: Option<String>,
    available_pairs: Vec<String>,
    auto_duration_config: &'a crate::domain::auto_duration::AutoDurationConfig,
    time_horizon_days: u64,
}

impl<'a> DataGenerationPanel<'a> {
    pub fn new(
        zone_count: usize,
        selected_pair: Option<String>,
        available_pairs: Vec<String>,
        auto_duration_config: &'a crate::domain::auto_duration::AutoDurationConfig,
        time_horizon_days: u64,
    ) -> Self {
        Self {
            zone_count,
            selected_pair,
            available_pairs,
            auto_duration_config,
            time_horizon_days,
        }
    }

    fn render_auto_duration_display(&mut self, ui: &mut Ui) -> Option<f64> {
        let mut changed = None;

        ui.add_space(5.0);
        ui.label(colored_subsection_heading(UI_TEXT.price_horizon_heading));

        let mut threshold_pct = self.auto_duration_config.relevancy_threshold * 100.0;
        let response = ui.add(
            Slider::new(&mut threshold_pct, 1.0..=80.0)
                .step_by(1.0)
                .suffix("%"),
        );

        if response.changed() {
            changed = Some(threshold_pct / 100.0);
        }

        let helper_text = format!(
            "{}{}{}",
            UI_TEXT.price_horizon_helper_prefix,
            threshold_pct.round(),
            UI_TEXT.price_horizon_helper_suffix
        );
        ui.label(
            RichText::new(helper_text)
                .small()
                .color(Color32::GRAY),
        );

        changed
    }

    fn render_time_horizon_slider(&mut self, ui: &mut Ui) -> Option<u64> {
        let mut changed = None;

        ui.add_space(5.0);
        ui.label(colored_subsection_heading(UI_TEXT.time_horizon_heading));

        let mut horizon_days = self.time_horizon_days as f64;
        let response = ui.add(
            Slider::new(
                &mut horizon_days,
                ANALYSIS.time_horizon.min_days as f64..=ANALYSIS.time_horizon.max_days as f64,
            )
            .integer()
            .suffix(" days"),
        );

        let new_value = horizon_days.round() as u64;
        self.time_horizon_days = new_value;

        if response.changed() {
            changed = Some(new_value);
        }

        let helper_text = format!(
            "{}{}{}",
            UI_TEXT.time_horizon_helper_prefix, new_value, UI_TEXT.time_horizon_helper_suffix
        );
        ui.label(
            RichText::new(helper_text)
                .small()
                .color(Color32::GRAY),
        );

        changed
    }

    fn render_pair_selector(&mut self, ui: &mut Ui) -> Option<String> {
        let mut changed = None;
        let previously_selected_pair = self.selected_pair.clone();

        ui.label(colored_subsection_heading(UI_TEXT.pair_selector_heading));
        ScrollArea::vertical()
            .max_height(160.)
            .id_salt("pair_selector")
            .show(ui, |ui| {
                for item in &self.available_pairs {
                    let is_selected = self.selected_pair.as_ref() == Some(item);
                    if ui.selectable_label(is_selected, item).clicked() {
                        self.selected_pair = Some(item.clone());
                        changed = Some(item.clone());
                    }
                }
            });

        // Defensive check: catch changes even if .clicked() didn't fire
        if self.selected_pair != previously_selected_pair {
            changed = self.selected_pair.clone();
            #[cfg(debug_assertions)]
            if DEBUG_FLAGS.print_ui_interactions {
                log::info!("A new pair was selected: {:?}", self.selected_pair);
            }
        }

        changed
    }
}

#[derive(Debug)]
pub enum DataGenerationEventChanged {
    // ZoneCount(usize),
    Pair(String),
    AutoDurationThreshold(f64),
    TimeHorizonDays(u64),
}

impl<'a> Panel for DataGenerationPanel<'a> {
    type Event = DataGenerationEventChanged;
    fn render(&mut self, ui: &mut Ui) -> Vec<Self::Event> {
        let mut events = Vec::new();
        section_heading(ui, UI_TEXT.data_generation_heading);

        // Auto duration display (always enabled)
        if let Some(threshold) = self.render_auto_duration_display(ui) {
            events.push(DataGenerationEventChanged::AutoDurationThreshold(threshold));
        }
        spaced_separator(ui);

        if let Some(days) = self.render_time_horizon_slider(ui) {
            events.push(DataGenerationEventChanged::TimeHorizonDays(days));
        }
        spaced_separator(ui);

        if let Some(pair) = self.render_pair_selector(ui) {
            events.push(DataGenerationEventChanged::Pair(pair));
        }
        if let Some(pair) = &self.selected_pair {
            ui.label(format!(
                "Selected: {:?}",
                PairInterval::split_pair_name(pair)
            ));
        }
        ui.add_space(20.0);
        events
    }
}

/// Panel for view options
pub struct ViewPanel {
    selected_score_type: ScoreType,
}

impl ViewPanel {
    pub fn new(score_type: ScoreType) -> Self {
        Self {
            selected_score_type: score_type,
        }
    }
}

impl Panel for ViewPanel {
    type Event = ScoreType;
    fn render(&mut self, ui: &mut Ui) -> Vec<Self::Event> {
        let mut events = Vec::new();
        section_heading(ui, UI_TEXT.view_options_heading);

        ui.label(colored_subsection_heading(UI_TEXT.view_data_source_heading));
        ComboBox::from_id_salt("Score Type")
            .selected_text(self.selected_score_type.to_string())
            .show_ui(ui, |ui| {
                for score_type_variant in ScoreType::iter() {
                    if ui
                        .selectable_value(
                            &mut self.selected_score_type,
                            score_type_variant,
                            score_type_variant.to_string(),
                        )
                        .clicked()
                    {
                        events.push(self.selected_score_type);
                    }
                }
            });

        ui.add_space(20.0);
        events
    }
}

/// Panel showing trading opportunities across all monitored pairs
pub struct SignalsPanel<'a> {
    signals: Vec<&'a PairContext>,
}

impl<'a> SignalsPanel<'a> {
    pub fn new(signals: Vec<&'a PairContext>) -> Self {
        Self { signals }
    }
}

impl<'a> Panel for SignalsPanel<'a> {
    type Event = String; // Returns pair name if clicked

    fn render(&mut self, ui: &mut Ui) -> Vec<Self::Event> {
        let mut events = Vec::new();
        section_heading(ui, UI_TEXT.signals_heading);

        if self.signals.is_empty() {
            ui.label(
                RichText::new("No high-interest signals")
                    .small()
                    .color(Color32::GRAY),
            );
        } else {
            ui.label(
                RichText::new(format!("{} active", self.signals.len()))
                    .small()
                    .color(Color32::from_rgb(100, 200, 255)),
            );
            ui.add_space(5.0);

            for opp in &self.signals {
                ui.group(|ui| {
                    // Pair name as clickable button
                    let pair_label = format!("📌 {}", opp.pair_name);
                    if ui.button(pair_label).clicked() {
                        events.push(opp.pair_name.clone());
                    }

                    // Current zone types (as lng as it is sticky)
                    for (zone_index, zone_type) in &opp.current_zones {
                        let zone_label = match zone_type {
                            ZoneType::Sticky => Some((
                                format!("🔑 Sticky superzone {}", zone_index),
                                PLOT_CONFIG.sticky_zone_color,
                            )),
                            _ => None,
                        };

                        if let Some((text, color)) = zone_label {
                            ui.label(RichText::new(text).small().color(color));
                        }
                    }
                });
                ui.add_space(3.0);
            }
        }
        ui.add_space(10.0);
        events
    }
}
