//! The "save changes?" question shown before closing a drawing or the app.

use eframe::egui;
use od_ui::widgets;

use super::OpenDraftApp;
use crate::files::{After, Choice};
use crate::text::age_text;

impl OpenDraftApp {
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

    /// Offers the drawings a crashed run left behind: Recover opens them as
    /// unsaved drawings, Discard deletes the copies, and closing the dialog
    /// keeps them for the next start.
    pub(super) fn recovery_dialog(&mut self, ctx: &egui::Context) {
        if self.state.pending_recovery.is_empty() || self.state.confirm.is_some() {
            return;
        }
        let now = std::time::SystemTime::now();
        let lines: Vec<String> = self
            .state
            .pending_recovery
            .iter()
            .flat_map(|orphan| orphan.items.iter())
            .map(|item| {
                let age = item
                    .saved
                    .and_then(|t| now.duration_since(t).ok())
                    .map(age_text);
                let from = item
                    .original
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| format!(" (from {})", n.to_string_lossy()))
                    .unwrap_or_default();
                match age {
                    Some(age) => format!("{}{from}, autosaved {age}", item.title),
                    None => format!("{}{from}", item.title),
                }
            })
            .collect();
        let mut open = true;
        let mut recover = false;
        let mut discard = false;
        widgets::dialog(
            ctx,
            &self.theme,
            "Recover drawings",
            &mut open,
            460.0,
            |ui| {
                ui.label("OpenDraft closed unexpectedly. These drawings had unsaved changes:");
                ui.add_space(6.0);
                for line in &lines {
                    ui.label(
                        egui::RichText::new(format!("\u{2022}  {line}"))
                            .color(self.theme.colors.text),
                    );
                }
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        recover = widgets::primary_button(ui, &self.theme, "Recover").clicked();
                        discard = widgets::secondary_button(ui, &self.theme, "Discard").clicked();
                    });
                });
            },
        );
        if recover {
            self.state.recover_pending();
        } else if discard {
            self.state.discard_pending();
        } else if !open {
            self.state.defer_pending();
        }
    }
}
