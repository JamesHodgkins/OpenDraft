//! The Start section: the three ways to begin, as tiles at the top of the
//! page. New drawing and Open go straight to their commands; From template
//! opens a dropdown of the starters built into the program and the user's own
//! templates, read from their folder on a worker thread when it is opened.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use eframe::egui;
use od_ui::{icon_set, icons, menu, widgets};

use super::Pick;
use crate::templates::{scan, starters, Source, Template};

/// Below this width the tiles go one to a row.
const STACK_BELOW: f32 = 480.0;
const GAP: f32 = 12.0;
const TILE_HEIGHT: f32 = 64.0;
const MENU_WIDTH: f32 = 320.0;
const ROW_HEIGHT: f32 = 46.0;

/// The templates the dropdown lists.
pub struct TemplateList {
    starters: Vec<Template>,
    /// The user's, once their folder has been read.
    user: Option<Vec<Template>>,
    reading: Option<Receiver<Vec<Template>>>,
}

impl Default for TemplateList {
    fn default() -> Self {
        Self {
            starters: starters(),
            user: None,
            reading: None,
        }
    }
}

impl TemplateList {
    /// Start reading the user's folder (none: there is nothing to read).
    pub fn refresh(&mut self, ctx: &egui::Context, folder: Option<PathBuf>) {
        self.user = None;
        match folder {
            Some(folder) => {
                let (tx, rx) = channel();
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    // The dropdown may have been closed; then no one wants it.
                    let _ = tx.send(scan(&folder));
                    ctx.request_repaint();
                });
                self.reading = Some(rx);
            }
            None => {
                self.reading = None;
                self.user = Some(Vec::new());
            }
        }
    }

    /// Take in the folder's templates when they have been read.
    pub fn collect(&mut self) {
        if let Some(rx) = &self.reading {
            if let Ok(found) = rx.try_recv() {
                self.user = Some(found);
                self.reading = None;
            }
        }
    }
}

pub(super) fn start_section(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    list: &TemplateList,
    folder: Option<&Path>,
    pick: &mut Option<Pick>,
) {
    widgets::section_header(ui, theme, "Start");
    let width = ui.available_width();
    let across = if width >= STACK_BELOW { 3 } else { 1 };
    let tile_w = (width - GAP * (across - 1) as f32) / across as f32;
    let rows = 3_usize.div_ceil(across);
    let total = rows as f32 * (TILE_HEIGHT + GAP) - GAP;
    // Placed by hand, so the page needs telling how much room they take.
    let (area, _) = ui.allocate_exact_size(egui::vec2(width, total), egui::Sense::hover());
    let tiles = [
        Tile {
            id: "new",
            icon: icons::FILE_NEW,
            title: "New drawing",
            subtitle: "A blank drawing",
            primary: true,
            dropdown: false,
        },
        Tile {
            id: "open",
            icon: icons::FILE_OPEN,
            title: "Open...",
            subtitle: "ODX, DXF or DWG",
            primary: false,
            dropdown: false,
        },
        Tile {
            id: "template",
            icon: icons::HOME_TEMPLATE,
            title: "From template",
            subtitle: "Starters and your own",
            primary: false,
            dropdown: true,
        },
    ];
    for (i, tile) in tiles.iter().enumerate() {
        let (row, column) = (i / across, i % across);
        let min = area.min
            + egui::vec2(
                column as f32 * (tile_w + GAP),
                row as f32 * (TILE_HEIGHT + GAP),
            );
        let rect = egui::Rect::from_min_size(min, egui::vec2(tile_w, TILE_HEIGHT));
        let response = ui.push_id(tile.id, |ui| tile.show(ui, theme, rect)).inner;
        match tile.id {
            "new" if response.clicked() => *pick = Some(Pick::New),
            "open" if response.clicked() => *pick = Some(Pick::Open),
            "template" => {
                // Read the folder each time the dropdown is opened.
                if response.clicked() {
                    *pick = Some(Pick::ScanTemplates);
                }
                let chosen = menu::popup(
                    ui,
                    theme,
                    response.id.with("menu"),
                    &response,
                    MENU_WIDTH,
                    |ui| template_menu(ui, theme, list, folder),
                );
                if chosen.is_some() {
                    *pick = chosen;
                }
            }
            _ => {}
        }
    }
    ui.add_space(22.0);
}

struct Tile {
    id: &'static str,
    icon: char,
    title: &'static str,
    subtitle: &'static str,
    /// The one action the page leads with, in the accent colour.
    primary: bool,
    /// Opens a menu, shown by a small arrow.
    dropdown: bool,
}

