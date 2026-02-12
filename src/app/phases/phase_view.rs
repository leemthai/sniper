use eframe::egui::Context;

use crate::app::{App, state::AppState};

pub(crate) trait PhaseView {
    fn tick(&mut self, app: &mut App, ctx: &Context) -> AppState;
}
