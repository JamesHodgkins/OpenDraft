//! The recent drawings as cards: a picture of the drawing over its name, folder
//! and age. One column when the page is narrow, two otherwise.

use std::collections::HashMap;

use eframe::egui;

use super::controls::{entry_controls, BUTTON};
use super::recent::{meta_text, Section};
use super::Pick;
use crate::home_scan::{FileState, RecentEntry};
use crate::recent::RecentId;

/// Below this content width the cards go one to a row.
const TWO_COLUMNS_FROM: f32 = 540.0;
const GAP: f32 = 12.0;
const PAD: f32 = 10.0;
/// The name, folder and note under a card's picture.
const TEXT_HEIGHT: f32 = 68.0;

pub(super) fn grid(ui: &mut egui::Ui, s: &Section, pick: &mut Option<Pick>) {
    let width = ui.available_width();
    let columns = if width >= TWO_COLUMNS_FROM { 2 } else { 1 };
    let card_w = (width - GAP * (columns - 1) as f32) / columns as f32;
    // 16:10, like the pictures.
    let card_h = card_w * 0.625 + TEXT_HEIGHT;
    let rows = s.entries.len().div_ceil(columns);
    let total = rows as f32 * (card_h + GAP) - GAP;
    // Cards are placed by hand, so the page needs telling how much room they take.
    let (area, _) = ui.allocate_exact_size(egui::vec2(width, total.max(0.0)), egui::Sense::hover());
    for (i, entry) in s.entries.iter().enumerate() {
        let (row, column) = (i / columns, i % columns);
        let min =
            area.min + egui::vec2(column as f32 * (card_w + GAP), row as f32 * (card_h + GAP));
        let rect = egui::Rect::from_min_size(min, egui::vec2(card_w, card_h));
        card(ui, s, entry, rect, pick);
    }
}

fn card(
    ui: &mut egui::Ui,
    s: &Section,
    entry: &RecentEntry,
    rect: egui::Rect,
    pick: &mut Option<Pick>,
) {
    // Keyed by the record, so focus and menus stay with the file.
    ui.push_id(entry.id, |ui| {
        let theme = s.theme;
        let c = &theme.colors;
        // First, so the buttons laid over it win the pointer.
        let response = ui
            .interact(rect, ui.id().with("open"), egui::Sense::click())
            .on_hover_text(entry.path.display().to_string());
        if response.clicked() {
            *pick = Some(Pick::Recent(entry.id));
        }
        let radius = f32::from(theme.radius);
        let border = if response.has_focus() {
            egui::Stroke::new(2.0_f32, c.accent_text)
        } else if response.hovered() {
            egui::Stroke::new(1.0_f32, c.border_strong)
        } else {
            egui::Stroke::new(1.0_f32, c.border)
        };
        let painter = ui.painter();
        painter.rect_filled(rect, radius, c.panel_raised);
        let missing = entry.state == FileState::Missing;
        let picture = egui::Rect::from_min_size(
            rect.min + egui::vec2(1.0, 1.0),
            egui::vec2(rect.width() - 2.0, (rect.width() - 2.0) * 0.625),
        );
        picture_or_placeholder(ui, s, entry, picture, missing);
        ui.painter()
            .rect_stroke(rect, radius, border, egui::StrokeKind::Inside);
        badges(ui, s, entry, picture);
        // The words, kept clear of the buttons at the bottom right.
        let controls_w = 2.0 * BUTTON + PAD;
        let text_w = rect.width() - 2.0 * PAD;
        let top = picture.bottom() + 8.0;
        let name = entry.path.file_name().map_or_else(
            || entry.path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let folder = entry
            .path
            .parent()
            .map_or_else(String::new, |p| p.display().to_string());
        let (name_colour, weak) = if missing {
            (c.text_disabled, c.text_disabled)
        } else {
            (c.text, c.text_weak)
        };
        let lines = [
            (name, theme.font_size_body, name_colour, text_w, 0.0),
            (
                folder,
                theme.font_size_small,
                weak,
                text_w - controls_w,
                20.0,
            ),
            (
                meta_text(entry),
                theme.font_size_small,
                weak,
                text_w - controls_w,
                38.0,
            ),
        ];
        for (text, size, colour, room, down) in lines {
            let galley = elided(ui, text, size, colour, room);
            ui.painter()
                .galley(egui::pos2(rect.left() + PAD, top + down), galley, colour);
        }
        let more_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right() - 6.0 - BUTTON, rect.bottom() - 4.0 - BUTTON),
            egui::Vec2::splat(BUTTON),
        );
        entry_controls(ui, theme, entry, s.pinned_full, &response, more_rect, pick);
    });
}

