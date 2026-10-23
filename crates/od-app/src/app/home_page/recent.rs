//! The Recent drawings section of the Home page, and the row both it and
//! "Open now" are made of.

use std::time::SystemTime;

use eframe::egui;
use od_ui::widgets;

use super::cards::{self, Pictures};
use super::controls::{entry_controls, BUTTON};
use super::layout::bar;
use super::Pick;
use crate::home::size_text;
use crate::home_scan::{FileState, RecentEntry};
use crate::recent::RecentView;
use crate::text::{age_text, plural};

const ROW_HEIGHT: f32 = 46.0;

/// What the Recent drawings section needs to draw itself.
pub(super) struct Section<'a> {
    pub theme: &'a od_ui::Theme,
    pub entries: &'a [RecentEntry],
    pub pinned_full: bool,
    pub pictures: &'a Pictures,
    pub view: RecentView,
}

pub(super) fn recent_section(ui: &mut egui::Ui, s: &Section, pick: &mut Option<Pick>) {
    let (theme, entries) = (s.theme, s.entries);
    let unpinned = entries.iter().filter(|e| !e.pinned).count();
    let confirm_id = ui.id().with("confirm_clear");
    let confirming = ui.data(|d| d.get_temp::<bool>(confirm_id)).unwrap_or(false) && unpinned > 1;
    bar(
        ui,
        |ui| {
            widgets::section_header(ui, theme, "Recent drawings");
        },
        |ui| {
            let mut chosen = usize::from(s.view == RecentView::List);
            if widgets::tab_strip(ui, theme, &mut chosen, &["Grid", "List"]) {
                *pick = Some(Pick::SetView(if chosen == 0 {
                    RecentView::Grid
                } else {
                    RecentView::List
                }));
            }
            if unpinned == 0 {
                return;
            }
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
        },
    );
    if entries.is_empty() {
        ui.label(
            egui::RichText::new("Drawings you open or save will appear here.")
                .color(theme.colors.text_weak),
        );
        return;
    }
    match s.view {
        RecentView::Grid => cards::grid(ui, s, pick),
        RecentView::List => {
            for entry in entries {
                recent_row(ui, theme, entry, s.pinned_full, pick);
            }
        }
    }
}

/// What is known of the file: its age and size, or that it is missing.
pub(super) fn meta_text(entry: &RecentEntry) -> String {
    match &entry.state {
        FileState::Pending => String::new(),
        FileState::Present { modified, size } => match modified {
            Some(at) => format!("{}  ·  {}", modified_text(*at), size_text(*size)),
            None => size_text(*size),
        },
        FileState::Missing => "Missing".to_owned(),
    }
}

fn recent_row(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    entry: &RecentEntry,
    pinned_full: bool,
    pick: &mut Option<Pick>,
) {
    // Everything in the row is keyed by the record, so focus and an open menu
    // stay with the file when the list is reordered.
    ui.push_id(entry.id, |ui| {
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
        notes.push(meta_text(entry));
        notes.retain(|n| !n.is_empty());
        let response = row(
            ui,
            theme,
            "open",
            &RowText {
                name: &name,
                detail: &folder,
                right: &notes.join("  ·  "),
                dim: missing,
                trailing: 2.0 * BUTTON + 8.0,
            },
        )
        .on_hover_text(entry.path.display().to_string());
        if response.clicked() {
            *pick = Some(Pick::Recent(entry.id));
        }
        // The buttons come after the row, so Tab visits the row, Pin, More.
        let centre = response.rect.right_center();
        let more_rect = egui::Rect::from_center_size(
            egui::pos2(centre.x - 8.0 - BUTTON / 2.0, centre.y),
            egui::Vec2::splat(BUTTON),
        );
        entry_controls(ui, theme, entry, pinned_full, &response, more_rect, pick);
    });
}

fn modified_text(at: SystemTime) -> String {
    SystemTime::now()
        .duration_since(at)
        .map_or_else(|_| "just now".to_owned(), age_text)
}

/// What a [`row`] says.
#[derive(Clone, Copy)]
pub(super) struct RowText<'a> {
    pub name: &'a str,
    pub detail: &'a str,
    pub right: &'a str,
    /// Greyed, for a file that is not there.
    pub dim: bool,
    /// Room kept free at the end for buttons.
    pub trailing: f32,
}

/// A full-width clickable line: a name over a weaker detail, and a note at
/// the right, leaving `trailing` at the end free for buttons. `id` keys it by
/// what it is for. Tab reaches it, Enter or Space click it, and it shows a ring
/// while it has focus.
pub(super) fn row(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    id: impl std::hash::Hash,
    text: &RowText,
) -> egui::Response {
    let RowText {
        name,
        detail,
        right,
        dim,
        trailing,
    } = *text;
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
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            6.0,
            egui::Stroke::new(2.0_f32, c.accent_text),
            egui::StrokeKind::Inside,
        );
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
    let text_room = (rect.width() - 2.0 * pad - right_width - 16.0 - trailing).max(40.0);
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
        egui::pos2(rect.right() - pad - trailing, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        right,
        egui::FontId::proportional(theme.font_size_small),
        c.text_weak,
    );
    response
}
