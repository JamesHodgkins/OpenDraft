//! The What's new note on the Home page: a small card for this version, with
//! a link to its release page that is followed only when clicked, and Dismiss.

use eframe::egui;
use od_ui::widgets;

use super::Pick;
use crate::whats_new::{release_url, Note};

pub(super) fn whats_new_section(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    note: &Note,
    pick: &mut Option<Pick>,
) {
    let c = &theme.colors;
    ui.add_space(22.0);
    widgets::section_header(ui, theme, "What's new");
    egui::Frame::new()
        .fill(c.panel_raised)
        .stroke(egui::Stroke::new(1.0_f32, c.border))
        .corner_radius(egui::CornerRadius::same(theme.radius))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                egui::RichText::new(format!("New in OpenDraft {}", env!("CARGO_PKG_VERSION")))
                    .strong()
                    .color(c.text),
            );
            ui.label(egui::RichText::new(note.title).color(c.text_weak));
            ui.add_space(6.0);
            for change in note.changes {
                ui.horizontal_top(|ui| {
                    ui.label(egui::RichText::new("\u{2022}").color(c.text_weak));
                    ui.add(egui::Label::new(egui::RichText::new(*change).color(c.text)).wrap());
                });
            }
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if widgets::secondary_button(ui, theme, "View release notes").clicked() {
                    // Nothing is fetched here: the system opens the page.
                    ui.ctx().open_url(egui::OpenUrl::new_tab(release_url(note)));
                }
                if widgets::secondary_button(ui, theme, "Dismiss").clicked() {
                    *pick = Some(Pick::DismissWhatsNew);
                }
            });
        });
}
