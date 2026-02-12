// app/phases/bootstrap.rs

use eframe::egui::Context;

use crate::app::{state::AppState, state::BootstrapState, App, phases::PhaseView};

impl PhaseView for BootstrapState {
    fn tick(&mut self, app: &mut App, ctx: &Context) -> AppState {
        app.tick_bootstrap_state(ctx, self)
    }
}
