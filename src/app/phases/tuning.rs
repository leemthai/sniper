use eframe::egui::Context;

use crate::app::{state::AppState, state::TuningState, App, phases::PhaseView};

impl PhaseView for TuningState {
    fn tick(&mut self, app: &mut App, ctx: &Context) -> AppState {
        app.tick_tuning_state(ctx, self)
    }
}

