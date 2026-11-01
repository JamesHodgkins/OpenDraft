//! The Drawing Properties dialog (DWGPROPS): the title, subject, author,
//! keywords, comments and hyperlink base of the drawing, which data fields
//! can show. It edits a draft copy; OK commits it as one undo step, Cancel,
//! Escape and the X drop it.

use eframe::egui::{self, vec2};
use od_ui::{widgets, Theme};

use super::OpenDraftApp;

const WIDTH: f32 = 460.0;
const LABEL: f32 = 110.0;

/// A label and a one-line field.
fn line(ui: &mut egui::Ui, theme: &Theme, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.add_sized(vec2(LABEL, theme.control_height), egui::Label::new(label));
        widgets::text_field(ui, theme, value, "");
    });
    ui.add_space(4.0);
}

impl OpenDraftApp {
    /// The window opened by `format.properties`.
    pub(super) fn properties_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.state.command_dialogs.properties.take() else {
            return;
        };
        let (mut open, mut ok, mut cancel) = (true, false, false);
        let theme = &self.theme;
        widgets::dialog(ctx, theme, "Drawing Properties", &mut open, WIDTH, |ui| {
            widgets::group_box(ui, theme, "Summary", |ui| {
                line(ui, theme, "Title:", &mut draft.title);
                line(ui, theme, "Subject:", &mut draft.subject);
                line(ui, theme, "Author:", &mut draft.author);
                line(ui, theme, "Keywords:", &mut draft.keywords);
                ui.horizontal_top(|ui| {
                    ui.add_sized(
                        vec2(LABEL, theme.control_height),
                        egui::Label::new("Comments:"),
                    );
                    ui.add(
                        egui::TextEdit::multiline(&mut draft.comments)
                            .desired_rows(4)
                            .desired_width(ui.available_width()),
                    );
                });
                ui.add_space(4.0);
                line(ui, theme, "Hyperlink base:", &mut draft.hyperlink_base);
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ok = widgets::primary_button(ui, theme, "OK").clicked();
                cancel = widgets::secondary_button(ui, theme, "Cancel").clicked();
            });
        });
        if ok {
            self.state.set_drawing_properties(draft);
        } else if !(cancel || !open) {
            self.state.command_dialogs.properties = Some(draft);
        }
    }
}
