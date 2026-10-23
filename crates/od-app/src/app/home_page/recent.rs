//! The Recent drawings section of the Home page, and the row both it and
//! "Open now" are made of.

use std::time::SystemTime;

use eframe::egui;
use od_ui::icon_set;
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

/// Width and height of the Pin and More buttons at the end of a row.
const BUTTON: f32 = 28.0;
const BUTTON_ICON: f32 = 16.0;

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
        response.context_menu(|ui| {
            if let Some(chosen) = actions(ui, entry, pinned_full) {
                *pick = Some(chosen);
                ui.close_menu();
            }
        });
        // The buttons come after the row, so Tab visits the row, Pin, More.
        let centre = response.rect.right_center();
        let more_rect = egui::Rect::from_center_size(
            egui::pos2(centre.x - 8.0 - BUTTON / 2.0, centre.y),
            egui::Vec2::splat(BUTTON),
        );
        let pin_rect = more_rect.translate(egui::vec2(-BUTTON, 0.0));
        let pin_disabled = !entry.pinned && pinned_full;
        let pin = icon_button(ui, theme, pin_rect, od_ui::icons::HOME_PIN, entry.pinned, {
            !pin_disabled
        });
        let pin = if pin_disabled {
            pin.on_hover_text(format!("You can pin up to {MAX_PINNED} drawings"))
        } else {
            pin.on_hover_text(if entry.pinned { "Unpin" } else { "Pin" })
        };
        if pin.clicked() {
            *pick = Some(if entry.pinned {
                Pick::Unpin(entry.id)
            } else {
                Pick::Pin(entry.id)
            });
        }
        let more = icon_button(ui, theme, more_rect, od_ui::icons::HOME_MORE, false, true)
            .on_hover_text("More actions");
        let menu_id = ui.id().with("more");
        if more.clicked() {
            ui.memory_mut(|m| m.toggle_popup(menu_id));
        }
        egui::popup_below_widget(
            ui,
            menu_id,
            &more,
            egui::PopupCloseBehavior::CloseOnClick,
            |ui| {
                ui.set_min_width(150.0);
                if let Some(chosen) = actions(ui, entry, pinned_full) {
                    *pick = Some(chosen);
                }
            },
        );
    });
}

/// A small icon button in `rect`, lit while `on`. Its id comes from `icon`,
/// not from where it was added, so focus stays with it when the list moves.
fn icon_button(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    rect: egui::Rect,
    icon: char,
    on: bool,
    enabled: bool,
) -> egui::Response {
    let c = &theme.colors;
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let response = ui.interact(rect, ui.id().with(icon), sense);
    if enabled && (response.hovered() || on) {
        ui.painter().rect_filled(rect, 4.0, c.hover);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(2.0_f32, c.accent),
            egui::StrokeKind::Inside,
        );
    }
    let colour = match (enabled, on) {
        (false, _) => c.text_disabled,
        (true, true) => c.text,
        (true, false) => c.text_weak,
    };
    icon_set::paint(
        ui.painter(),
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        BUTTON_ICON,
        colour,
    );
    response
}

/// The things that can be done to a recent drawing. Copy path is done here;
/// the rest is for the caller to carry out.
fn actions(ui: &mut egui::Ui, entry: &RecentEntry, pinned_full: bool) -> Option<Pick> {
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
    if entry.state == FileState::Missing && ui.button("Locate...").clicked() {
        chosen = Some(Pick::Locate(entry.id));
    }
    if ui.button("Remove from list").clicked() {
        chosen = Some(Pick::Forget(entry.id));
    }
    chosen
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
            egui::Stroke::new(2.0_f32, c.accent),
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
