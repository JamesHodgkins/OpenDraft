//! The Purge dialog (PURGE), laid out like AutoCAD's: the tree of unused
//! named items on the left; a preview and the options on the right; Purge
//! Checked Items, Purge All and Close underneath. A second page says why the
//! rest has to stay. The lists are `od_core::purge`; deleting is
//! `purge_cmd.rs`; the tree is `purge_tree.rs`.

use std::collections::BTreeSet;

use eframe::egui;
use od_core::purge::{users_of, Item};
use od_ui::widgets;

mod purge_tree;

use purge_tree::{blocked_list, purgeable_tree};

use super::OpenDraftApp;
use crate::purge_cmd::{block_picture, purge_run, Dialog, Purge, Tab};
use crate::purge_memory::{memory, remember, Memory};

const WIDTH: f32 = 700.0;
const LEFT: f32 = 310.0;
const RIGHT: f32 = 350.0;
const PANE: f32 = 330.0;
const PREVIEW: f32 = 150.0;

/// What a click in the dialog asks for.
#[derive(Default)]
struct Clicks {
    purge: bool,
    all: bool,
    close: bool,
    select: bool,
}

fn tab_button(ui: &mut egui::Ui, dialog: &mut Dialog, tab: Tab, text: &str) {
    let button = egui::Button::new(text)
        .selected(dialog.tab == tab)
        .min_size(egui::vec2(200.0, 28.0));
    if ui.add(button).clicked() {
        dialog.tab = tab;
    }
}

/// The picture of the selected item, or the reasons an item stays.
fn top_right(ui: &mut egui::Ui, dialog: &Dialog, select: &mut bool) {
    ui.set_min_height(PREVIEW);
    ui.set_width(RIGHT - 28.0);
    match dialog.tab {
        Tab::Purgeable => {
            if let Some((_, texture)) = &dialog.preview {
                let size = egui::vec2(RIGHT - 28.0, PREVIEW);
                ui.add(
                    egui::Image::new((texture.id(), texture.size_vec2()))
                        .max_size(size)
                        .maintain_aspect_ratio(true),
                );
            }
        }
        Tab::NonPurgeable => {
            if let Some(b) = dialog.detail.and_then(|i| dialog.blocked.get(i)) {
                ui.label(format!("\"{}\" stays because:", b.name));
                for reason in &b.reasons {
                    ui.label(format!("  \u{2022} {reason}"));
                }
                *select = ui.button("Select Objects").clicked();
            } else {
                ui.label("Pick an item to see why it stays.");
            }
        }
    }
}

fn options(ui: &mut egui::Ui, theme: &od_ui::Theme, o: &mut Memory, dialog: &Dialog) {
    ui.checkbox(&mut o.confirm, "Confirm each item to be purged");
    ui.checkbox(&mut o.nested, "Purge nested items");
    ui.add_space(4.0);
    widgets::group_box(ui, theme, "Purge Unnamed Objects", |ui| {
        let zero = format!("Zero-length geometry ({})", dialog.zero_length);
        ui.add_enabled(
            dialog.zero_length > 0,
            egui::Checkbox::new(&mut o.zero_length, zero),
        );
        let empty = format!("Empty text objects ({})", dialog.empty_text);
        ui.add_enabled(
            dialog.empty_text > 0,
            egui::Checkbox::new(&mut o.empty_text, empty),
        );
        let mut off = false;
        ui.add_enabled(false, egui::Checkbox::new(&mut off, "Orphaned data"))
            .on_disabled_hover_text("OpenDraft does not store this.");
    });
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
        let mut o = memory();
        let (mut open, mut clicks) = (true, Clicks::default());
        let mut picked = None;
        let theme = &self.theme;
        let width = LEFT + RIGHT + 12.0;
        widgets::dialog(ctx, theme, "Purge", &mut open, width.max(WIDTH), |ui| {
            ui.horizontal(|ui| {
                tab_button(ui, &mut dialog, Tab::Purgeable, "Purgeable Items");
                tab_button(
                    ui,
                    &mut dialog,
                    Tab::NonPurgeable,
                    "Find Non-Purgeable Items",
                );
            });
            ui.add_space(6.0);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(LEFT);
                    let title = match dialog.tab {
                        Tab::Purgeable => "Named Items Not Used",
                        Tab::NonPurgeable => "Named Items That Stay",
                    };
                    ui.strong(title);
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_width(LEFT - 20.0);
                        egui::ScrollArea::vertical()
                            .id_salt("purge_tree")
                            .auto_shrink([false, false])
                            .max_height(PANE)
                            .min_scrolled_height(PANE)
                            .show(ui, |ui| match dialog.tab {
                                Tab::Purgeable => picked = purgeable_tree(ui, &mut dialog).0,
                                Tab::NonPurgeable => blocked_list(ui, &mut dialog),
                            });
                    });
                });
                ui.vertical(|ui| {
                    ui.set_width(RIGHT);
                    let title = match dialog.tab {
                        Tab::Purgeable => "Preview",
                        Tab::NonPurgeable => "Why it stays",
                    };
                    widgets::group_box(ui, theme, title, |ui| {
                        top_right(ui, &dialog, &mut clicks.select);
                    });
                    ui.add_space(6.0);
                    widgets::group_box(ui, theme, "Options", |ui| {
                        options(ui, theme, &mut o, &dialog);
                    });
                });
            });
            ui.add_space(8.0);
            let ticked = dialog.items.iter().any(|(_, on)| *on);
            let loose = (o.zero_length && dialog.zero_length > 0)
                || (o.empty_text && dialog.empty_text > 0);
            let anything = !dialog.items.is_empty() || loose;
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                clicks.close = widgets::secondary_button(ui, theme, "Close").clicked();
                ui.add_enabled_ui(anything, |ui| {
                    clicks.all = widgets::secondary_button(ui, theme, "Purge All").clicked();
                });
                ui.add_enabled_ui(ticked || loose, |ui| {
                    clicks.purge =
                        widgets::primary_button(ui, theme, "Purge Checked Items").clicked();
                });
            });
        });
        remember(|m| *m = o);
        open &= !clicks.close;
        if let Some(item) = picked {
            self.show_preview(ctx, &mut dialog, item);
        }
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
                nested: o.nested,
                confirm: o.confirm,
                zero_length: o.zero_length,
                empty_text: o.empty_text,
            };
            self.state
                .start_command("PURGE", move |ed| purge_run(ed, purge));
            dialog.stale = true;
        }
        dialog.open = open;
        self.state.purge = dialog;
    }

    /// Select `item` in the tree and picture it (blocks only).
    fn show_preview(&mut self, ctx: &egui::Context, dialog: &mut Dialog, item: Item) {
        dialog.preview = None;
        if let Item::Block(id) = &item {
            let picture = block_picture(self.state.session().document(), *id);
            if let Some(p) = picture {
                let size = [p.width as usize, p.height as usize];
                let image = egui::ColorImage::from_rgba_unmultiplied(size, &p.rgba);
                let texture =
                    ctx.load_texture("purge_preview", image, egui::TextureOptions::LINEAR);
                dialog.preview = Some((item.clone(), texture));
            }
        }
        dialog.selected = Some(item);
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
