//! The Purge dialog (PURGE): what the drawing no longer uses, by kind, to
//! tick and delete. The list is `od_core::purge::purgeable`; deleting is
//! `purge_cmd.rs`.

use eframe::egui;
use od_core::purge::{Category, Item};
use od_ui::widgets;

use super::OpenDraftApp;
use crate::purge_cmd::{purge_all, purge_items};

const WIDTH: f32 = 420.0;

impl OpenDraftApp {
    pub(super) fn purge_window(&mut self, ctx: &egui::Context) {
        if !self.state.purge.open {
            return;
        }
        if self.state.purge.stale && !self.state.command_running() {
            self.state.refresh_purge();
        }
        let busy = self.state.command_running();
        let mut dialog = std::mem::take(&mut self.state.purge);
        let (mut open, mut purge, mut all, mut close) = (true, false, false, false);
        let theme = &self.theme;
        widgets::dialog(ctx, theme, "Purge", &mut open, WIDTH, |ui| {
            ui.label("Unused named items you can purge:");
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    if dialog.items.is_empty() {
                        ui.label("There is nothing to purge.");
                    }
                    for category in Category::ALL {
                        let rows: Vec<usize> = (0..dialog.items.len())
                            .filter(|&i| dialog.items[i].0.item.category() == category)
                            .collect();
                        if rows.is_empty() {
                            continue;
                        }
                        let title = format!("{} ({})", category.title(), rows.len());
                        egui::CollapsingHeader::new(title)
                            .id_salt(("purge", category as u8))
                            .default_open(true)
                            .show(ui, |ui| {
                                let mut all_on = rows.iter().all(|&i| dialog.items[i].1);
                                if ui.checkbox(&mut all_on, "All").changed() {
                                    for &i in &rows {
                                        dialog.items[i].1 = all_on;
                                    }
                                }
                                for &i in &rows {
                                    let (p, on) = &mut dialog.items[i];
                                    ui.checkbox(on, p.name.clone());
                                }
                            });
                    }
                });
            ui.add_space(10.0);
            let any = dialog.items.iter().any(|(_, on)| *on);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!busy, |ui| {
                    ui.add_enabled_ui(any, |ui| {
                        purge = widgets::primary_button(ui, theme, "Purge").clicked();
                    });
                    ui.add_enabled_ui(!dialog.items.is_empty(), |ui| {
                        all = widgets::secondary_button(ui, theme, "Purge All").clicked();
                    });
                });
                close = widgets::secondary_button(ui, theme, "Close").clicked();
            });
        });
        open &= !close;
        if purge {
            let wanted: Vec<Item> = dialog
                .items
                .iter()
                .filter(|(_, on)| *on)
                .map(|(p, _)| p.item.clone())
                .collect();
            self.state
                .start_command("PURGE", move |ed| purge_items(ed, wanted));
            dialog.stale = true;
        } else if all {
            self.state.start_command("-PURGE", purge_all);
            dialog.stale = true;
        }
        dialog.open = open;
        self.state.purge = dialog;
    }
}
