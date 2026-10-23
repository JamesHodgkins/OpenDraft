//! The Home page: shown in place of the drawing and panels while the Home tab
//! is selected. Start something, pick up a recent drawing, jump to an open
//! one, and a reminder of the keys that save the most time.

use std::time::{Duration, Instant, SystemTime};

use eframe::egui;
use od_ui::widgets;

use super::OpenDraftApp;
use crate::home::{recent_entries, size_text, RecentEntry};
use crate::text::{age_text, plural};

/// How long a look at the files on disk is trusted.
const REFRESH: Duration = Duration::from_secs(3);
const PAGE_WIDTH: f32 = 720.0;
const ROW_HEIGHT: f32 = 46.0;

/// Mode keys worth knowing: key, what it does.
const KEYS: &[(&str, &str)] = &[
    ("F3", "Object snap on/off"),
    ("F7", "Grid on/off"),
    ("F8", "Ortho on/off"),
    ("F12", "Dynamic input on/off"),
    ("Ctrl+Z / Ctrl+Y", "Undo / redo"),
    ("Enter, Space", "Repeat the last command"),
];

enum Pick {
    New,
    Open,
    Recent(usize),
    Forget(usize),
    ClearRecent,
    Show(usize),
}

impl OpenDraftApp {
    pub(super) fn home_page(&mut self, ui: &mut egui::Ui) {
        self.refresh_recent_entries();
        let entries = self
            .home_recent
            .as_ref()
            .map(|(_, e)| e.clone())
            .unwrap_or_default();
        let theme = &self.theme;
        let mut pick = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                let side = ((ui.available_width() - PAGE_WIDTH) / 2.0).max(16.0);
                ui.horizontal_top(|ui| {
                    ui.add_space(side);
                    ui.vertical(|ui| {
                        ui.set_width(PAGE_WIDTH.min(ui.available_width() - side));
                        ui.push_id("home_header", |ui| header_section(ui, theme, &mut pick));
                        ui.push_id("home_open_now", |ui| {
                            self.open_drawings_section(ui, &mut pick)
                        });
                        ui.push_id("home_recent", |ui| {
                            recent_section(ui, theme, &entries, &mut pick)
                        });
                        ui.add_space(22.0);
                        ui.push_id("home_keys", |ui| keys_section(ui, theme));
                        ui.add_space(30.0);
                    });
                });
            });
        self.apply_home_pick(pick);
    }

    /// Re-read the recent files' dates and sizes when they are stale or the
    /// list changed.
    fn refresh_recent_entries(&mut self) {
        let paths = self.state.recent.paths();
        let stale = match &self.home_recent {
            Some((at, entries)) => {
                at.elapsed() > REFRESH
                    || entries.len() != paths.len()
                    || entries.iter().zip(paths).any(|(e, p)| e.path != *p)
            }
            None => true,
        };
        if stale {
            self.home_recent = Some((Instant::now(), recent_entries(paths)));
        }
    }

    /// Drawings already open, for jumping between many tabs.
    fn open_drawings_section(&self, ui: &mut egui::Ui, pick: &mut Option<Pick>) {
        let open: Vec<_> = self
            .state
            .docs
            .iter()
            .enumerate()
            .filter(|(_, d)| !d.placeholder)
            .collect();
        if open.is_empty() {
            return;
        }
        widgets::section_header(ui, &self.theme, "Open now");
        for (i, doc) in open {
            let detail = match &doc.path {
                Some(path) => path.display().to_string(),
                None => "Not saved yet".to_owned(),
            };
            let right = plural(doc.session.document().len_here(), "object");
            if row(
                ui,
                &self.theme,
                doc.uid,
                &doc.tab_title(),
                &detail,
                &right,
                false,
            )
            .clicked()
            {
                *pick = Some(Pick::Show(i));
            }
        }
        ui.add_space(22.0);
    }

    fn apply_home_pick(&mut self, pick: Option<Pick>) {
        match pick {
            Some(Pick::New) => self.run("file.new"),
            Some(Pick::Open) => self.run("file.open"),
            Some(Pick::Recent(i)) => self.state.open_recent(i),
            Some(Pick::Forget(i)) => {
                if let Some(path) = self.state.recent.paths().get(i).cloned() {
                    self.state.recent.remove(&path);
                }
            }
            Some(Pick::ClearRecent) => self.state.recent.clear(),
            Some(Pick::Show(i)) => self.state.show_document(i),
            None => {}
        }
    }
}

/// Title, version and the two ways to start.
fn header_section(ui: &mut egui::Ui, theme: &od_ui::Theme, pick: &mut Option<Pick>) {
    let c = &theme.colors;
    ui.add_space(36.0);
    ui.label(
        egui::RichText::new("OpenDraft")
            .size(theme.font_size_heading * 1.6)
            .color(c.text),
    );
    ui.label(
        egui::RichText::new(format!(
            "2D drafting  ·  version {}",
            env!("CARGO_PKG_VERSION")
        ))
        .color(c.text_weak),
    );
    ui.add_space(18.0);
    ui.horizontal(|ui| {
        if widgets::primary_button(ui, theme, "New drawing").clicked() {
            *pick = Some(Pick::New);
        }
        if widgets::secondary_button(ui, theme, "Open...").clicked() {
            *pick = Some(Pick::Open);
        }
        ui.label(
            egui::RichText::new("or drop a .odx, .dxf or .dwg file on the window")
                .color(c.text_weak),
        );
    });
    ui.add_space(26.0);
}

fn keys_section(ui: &mut egui::Ui, theme: &od_ui::Theme) {
    let c = &theme.colors;
    widgets::section_header(ui, theme, "Handy keys");
    egui::Grid::new("grid")
        .num_columns(2)
        .spacing([24.0, 4.0])
        .show(ui, |ui| {
            for (key, what) in KEYS {
                ui.label(egui::RichText::new(*key).strong().color(c.text));
                ui.label(egui::RichText::new(*what).color(c.text_weak));
                ui.end_row();
            }
        });
}

fn recent_section(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    entries: &[RecentEntry],
    pick: &mut Option<Pick>,
) {
    ui.horizontal(|ui| {
        widgets::section_header(ui, theme, "Recent drawings");
        if !entries.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::secondary_button(ui, theme, "Clear list").clicked() {
                    *pick = Some(Pick::ClearRecent);
                }
            });
        }
    });
    if entries.is_empty() {
        ui.label(
            egui::RichText::new("Drawings you open or save will appear here.")
                .color(theme.colors.text_weak),
        );
        return;
    }
    for (i, entry) in entries.iter().enumerate() {
        let name = entry.path.file_name().map_or_else(
            || entry.path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let folder = entry
            .path
            .parent()
            .map_or_else(String::new, |p| p.display().to_string());
        let missing = entry.modified.is_none();
        let right = match (entry.modified, entry.size) {
            (Some(at), Some(size)) => format!("{}  ·  {}", modified_text(at), size_text(size)),
            _ => "Missing".to_owned(),
        };
        let response = row(ui, theme, &entry.path, &name, &folder, &right, missing)
            .on_hover_text(entry.path.display().to_string());
        if response.clicked() {
            *pick = Some(Pick::Recent(i));
        }
        response.context_menu(|ui| {
            if ui.button("Remove from list").clicked() {
                *pick = Some(Pick::Forget(i));
                ui.close_menu();
            }
        });
    }
}

fn modified_text(at: SystemTime) -> String {
    SystemTime::now()
        .duration_since(at)
        .map_or_else(|_| "just now".to_owned(), age_text)
}

/// A full-width clickable line: a name over a weaker detail, and a note at
/// the right.
fn row(
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
