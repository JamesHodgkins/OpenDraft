//! The left pane of the Purge dialog: the tree of named items nothing uses
//! (every kind AutoCAD lists, ticked or not), or the list of items that have
//! to stay.

use eframe::egui;
use od_core::purge::{Category, Item};

use crate::purge_cmd::Dialog;

/// The kinds in the order AutoCAD's tree lists them: ours, and the ones
/// OpenDraft has no table of.
#[derive(Clone, Copy)]
enum Row {
    Named(Category),
    Missing(&'static str),
}

const ROWS: [Row; 15] = [
    Row::Named(Category::Blocks),
    Row::Missing("Detail view styles"),
    Row::Named(Category::DimStyles),
    Row::Missing("Groups"),
    Row::Named(Category::Layers),
    Row::Named(Category::Linetypes),
    Row::Missing("Materials"),
    Row::Missing("Mline styles"),
    Row::Named(Category::MLeaderStyles),
    Row::Missing("Plot styles"),
    Row::Missing("Section view styles"),
    Row::Missing("Shapes"),
    Row::Missing("Table styles"),
    Row::Named(Category::TextStyles),
    Row::Missing("Visual styles"),
];

/// A checkbox that is half ticked when only some of `states` are.
fn tri_state(ui: &mut egui::Ui, label: &str, states: &[bool]) -> Option<bool> {
    let on = states.iter().filter(|s| **s).count();
    let mut value = !states.is_empty() && on == states.len();
    let partial = on > 0 && on < states.len();
    let response = ui.add(egui::Checkbox::new(&mut value, label).indeterminate(partial));
    response.changed().then_some(value)
}

/// A row for a kind with nothing to purge, switched off.
fn empty_row(ui: &mut egui::Ui, label: &str, why: &str) {
    ui.horizontal(|ui| {
        ui.add_space(ui.spacing().indent);
        let mut off = false;
        ui.add_enabled(false, egui::Checkbox::new(&mut off, label))
            .on_disabled_hover_text(why);
    });
}

/// What a click on an item's name asks for.
#[derive(Default)]
pub(super) struct Picked(pub Option<Item>);

/// The tree of purgeable items; returns the item whose name was clicked.
pub(super) fn purgeable_tree(ui: &mut egui::Ui, dialog: &mut Dialog) -> Picked {
    let mut picked = Picked::default();
    let all: Vec<bool> = dialog.items.iter().map(|(_, on)| *on).collect();
    ui.horizontal(|ui| {
        if let Some(v) = tri_state(ui, "All items", &all) {
            dialog.items.iter_mut().for_each(|(_, on)| *on = v);
        }
    });
    for row in ROWS {
        match row {
            Row::Missing(name) => empty_row(ui, name, "OpenDraft does not store these."),
            Row::Named(category) => {
                let rows: Vec<usize> = (0..dialog.items.len())
                    .filter(|&i| dialog.items[i].0.item.category() == category)
                    .collect();
                if rows.is_empty() {
                    empty_row(ui, category.title(), "Nothing of this kind is unused.");
                    continue;
                }
                let id = ui.make_persistent_id(("purge", category as u8));
                egui::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    id,
                    false,
                )
                .show_header(ui, |ui| {
                    let states: Vec<bool> = rows.iter().map(|&i| dialog.items[i].1).collect();
                    let label = format!("{} ({})", category.title(), rows.len());
                    if let Some(v) = tri_state(ui, &label, &states) {
                        rows.iter().for_each(|&i| dialog.items[i].1 = v);
                    }
                })
                .body(|ui| {
                    for &i in &rows {
                        let selected = dialog.selected.as_ref() == Some(&dialog.items[i].0.item);
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut dialog.items[i].1, "");
                            let name = dialog.items[i].0.name.clone();
                            if ui.selectable_label(selected, name).clicked() {
                                picked.0 = Some(dialog.items[i].0.item.clone());
                            }
                        });
                    }
                });
            }
        }
    }
    picked
}

/// The list of items that stay; returns whether a row was clicked.
pub(super) fn blocked_list(ui: &mut egui::Ui, dialog: &mut Dialog) {
    if dialog.blocked.is_empty() {
        ui.label("Everything here is in use.");
    }
    for row in ROWS {
        let Row::Named(category) = row else { continue };
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
}
