use eframe::egui::Context;

use crate::app::state::AppState;
use crate::app::App;

pub(crate) trait PhaseView {
    fn tick(&mut self, app: &mut App, ctx: &Context) -> AppState;
}
