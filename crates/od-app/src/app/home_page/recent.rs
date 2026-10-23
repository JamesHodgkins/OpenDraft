//! The Recent drawings section of the Home page, and the row both it and
//! "Open now" are made of.

use std::time::SystemTime;

use eframe::egui;
use od_ui::widgets;

use super::Pick;
use crate::home::size_text;
use crate::home_scan::{FileState, RecentEntry};
use crate::recent::MAX_PINNED;
use crate::text::{age_text, plural};

const ROW_HEIGHT: f32 = 46.0;

pub(super) fn recent_section(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    entries: &[RecentEntry],
    pinned_full: bool,
    pick: &mut Option<Pick>,
) {
    let unpinned = entries.iter().filter(|e| !e.pinned).count();
    let confirm_id = ui.id().with("confirm_clear");
    let confirming = ui.data(|d| d.get_temp::<bool>(confirm_id)).unwrap_or(false) && unpinned > 1;
    ui.horizontal(|ui| {
        widgets::section_header(ui, theme, "Recent drawings");
        if unpinned == 0 {
            return;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if confirming {
                if widgets::secondary_button(ui, theme, "Cancel").clicked() {
                    ui.data_mut(|d| d.insert_temp(confirm_id, false));
                }
                if widgets::primary_button(ui, theme, "Remove").clicked() {
                    ui.data_mut(|d| d.insert_temp(confirm_id, false));
                    *pick = Some(Pick::ClearUnpinned);
                }
                ui.label(
                    egui::RichText::new(format!(
                        "Remove {} from the list?",
                        plural(unpinned, "drawing")
                    ))
                    .color(theme.colors.text_weak),
                );
            } else if widgets::secondary_button(ui, theme, "Clear unpinned").clicked() {
                if unpinned > 1 {
                    ui.data_mut(|d| d.insert_temp(confirm_id, true));
                } else {
                    *pick = Some(Pick::ClearUnpinned);
                }
            }
        });
    });
    if entries.is_empty() {
        ui.label(
            egui::RichText::new("Drawings you open or save will appear here.")
                .color(theme.colors.text_weak),
        );
        return;
    }
    for entry in entries {
        recent_row(ui, theme, entry, pinned_full, pick);
    }
}

fn recent_row(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    entry: &RecentEntry,
    pinned_full: bool,
    pick: &mut Option<Pick>,
) {
    let name = entry.path.file_name().map_or_else(
        || entry.path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let folder = entry
        .path
        .parent()
        .map_or_else(String::new, |p| p.display().to_string());
    let mut notes = Vec::new();
    if entry.pinned {
        notes.push("Pinned".to_owned());
    }
    if entry.open_uid.is_some() {
        notes.push("Open".to_owned());
    }
    let missing = entry.state == FileState::Missing;
    match &entry.state {
        FileState::Pending => {}
        FileState::Present { modified, size } => {
            if let Some(at) = modified {
                notes.push(modified_text(*at));
            }
            notes.push(size_text(*size));
        }
        FileState::Missing => notes.push("Missing".to_owned()),
    }
    let response = row(
        ui,
        theme,
        entry.id,
        &name,
        &folder,
        &notes.join("  ·  "),
        missing,
    )
    .on_hover_text(entry.path.display().to_string());
    if response.clicked() {
        *pick = Some(Pick::Recent(entry.id));
    }
    response.context_menu(|ui| {
        let mut chosen = None;
        if ui.button("Open").clicked() {
            chosen = Some(Pick::Recent(entry.id));
        }
        if entry.pinned {
            if ui.button("Unpin").clicked() {
                chosen = Some(Pick::Unpin(entry.id));
            }
        } else if ui
            .add_enabled(!pinned_full, egui::Button::new("Pin"))
            .on_disabled_hover_text(format!("You can pin up to {MAX_PINNED} drawings"))
            .clicked()
        {
            chosen = Some(Pick::Pin(entry.id));
        }
        if ui.button("Copy path").clicked() {
            ui.ctx().copy_text(entry.path.display().to_string());
            ui.close_menu();
        }
        if entry.path.parent().is_some_and(std::path::Path::is_dir)
            && ui.button("Show in folder").clicked()
        {
            chosen = Some(Pick::ShowInFolder(entry.path.clone()));
        }
        if missing && ui.button("Locate...").clicked() {
            chosen = Some(Pick::Locate(entry.id));
        }
        if ui.button("Remove from list").clicked() {
            chosen = Some(Pick::Forget(entry.id));
        }
        if chosen.is_some() {
            *pick = chosen;
            ui.close_menu();
        }
    });
}

fn modified_text(at: SystemTime) -> String {
    SystemTime::now()
        .duration_since(at)
        .map_or_else(|_| "just now".to_owned(), age_text)
}

/// A full-width clickable line: a name over a weaker detail, and a note at
/// the right. `id` keys it by what it is for.
pub(super) fn row(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    id: impl std::hash::Hash,
    name: &str,
    detail: &str,
    right: &str,
    dim: bool,
) -> egui::Response {
    let c = &theme.colors;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::hover(),
    );
    // Keyed by what the row is for, so it keeps its state when the list moves.
    let response = ui.interact(rect, ui.id().with(id), egui::Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 6.0, c.hover);
    }
    let pad = 12.0;
    let right_width = ui
        .painter()
        .layout_no_wrap(
            right.to_owned(),
            egui::FontId::proportional(theme.font_size_small),
            c.text_weak,
        )
        .size()
        .x;
    let text_room = (rect.width() - 2.0 * pad - right_width - 16.0).max(40.0);
    let clip = egui::Rect::from_min_size(rect.min, egui::vec2(text_room + pad, rect.height()));
    let painter = ui.painter().with_clip_rect(clip);
    let (name_colour, detail_colour) = if dim {
        (c.text_disabled, c.text_disabled)
    } else {
        (c.text, c.text_weak)
    };
    painter.text(
        egui::pos2(rect.left() + pad, rect.top() + 14.0),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(theme.font_size_body),
        name_colour,
    );
    painter.text(
        egui::pos2(rect.left() + pad, rect.top() + 32.0),
        egui::Align2::LEFT_CENTER,
        detail,
        egui::FontId::proportional(theme.font_size_small),
        detail_colour,
    );
    ui.painter().text(
        egui::pos2(rect.right() - pad, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        right,
        egui::FontId::proportional(theme.font_size_small),
        c.text_weak,
    );
    response
}
