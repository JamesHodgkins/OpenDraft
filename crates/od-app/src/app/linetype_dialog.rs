//! The Linetype Manager (LINETYPE): the drawing's linetypes with a picture and
//! description of each, Load from the stock library, Delete (never one in
//! use) and the global scale factor. It edits a draft of the table and
//! `LTSCALE`; OK commits them as one undo step, Cancel / Escape / X drop it.

use eframe::egui::{self, Align2, FontId, Sense};
use od_core::linetype::stock;
use od_core::{LinetypeId, LinetypeTable};
use od_ui::{widgets, Theme};

use super::OpenDraftApp;
use crate::linetype_ui;

const WIDTH: f32 = 580.0;
const LOAD_WIDTH: f32 = 560.0;
const LIST_HEIGHT: f32 = 260.0;
const ROW_HEIGHT: f32 = 24.0;
/// Where the picture and the description start in a row.
const SAMPLE_AT: f32 = 190.0;
const DESCRIPTION_AT: f32 = SAMPLE_AT + linetype_ui::SAMPLE_WIDTH + 12.0;

/// The working copy the dialog edits.
pub(in crate::app) struct Draft {
    table: LinetypeTable,
    scale: f64,
    selected: Option<LinetypeId>,
    /// While Load is open: whether each stock definition is ticked.
    load: Option<Vec<bool>>,
}

#[derive(Clone, Copy)]
enum Choice {
    Ok,
    Cancel,
    Load,
    Delete(LinetypeId),
    LoadOk,
    LoadCancel,
}

impl OpenDraftApp {
    /// The window opened by `format.linetype`.
    pub(super) fn linetype_window(&mut self, ctx: &egui::Context) {
        if !self.state.show_linetypes {
            self.linetype_draft = None;
            return;
        }
        let mut draft = self.linetype_draft.take().unwrap_or_else(|| {
            let document = &self.state.session().document();
            Draft {
                table: document.linetypes().clone(),
                scale: document.linetype_scale(),
                selected: None,
                load: None,
            }
        });
        let (mut open, mut choice) = (true, None);
        let loading = draft.load.is_some();
        let document = &self.state.session().document();
        widgets::dialog(
            ctx,
            &self.theme,
            "Linetype Manager",
            &mut open,
            WIDTH,
            |ui| {
                let theme = &self.theme;
                ui.add_enabled_ui(!loading, |ui| {
                widgets::group_box(ui, theme, "Linetypes", |ui| list(ui, theme, &mut draft));
                ui.add_space(6.0);
                widgets::group_box(ui, theme, "Details", |ui| {
                    ui.horizontal(|ui| {
                        widgets::field_label(ui, theme, "Global scale factor:");
                        widgets::number_field(ui, theme, &mut draft.scale, "", 0.05);
                    });
                });
                draft.scale = draft.scale.max(1e-6);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if widgets::primary_button(ui, theme, "OK").clicked() {
                        choice = Some(Choice::Ok);
                    }
                    if widgets::secondary_button(ui, theme, "Cancel").clicked() {
                        choice = Some(Choice::Cancel);
                    }
                    if widgets::secondary_button(ui, theme, "Load...").clicked() {
                        choice = Some(Choice::Load);
                    }
                    let deletable = draft
                        .selected
                        .and_then(|id| draft.table.get(id))
                        .filter(|r| !r.id.is_special() && !document.linetype_in_use(&r.name))
                        .map(|r| r.id);
                    let button = ui.add_enabled_ui(deletable.is_some(), |ui| {
                        widgets::secondary_button(ui, theme, "Delete")
                    });
                    if let Some(id) = deletable.filter(|_| button.inner.clicked()) {
                        choice = Some(Choice::Delete(id));
                    }
                    if deletable.is_none() && draft.selected.is_some() {
                        button.response.on_hover_text(
                            "A linetype that is in use, or one of the three built in, cannot be deleted",
                        );
                    }
                });
            });
            },
        );
        if loading {
            choice = self.load_window(ctx, &mut draft).or(choice);
        } else if !open {
            choice = Some(Choice::Cancel);
        }
        match choice {
            Some(Choice::Ok) => {
                self.state.set_linetypes(draft.table, draft.scale);
                self.state.show_linetypes = false;
            }
            Some(Choice::Cancel) => self.state.show_linetypes = false,
            Some(Choice::Load) => {
                draft.load = Some(vec![false; stock::LIBRARY.len()]);
                self.linetype_draft = Some(draft);
            }
            Some(Choice::Delete(id)) => {
                let _ = draft.table.remove(id);
                draft.selected = None;
                self.linetype_draft = Some(draft);
            }
            Some(Choice::LoadOk) => {
                for (ticked, entry) in draft.load.take().iter().flatten().zip(stock::LIBRARY) {
                    if *ticked {
                        let _ = draft.table.load_stock(entry.name);
                    }
                }
                self.linetype_draft = Some(draft);
            }
            Some(Choice::LoadCancel) => {
                draft.load = None;
                self.linetype_draft = Some(draft);
            }
            None => self.linetype_draft = Some(draft),
        }
    }

    /// Load or Reload Linetypes: the stock library, with what the drawing has
    /// already marked and not offered again.
    fn load_window(&mut self, ctx: &egui::Context, draft: &mut Draft) -> Option<Choice> {
        let (mut open, mut choice) = (true, None);
        let theme = &self.theme;
        widgets::dialog(ctx, theme, "Load Linetypes", &mut open, LOAD_WIDTH, |ui| {
            egui::ScrollArea::vertical()
                .max_height(LIST_HEIGHT + 60.0)
                .show(ui, |ui| {
                    let ticks = draft.load.get_or_insert_with(Vec::new);
                    for (i, entry) in stock::LIBRARY.iter().enumerate() {
                        let loaded = draft.table.named(entry.name).is_some();
                        let Some(ticked) = ticks.get_mut(i) else {
                            continue;
                        };
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), ROW_HEIGHT),
                            Sense::click(),
                        );
                        if response.clicked() && !loaded {
                            *ticked = !*ticked;
                        }
                        if response.hovered() {
                            ui.painter().rect_filled(rect, 2.0, theme.colors.hover);
                        }
                        let mark = if loaded {
                            "loaded"
                        } else if *ticked {
                            "\u{2611}"
                        } else {
                            "\u{2610}"
                        };
                        paint_row(ui, theme, rect, entry.name, entry.description, Some(mark));
                        let elements: Vec<_> = entry
                            .pattern
                            .iter()
                            .filter_map(|v| od_core::StrokeElement::from_dxf(*v))
                            .collect();
                        paint_sample(ui, theme, rect, &elements);
                    }
                });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, theme, "OK").clicked() {
                    choice = Some(Choice::LoadOk);
                }
                if widgets::secondary_button(ui, theme, "Cancel").clicked() {
                    choice = Some(Choice::LoadCancel);
                }
            });
        });
        if !open {
            choice = Some(Choice::LoadCancel);
        }
        choice
    }
}

