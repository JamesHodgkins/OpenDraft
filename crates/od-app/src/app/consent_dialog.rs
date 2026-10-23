//! The question an assistant's file operation puts to the user. It is built
//! only from what the app knows (the path, the kind of operation, the name
//! the connection gave itself), and nothing a model says can answer it.

use eframe::egui;
use od_ui::widgets;

use super::OpenDraftApp;
use crate::automation::{answer_consent, consent_text};

impl OpenDraftApp {
    pub(super) fn consent_dialog(&mut self, ctx: &egui::Context) {
        let Some((id, text)) = consent_text(&self.state) else {
            return;
        };
        // Questions time out, so keep looking while one is up.
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
        let mut open = true;
        let mut answer = None;
        widgets::dialog(ctx, &self.theme, "Replace file?", &mut open, 460.0, |ui| {
            ui.label(text);
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "The file there now will be overwritten. Choose Don't replace if you did not expect this.",
                )
                .color(self.theme.colors.text_weak),
            );
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::secondary_button(ui, &self.theme, "Replace").clicked() {
                        answer = Some(true);
                    }
                    if widgets::primary_button(ui, &self.theme, "Don't replace").clicked() {
                        answer = Some(false);
                    }
                });
            });
        });
        if !open {
            answer = Some(false);
        }
        if let Some(allow) = answer {
            answer_consent(&mut self.state, id, allow);
        }
    }
}
