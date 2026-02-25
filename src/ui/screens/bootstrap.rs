use {
    crate::{
        app::{BootstrapState, SyncStatus},
        config::{BASE_INTERVAL, PLOT_CONFIG},
        ui::UI_TEXT,
        utils::TimeUtils,
    },
    eframe::egui::{
        Align, CentralPanel, Context, Grid, Layout, ProgressBar, RichText, ScrollArea, Ui,
    },
};

pub(crate) fn render_bootstrap(ctx: &Context, state: &BootstrapState) {
    CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.heading(
                RichText::new(&UI_TEXT.ls_title)
                    .size(24.0)
                    .strong()
                    .color(PLOT_CONFIG.color_warning),
            );
            let interval_str = TimeUtils::interval_to_string(BASE_INTERVAL.as_millis() as i64);
            ui.label(
                RichText::new(format!(
                    "{} {} {}",
                    UI_TEXT.ls_syncing, interval_str, UI_TEXT.ls_main,
                ))
                .italics()
                .color(PLOT_CONFIG.color_text_neutral),
            );
            ui.add_space(20.0);
            let total = state.total_pairs;
            let done = state.completed + state.failed;
            let progress = if total > 0 {
                done as f32 / total as f32
            } else {
                0.0
            };
            ui.add_space(20.0);
            ui.add(
                ProgressBar::new(progress)
                    .show_percentage()
                    .animate(true)
                    .text(format!("Processed {}/{}", done, total)),
            );
            if state.failed > 0 {
                ui.add_space(5.0);
                ui.label(
                    RichText::new(format!(
                        "{} {} {}",
                        UI_TEXT.label_warning, state.failed, UI_TEXT.label_failures
                    ))
                    .color(PLOT_CONFIG.color_loss),
                );
            }
            ui.add_space(20.0);
        });

        render_loading_grid(ui, state);
    });
}

fn render_loading_grid(ui: &mut Ui, state: &BootstrapState) {
    ScrollArea::vertical().show(ui, |ui| {
        Grid::new("loading_grid_multi_col")
            .striped(true)
            .spacing([20.0, 10.0])
            .min_col_width(250.0)
            .show(ui, |ui| {
                for (i, (_idx, (pair, status))) in state.pairs.iter().enumerate() {
                    let (color, status_text, status_color) = match status {
                        SyncStatus::Pending => (
                            PLOT_CONFIG.color_text_subdued,
                            "-".to_string(),
                            PLOT_CONFIG.color_text_subdued,
                        ),
                        SyncStatus::Syncing => (
                            PLOT_CONFIG.color_warning,
                            UI_TEXT.ls_syncing.to_string(),
                            PLOT_CONFIG.color_warning,
                        ),
                        SyncStatus::Completed(n) => (
                            PLOT_CONFIG.color_text_primary,
                            format!("+{}", n),
                            PLOT_CONFIG.color_profit,
                        ),
                        SyncStatus::Failed(_) => (
                            PLOT_CONFIG.color_loss,
                            UI_TEXT.ls_failed.to_string(),
                            PLOT_CONFIG.color_loss,
                        ),
                    };
                    ui.horizontal(|ui| {
                        ui.set_min_width(240.0);
                        ui.label(RichText::new(pair).strong().color(color));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| match status {
                            SyncStatus::Syncing => {
                                ui.spinner();
                            }
                            SyncStatus::Completed(_) => {
                                ui.label(RichText::new(status_text).color(status_color));
                            }
                            _ => {
                                ui.label(RichText::new(status_text).color(status_color));
                            }
                        });
                    });

                    if (i + 1) % 3 == 0 {
                        ui.end_row();
                    }
                }
            });
    });
}