/// The list of linetypes: name, picture and description, one selected.
fn list(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft) {
    egui::ScrollArea::vertical()
        .max_height(LIST_HEIGHT)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let records: Vec<_> = draft
                .table
                .records()
                .iter()
                .filter(|r| r.id != LinetypeId::BYLAYER && r.id != LinetypeId::BYBLOCK)
                .cloned()
                .collect();
            for record in records {
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), ROW_HEIGHT),
                    Sense::click(),
                );
                if response.clicked() {
                    draft.selected = Some(record.id);
                }
                if draft.selected == Some(record.id) {
                    ui.painter().rect_filled(rect, 2.0, theme.colors.accent);
                } else if response.hovered() {
                    ui.painter().rect_filled(rect, 2.0, theme.colors.hover);
                }
                paint_row(ui, theme, rect, &record.name, &record.description, None);
                paint_sample(ui, theme, rect, &record.elements);
            }
        });
}

/// A row's texts: the name, the description and an optional mark before it.
fn paint_row(
    ui: &egui::Ui,
    theme: &Theme,
    rect: egui::Rect,
    name: &str,
    description: &str,
    mark: Option<&str>,
) {
    let painter = ui.painter().with_clip_rect(rect);
    // The description stops short of the mark at the right.
    let text_painter = if mark.is_some() {
        ui.painter()
            .with_clip_rect(rect.with_max_x(rect.right() - 64.0))
    } else {
        painter.clone()
    };
    let font = FontId::proportional(theme.font_size_body);
    let left = rect.left() + 8.0;
    painter.text(
        egui::pos2(left, rect.center().y),
        Align2::LEFT_CENTER,
        name,
        font.clone(),
        theme.colors.text,
    );
    text_painter.text(
        egui::pos2(rect.left() + DESCRIPTION_AT, rect.center().y),
        Align2::LEFT_CENTER,
        description,
        font.clone(),
        theme.colors.text_weak,
    );
    if let Some(mark) = mark {
        painter.text(
            egui::pos2(rect.right() - 8.0, rect.center().y),
            Align2::RIGHT_CENTER,
            mark,
            font,
            theme.colors.text_weak,
        );
    }
}

fn paint_sample(
    ui: &egui::Ui,
    theme: &Theme,
    rect: egui::Rect,
    elements: &[od_core::StrokeElement],
) {
    let area = egui::Rect::from_min_size(
        egui::pos2(rect.left() + SAMPLE_AT, rect.top()),
        egui::vec2(linetype_ui::SAMPLE_WIDTH, rect.height()),
    );
    linetype_ui::paint_sample(ui.painter(), area, elements, theme.colors.text);
}