/// The drawing's picture, or a quiet stand-in with its format named; dimmed
/// and marked when the file is not there.
fn picture_or_placeholder(
    ui: &mut egui::Ui,
    s: &Section,
    entry: &RecentEntry,
    picture: egui::Rect,
    missing: bool,
) {
    let c = &s.theme.colors;
    let radius = egui::CornerRadius {
        nw: s.theme.radius.saturating_sub(1),
        ne: s.theme.radius.saturating_sub(1),
        sw: 0,
        se: 0,
    };
    let painter = ui.painter();
    match s.pictures.get(&entry.id) {
        Some(texture) => {
            // Behind the picture, so rounded corners show the card's own colour.
            painter.rect_filled(picture, radius, egui::Color32::WHITE);
            let tint = if missing {
                egui::Color32::from_gray(90)
            } else {
                egui::Color32::WHITE
            };
            painter.image(
                texture.id(),
                picture,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                tint,
            );
        }
        None => {
            painter.rect_filled(picture, radius, c.base);
            let mut x = picture.left() + 24.0;
            while x < picture.right() {
                painter.vline(x, picture.y_range(), egui::Stroke::new(1.0_f32, c.surface));
                x += 24.0;
            }
            let mut y = picture.top() + 24.0;
            while y < picture.bottom() {
                painter.hline(picture.x_range(), y, egui::Stroke::new(1.0_f32, c.surface));
                y += 24.0;
            }
            painter.text(
                picture.center(),
                egui::Align2::CENTER_CENTER,
                format_name(entry),
                egui::FontId::proportional(s.theme.font_size_heading * 1.4),
                c.text_disabled,
            );
        }
    }
    if missing {
        painter.text(
            picture.center() + egui::vec2(0.0, 26.0),
            egui::Align2::CENTER_CENTER,
            "Missing",
            egui::FontId::proportional(s.theme.font_size_body),
            c.text,
        );
    }
}

/// `ODX`, `DXF` or `DWG` (the file's own extension for anything else).
fn format_name(entry: &RecentEntry) -> String {
    entry
        .path
        .extension()
        .map_or_else(String::new, |e| e.to_string_lossy().to_uppercase())
}

/// The format in a corner of the picture, with "Open" and a pin beside it
/// when they apply.
fn badges(ui: &mut egui::Ui, s: &Section, entry: &RecentEntry, picture: egui::Rect) {
    let c = &s.theme.colors;
    let font = egui::FontId::proportional(s.theme.font_size_small);
    let mut x = picture.left() + 8.0;
    let mut labels = vec![format_name(entry)];
    if entry.open_uid.is_some() {
        labels.push("Open".to_owned());
    }
    for label in labels {
        let galley = ui.painter().layout_no_wrap(label, font.clone(), c.text);
        let size = galley.size() + egui::vec2(10.0, 4.0);
        let rect = egui::Rect::from_min_size(egui::pos2(x, picture.top() + 8.0), size);
        ui.painter().rect_filled(rect, 3.0, c.panel_raised);
        ui.painter().rect_stroke(
            rect,
            3.0,
            egui::Stroke::new(1.0_f32, c.border),
            egui::StrokeKind::Inside,
        );
        ui.painter()
            .galley(rect.min + egui::vec2(5.0, 2.0), galley, c.text);
        x = rect.right() + 4.0;
    }
    if entry.pinned {
        let at = egui::pos2(picture.right() - 16.0, picture.top() + 16.0);
        od_ui::icon_set::paint(
            ui.painter(),
            at,
            egui::Align2::CENTER_CENTER,
            od_ui::icons::HOME_PIN,
            16.0,
            c.accent_text,
        );
    }
}

/// `text` on one line no wider than `room`, ending in an ellipsis if it is cut.
fn elided(
    ui: &egui::Ui,
    text: String,
    size: f32,
    colour: egui::Color32,
    room: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple(
        text,
        egui::FontId::proportional(size),
        colour,
        room.max(20.0),
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    ui.painter().layout_job(job)
}

/// What pictures there are, by recent drawing.
pub(super) type Pictures = HashMap<RecentId, egui::TextureHandle>;
