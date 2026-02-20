use eframe::egui::{Button, Grid, RichText, ScrollArea, Ui};

use crate::analysis::range_gap_finder::{DisplaySegment, GapReason};

use crate::config::plot::PLOT_CONFIG;

use crate::ui::UI_TEXT;
use crate::ui::UiStyleExt;

use crate::utils::epoch_ms_to_date_string;

pub struct CandleRangePanel<'a> {
    segments: &'a [DisplaySegment],
    current_range_idx: Option<usize>,
}

impl<'a> CandleRangePanel<'a> {
    pub(crate) fn new(segments: &'a [DisplaySegment], current_idx: Option<usize>) -> Self {
        Self {
            segments,
            current_range_idx: current_idx,
        }
    }

    pub(crate) fn render(&mut self, ui: &mut Ui, last_viewed_idx: usize) -> Option<Option<usize>> {
        let mut action = None;

        ui.add_space(5.0);
        // ui.heading(format!("{}", UI_TEXT.cr_title));
        // ui.separator();
        ui.label_subheader(format!(
            "{} {} {}",
            self.segments.len(),
            UI_TEXT.cr_title_1,
            UI_TEXT.cr_title_2
        ));

        ui.horizontal(|ui| {
            // PREV BUTTON
            let prev_enabled = self.current_range_idx.is_some_and(|i| i > 0);
            if ui.add_enabled(prev_enabled, Button::new("⬅")).clicked() {
                if let Some(curr) = self.current_range_idx {
                    action = Some(Some(curr - 1));
                }
            }

            // TOGGLE BUTTON (Middle)
            let is_viewing_all = self.current_range_idx.is_none();
            let (btn_label, target_idx) = if is_viewing_all {
                let safe_target = last_viewed_idx.min(self.segments.len().saturating_sub(1));
                let is_live = safe_target == self.segments.len().saturating_sub(1);

                let text = if is_live {
                    UI_TEXT.cr_nav_return_live.to_string()
                } else {
                    format!("{} {}", UI_TEXT.cr_nav_return_prefix, safe_target + 1)
                };
                (ui.button_text_secondary(text), Some(safe_target))
            } else {
                (ui.button_text_primary(&UI_TEXT.cr_nav_show_all), None)
            };

            if ui.button(btn_label).clicked() {
                action = Some(target_idx);
            }

            // NEXT BUTTON
            let next_enabled = self
                .current_range_idx
                .is_some_and(|i| i < self.segments.len() - 1);
            if ui.add_enabled(next_enabled, Button::new("➡")).clicked() {
                if let Some(curr) = self.current_range_idx {
                    action = Some(Some(curr + 1));
                } else {
                    action = Some(Some(self.segments.len() - 1));
                }
            }
        });

        ui.separator();

        // --- COMPACT LIST ---
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                Grid::new("cr_grid")
                    .striped(true)
                    .num_columns(2) // Reduced to 2
                    .spacing([10.0, 8.0]) // Tighter spacing
                    .show(ui, |ui| {
                        // Compact Headers
                        ui.label(RichText::new(&UI_TEXT.cr_date_range).strong().small());
                        ui.label(RichText::new(&UI_TEXT.cr_context).strong().small());
                        ui.end_row();

                        for (i, seg) in self.segments.iter().enumerate().rev() {
                            let is_selected = self.current_range_idx == Some(i);

                            // GAP ROW
                            if i > 0 {
                                // Merged Gap Info (Duration + Reason)
                                let gap_text = format!(
                                    "-- {} {} ({}) --",
                                    seg.gap_duration_str,
                                    &UI_TEXT.cr_gap,
                                    match seg.gap_reason {
                                        GapReason::PriceMismatch => &UI_TEXT.cr_price,
                                        GapReason::MissingSourceData => &UI_TEXT.cr_missing,
                                        GapReason::PriceAbovePH => &UI_TEXT.cr_high,
                                        GapReason::PriceBelowPH => &UI_TEXT.cr_low,
                                        _ => &UI_TEXT.cr_mixed,
                                    }
                                );

                                // Use Semantic Colors for Gaps
                                let gap_color = match seg.gap_reason {
                                    GapReason::MissingSourceData => PLOT_CONFIG.color_gap_missing,
                                    GapReason::PriceAbovePH => PLOT_CONFIG.color_gap_above,
                                    GapReason::PriceBelowPH => PLOT_CONFIG.color_gap_below,
                                    _ => PLOT_CONFIG.color_text_subdued,
                                };

                                ui.label(
                                    RichText::new(gap_text).italics().small().color(gap_color),
                                );
                                ui.label(""); // Empty context column for gap
                                ui.end_row();
                            }

                            // SEGMENT ROW
                            let start_date = epoch_ms_to_date_string(seg.start_ts);
                            let end_date = epoch_ms_to_date_string(seg.end_ts);

                            // Column 1: Date Range + Count (Clickable)
                            // Format: "2024-01-01 - 2024-02-01 (500c)"
                            let label_text =
                                format!("{} - {} ({}c)", start_date, end_date, seg.candle_count);
                            if ui
                                .selectable_label(is_selected, RichText::new(label_text).small())
                                .clicked()
                            {
                                action = Some(Some(i));
                            }

                            // Column 2: Context
                            if i == self.segments.len() - 1 {
                                ui.label(
                                    RichText::new(&UI_TEXT.cr_label_live)
                                        .color(PLOT_CONFIG.color_profit)
                                        .strong()
                                        .small(),
                                );
                            } else {
                                ui.label(
                                    RichText::new(&UI_TEXT.cr_label_historical)
                                        .small()
                                        .color(PLOT_CONFIG.color_text_subdued),
                                );
                            }
                            ui.end_row();
                        }
                    });
            });

        action
    }
}
