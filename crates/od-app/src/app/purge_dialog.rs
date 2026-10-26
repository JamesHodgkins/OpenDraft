//! The Purge dialog (PURGE): what the drawing no longer uses, by kind, to
//! tick and delete, and a second page saying why the rest has to stay. The
//! lists are `od_core::purge`; deleting is `purge_cmd.rs`.

use std::collections::BTreeSet;

use eframe::egui;
use od_core::purge::{users_of, Category, Item};
use od_ui::widgets;

use super::OpenDraftApp;
use crate::purge_cmd::{purge_run, Dialog, Purge, Tab};
use crate::purge_memory::{memory, remember};

const WIDTH: f32 = 440.0;

/// What a click in the dialog asks for.
#[derive(Default)]
struct Clicks {
    purge: bool,
    all: bool,
    close: bool,
    select: bool,
}

fn purgeable_page(ui: &mut egui::Ui, dialog: &mut Dialog) {
    ui.label("Unused named items you can purge:");
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .id_salt("purge_items")
        .max_height(220.0)
        .show(ui, |ui| {
            if dialog.items.is_empty() {
                ui.label("There is nothing to purge.");
            }
            for category in Category::NAMED {
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
}

fn non_purgeable_page(ui: &mut egui::Ui, dialog: &mut Dialog) -> bool {
    ui.label("Items that cannot be purged, and why:");
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .id_salt("purge_blocked")
        .max_height(160.0)
        .show(ui, |ui| {
            for category in Category::NAMED {
                for (i, b) in dialog.blocked.iter().enumerate() {
                    if b.item.category() != category {
                        continue;
                    }
                    let text = format!("{}: {}", category.title(), b.name);
                    if ui
                        .selectable_label(dialog.detail == Some(i), text)
                        .clicked()
                    {
                        dialog.detail = Some(i);
                    }
                }
            }
        });
    ui.add_space(6.0);
    let mut select = false;
    if let Some(b) = dialog.detail.and_then(|i| dialog.blocked.get(i)) {
        ui.label(format!("\"{}\" stays because:", b.name));
        for reason in &b.reasons {
            ui.label(format!("  \u{2022} {reason}"));
        }
        select = ui.button("Select Objects").clicked();
    } else {
        ui.label("Pick an item to see its reasons.");
    }
    select
}

impl OpenDraftApp {
    pub(super) fn purge_window(&mut self, ctx: &egui::Context) {
        if !self.state.purge.open {
            return;
        }
        // While a purge runs (and may be asking about each item on the
        // command line) the dialog steps aside.
        if self.state.command_running() {
            return;
        }
        if self.state.purge.stale {
            self.state.refresh_purge();
        }
        let mut dialog = std::mem::take(&mut self.state.purge);
        let mut options = memory();
        let (mut open, mut clicks) = (true, Clicks::default());
        let theme = &self.theme;
        widgets::dialog(ctx, theme, "Purge", &mut open, WIDTH, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut dialog.tab, Tab::Purgeable, "Purgeable items");
                ui.selectable_value(
                    &mut dialog.tab,
                    Tab::NonPurgeable,
                    "Find non-purgeable items",
                );
            });
            ui.separator();
            match dialog.tab {
                Tab::Purgeable => purgeable_page(ui, &mut dialog),
                Tab::NonPurgeable => clicks.select = non_purgeable_page(ui, &mut dialog),
            }
            ui.add_space(6.0);
            widgets::group_box(ui, theme, "Unnamed items", |ui| {
                let zero = format!("Zero-length geometry ({})", dialog.zero_length);
                ui.add_enabled(
                    dialog.zero_length > 0,
                    egui::Checkbox::new(&mut options.zero_length, zero),
                );
                let text = format!("Empty text objects ({})", dialog.empty_text);
                ui.add_enabled(
                    dialog.empty_text > 0,
                    egui::Checkbox::new(&mut options.empty_text, text),
                );
            });
            ui.add_space(4.0);
            ui.checkbox(&mut options.confirm, "Confirm each item to be purged");
            ui.checkbox(&mut options.nested, "Purge nested items");
            ui.add_space(8.0);
            let ticked = dialog.items.iter().any(|(_, on)| *on);
            let loose = (options.zero_length && dialog.zero_length > 0)
                || (options.empty_text && dialog.empty_text > 0);
            let anything = !dialog.items.is_empty() || loose;
            ui.horizontal(|ui| {
                ui.add_enabled_ui(ticked || loose, |ui| {
                    clicks.purge = widgets::primary_button(ui, theme, "Purge").clicked();
                });
                ui.add_enabled_ui(anything, |ui| {
                    clicks.all = widgets::secondary_button(ui, theme, "Purge All").clicked();
                });
                clicks.close = widgets::secondary_button(ui, theme, "Close").clicked();
            });
        });
        remember(|m| *m = options);
        open &= !clicks.close;
        if clicks.select {
            let item = dialog
                .detail
                .and_then(|i| dialog.blocked.get(i))
                .map(|b| b.item.clone());
            self.select_users(item, &mut open);
        }
        if clicks.purge || clicks.all {
            let only = clicks.purge.then(|| {
                dialog
                    .items
                    .iter()
                    .filter(|(_, on)| *on)
                    .map(|(p, _)| p.item.clone())
                    .collect::<Vec<Item>>()
            });
            let purge = Purge {
                only,
                nested: options.nested,
                confirm: options.confirm,
                zero_length: options.zero_length,
                empty_text: options.empty_text,
            };
            self.state
                .start_command("PURGE", move |ed| purge_run(ed, purge));
            dialog.stale = true;
        }
        dialog.open = open;
        self.state.purge = dialog;
    }

    /// Close the dialog on the objects that use `item`, selected.
    fn select_users(&mut self, item: Option<Item>, open: &mut bool) {
        let Some(item) = item else { return };
        let users = users_of(self.state.session().document(), &item);
        if users.is_empty() {
            self.state.message =
                "Nothing to select: it is used inside a block or another style.".to_owned();
            return;
        }
        let count = users.len();
        self.state.docs[self.state.active].selection = users.into_iter().collect::<BTreeSet<_>>();
        self.state.message = format!("{count} selected");
        *open = false;
    }
}