impl Tile {
    fn show(&self, ui: &mut egui::Ui, theme: &od_ui::Theme, rect: egui::Rect) -> egui::Response {
        let c = &theme.colors;
        let response = ui.interact(rect, ui.id().with("tile"), egui::Sense::click());
        let radius = f32::from(theme.radius);
        let (fill, title, subtitle) = match (self.primary, response.hovered()) {
            (true, false) => (c.accent, c.text_on_accent, c.text_on_accent),
            (true, true) => (c.accent_hover, c.text_on_accent, c.text_on_accent),
            (false, false) => (c.panel_raised, c.text, c.text_weak),
            (false, true) => (c.hover, c.text, c.text_weak),
        };
        let painter = ui.painter();
        painter.rect_filled(rect, radius, fill);
        let stroke = if response.has_focus() {
            egui::Stroke::new(2.0_f32, c.accent_text)
        } else if self.primary {
            egui::Stroke::NONE
        } else {
            egui::Stroke::new(1.0_f32, c.border)
        };
        painter.rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
        icon_set::paint(
            painter,
            egui::pos2(rect.left() + 28.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            self.icon,
            24.0,
            title,
        );
        let text_left = rect.left() + 54.0;
        let room = egui::Rect::from_min_max(
            egui::pos2(text_left, rect.top()),
            egui::pos2(rect.right() - 28.0, rect.bottom()),
        );
        let clipped = painter.with_clip_rect(room);
        clipped.text(
            egui::pos2(text_left, rect.center().y - 10.0),
            egui::Align2::LEFT_CENTER,
            self.title,
            egui::FontId::proportional(theme.font_size_body),
            title,
        );
        clipped.text(
            egui::pos2(text_left, rect.center().y + 10.0),
            egui::Align2::LEFT_CENTER,
            self.subtitle,
            egui::FontId::proportional(theme.font_size_small),
            subtitle,
        );
        if self.dropdown {
            icon_set::paint(
                painter,
                egui::pos2(rect.right() - 16.0, rect.center().y),
                egui::Align2::CENTER_CENTER,
                icons::RIBBON_SPLIT_ARROW,
                14.0,
                c.text_weak,
            );
        }
        response
    }
}

/// The dropdown: starters, then the user's templates, then the folder.
fn template_menu(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    list: &TemplateList,
    folder: Option<&Path>,
) -> Option<Pick> {
    let weak = theme.colors.text_weak;
    let mut chosen = None;
    let label = |ui: &mut egui::Ui, text: &str| {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(text).small().color(weak));
        ui.add_space(2.0);
    };
    label(ui, "STARTERS");
    for template in &list.starters {
        if template_row(ui, theme, template).clicked() {
            chosen = Some(Pick::NewFromTemplate(template.clone()));
        }
    }
    label(ui, "YOUR TEMPLATES");
    match &list.user {
        None => {
            ui.label(egui::RichText::new("  Looking...").color(weak));
        }
        Some(found) if found.is_empty() => {
            ui.label(egui::RichText::new("  None yet: use Save as Template").color(weak));
        }
        Some(found) => {
            for template in found {
                if template_row(ui, theme, template).clicked() {
                    chosen = Some(Pick::NewFromTemplate(template.clone()));
                }
            }
        }
    }
    if folder.is_some() {
        ui.separator();
        if menu::menu_row(ui, theme, "Open templates folder", false) {
            chosen = Some(Pick::OpenTemplatesFolder);
        }
    }
    chosen
}

/// A template as a clickable line: its name over what it sets up, or why it
/// is unavailable (which cannot be picked).
fn template_row(ui: &mut egui::Ui, theme: &od_ui::Theme, template: &Template) -> egui::Response {
    let c = &theme.colors;
    let usable = template.summary.is_ok();
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::hover(),
    );
    let id = ui.id().with(match &template.source {
        Source::Starter(s) => format!("starter-{s:?}"),
        Source::File(path) => path.display().to_string(),
    });
    let sense = if usable {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let response = ui.interact(rect, id, sense);
    if usable && response.hovered() {
        ui.painter().rect_filled(rect, 2.0, c.hover);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(2.0_f32, c.accent_text),
            egui::StrokeKind::Inside,
        );
    }
    let (name_colour, detail_colour) = if usable {
        (c.text, c.text_weak)
    } else {
        (c.text_disabled, c.text_disabled)
    };
    let detail = match &template.summary {
        Ok(summary) => summary.clone(),
        Err(why) => format!("Unavailable: {why}"),
    };
    let painter = ui.painter().with_clip_rect(rect);
    painter.text(
        egui::pos2(rect.left() + 8.0, rect.top() + 14.0),
        egui::Align2::LEFT_CENTER,
        &template.name,
        egui::FontId::proportional(theme.font_size_body),
        name_colour,
    );
    painter.text(
        egui::pos2(rect.left() + 8.0, rect.top() + 32.0),
        egui::Align2::LEFT_CENTER,
        detail,
        egui::FontId::proportional(theme.font_size_small),
        detail_colour,
    );
    response
}
