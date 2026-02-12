use eframe::egui::Context;

use crate::app::{App, state::{AppState, RunningState}, phases::phase_view::PhaseView};

impl PhaseView for RunningState {
    fn tick(&mut self, app: &mut App, ctx: &Context) -> AppState {

        #[cfg(feature = "ph_audit")]
        app.try_run_audit(ctx);

        app.tick_running_state(ctx);

        AppState::Running(RunningState)
    }
}