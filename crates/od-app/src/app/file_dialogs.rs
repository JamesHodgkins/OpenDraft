//! The "save changes?" question shown before closing a drawing or the app.

use eframe::egui;
use od_ui::widgets;

use super::OpenDraftApp;
use crate::files::{After, Choice};

impl OpenDraftApp {
    /// Asks what to do with a block's unsaved changes before the Block
    /// Editor closes: Save, Don't Save or Cancel. Escape and the close
    /// button mean Cancel.
    pub(super) fn block_close_dialog(&mut self, ctx: &egui::Context) {
        let Some(name) = self
            .state
            .docs
            .get(self.state.active)
            .and_then(|d| d.block_edit.as_ref())
            .filter(|edit| edit.closing)
            .map(|edit| edit.name.clone())
        else {
            return;
        };
        let mut open = true;
        let mut choice = None;
        widgets::dialog(
            ctx,
            &self.theme,
            "Save block changes",
            &mut open,
            420.0,
            |ui| {
                ui.label(format!("Save changes to block \"{name}\"?"));
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Your changes to the block will be lost if you do not save them.",
                    )
                    .color(self.theme.colors.text_weak),
                );
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::primary_button(ui, &self.theme, "Save").clicked() {
                            choice = Some(Choice::Save);
                        }
                        if widgets::secondary_button(ui, &self.theme, "Don't Save").clicked() {
                            choice = Some(Choice::Discard);
                        }
                        if widgets::secondary_button(ui, &self.theme, "Cancel").clicked() {
                            choice = Some(Choice::Cancel);
                        }
                    });
                });
            },
        );
        if !open {
            choice = Some(Choice::Cancel);
        }
        if let Some(choice) = choice {
            self.state.answer_block_close(choice);
        }
    }

    /// Asks what to do with a drawing's unsaved changes: Save, Don't Save or
    /// Cancel. Escape and the close button mean Cancel.
    pub(super) fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(after) = self.state.confirm.as_ref().map(|c| c.after()) else {
            return;
        };
        let Some(name) = self.state.confirm_subject().map(|d| d.title.clone()) else {
            // The drawing went away behind the question: move on.
            self.state.answer_confirm(Choice::Discard);
            return;
        };
        let leaving = match after {
            After::Close => "closing",
            After::Exit => "quitting",
        };
        let saving = self
            .state
            .confirm
            .as_ref()
            .is_some_and(|c| c.saving().is_some());
        let mut open = true;
        let mut choice = None;
        widgets::dialog(
            ctx,
            &self.theme,
            "Unsaved changes",
            &mut open,
            420.0,
            |ui| {
                ui.label(format!("Save changes to \"{name}\" before {leaving}?"));
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Your changes will be lost if you do not save them.")
                        .color(self.theme.colors.text_weak),
                );
                ui.add_space(14.0);
                if saving {
                    ui.label(egui::RichText::new("Saving...").color(self.theme.colors.text_weak));
                    return;
                }
                // Wrapped in horizontal so the right-to-left layout is one row
                // tall instead of stretching the dialog.
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::primary_button(ui, &self.theme, "Save").clicked() {
                            choice = Some(Choice::Save);
                        }
                        if widgets::secondary_button(ui, &self.theme, "Don't Save").clicked() {
                            choice = Some(Choice::Discard);
                        }
                        if widgets::secondary_button(ui, &self.theme, "Cancel").clicked() {
                            choice = Some(Choice::Cancel);
                        }
                    });
                });
            },
        );
        if !open {
            choice = Some(Choice::Cancel);
        }
        if let Some(choice) = choice {
            self.state.answer_confirm(choice);
        }
    }
}
