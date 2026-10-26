//! The Object Comparison Settings dialog OVERKILL opens once the objects are
//! chosen. OK runs it; Cancel cancels the command.

use eframe::egui;
use od_commands::{Reply, Wants};
use od_ui::widgets;

use super::OpenDraftApp;

const WIDTH: f32 = 400.0;

/// A row AutoCAD has that OpenDraft cannot act on yet: shown, switched off.
fn unavailable(ui: &mut egui::Ui, label: &str) {
    let mut off = false;
    ui.add_enabled(false, egui::Checkbox::new(&mut off, label))
        .on_disabled_hover_text("OpenDraft does not have this yet.");
}

impl OpenDraftApp {
    pub(super) fn overkill_window(&mut self, ctx: &egui::Context) {
        let wanted = self
            .state
            .waiting()
            .filter(|ask| ask.wants == Wants::OverkillDialog)
            .map(|ask| ask.overkill);
        let Some(start) = wanted else {
            self.state.overkill_draft = None;
            return;
        };
        if self.state.overkill_draft.is_none() {
            self.state.overkill_draft = start;
        }
        let Some(mut draft) = self.state.overkill_draft else {
            return;
        };
        let (mut open, mut ok, mut cancel) = (true, false, false);
        let theme = &self.theme;
        widgets::dialog(
            ctx,
            theme,
            "Delete Duplicate Objects",
            &mut open,
            WIDTH,
            |ui| {
                widgets::group_box(ui, theme, "Object Comparison Settings", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Tolerance:");
                        ui.add(
                            egui::DragValue::new(&mut draft.tolerance)
                                .range(0.0..=1.0e9)
                                .speed(0.000_001)
                                .fixed_decimals(6),
                        );
                    });
                    ui.add_space(4.0);
                    ui.label("Ignore object property:");
                    let ignore = &mut draft.ignore;
                    egui::Grid::new("overkill_ignore")
                        .num_columns(2)
                        .show(ui, |ui| {
                            ui.checkbox(&mut ignore.color, "Color");
                            unavailable(ui, "Thickness");
                            ui.end_row();
                            ui.checkbox(&mut ignore.layer, "Layer");
                            unavailable(ui, "Transparency");
                            ui.end_row();
                            ui.checkbox(&mut ignore.linetype, "Linetype");
                            unavailable(ui, "Plot style");
                            ui.end_row();
                            ui.checkbox(&mut ignore.linetype_scale, "Linetype scale");
                            unavailable(ui, "Material");
                            ui.end_row();
                            ui.checkbox(&mut ignore.lineweight, "Lineweight");
                            ui.end_row();
                        });
                });
                ui.add_space(6.0);
                widgets::group_box(ui, theme, "Options", |ui| {
                    ui.checkbox(
                        &mut draft.optimize_polylines,
                        "Optimize segments within polylines",
                    );
                    ui.indent("overkill_polyline", |ui| {
                        unavailable(ui, "Ignore polyline segment widths");
                        unavailable(ui, "Do not break polylines");
                    });
                    ui.checkbox(
                        &mut draft.overlapping,
                        "Combine co-linear objects that partially overlap",
                    );
                    ui.checkbox(
                        &mut draft.end_to_end,
                        "Combine co-linear objects when aligned end to end",
                    );
                    ui.checkbox(
                        &mut draft.maintain_associative,
                        "Maintain associative objects",
                    );
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ok = widgets::primary_button(ui, theme, "OK").clicked();
                    if widgets::secondary_button(ui, theme, "Cancel").clicked() {
                        cancel = true;
                    }
                });
            },
        );
        open &= !cancel;
        if ok {
            self.state.overkill_draft = None;
            self.state.reply(Reply::Overkill(draft));
        } else if !open {
            self.state.overkill_draft = None;
            self.state.cancel_command();
        } else {
            self.state.overkill_draft = Some(draft);
        }
    }
}
