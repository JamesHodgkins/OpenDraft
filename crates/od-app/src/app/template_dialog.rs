//! The "New from template" picker: the starters built into the program, then
//! the user's own templates. The folder is read on a worker thread when the
//! picker opens, so a slow or network folder cannot freeze the window.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

use eframe::egui;
use od_ui::widgets;

use super::OpenDraftApp;
use crate::templates::{scan, starters, Source, Template};

const WIDTH: f32 = 480.0;
const ROW_HEIGHT: f32 = 46.0;

#[derive(Default)]
pub struct TemplateDialog {
    open: bool,
    /// The user's templates once the folder has been read.
    user: Option<Vec<Template>>,
    reading: Option<Receiver<Vec<Template>>>,
}

impl TemplateDialog {
    /// Show the picker and start reading `folder` (none: there are no user
    /// templates to read).
    pub fn show(&mut self, ctx: &egui::Context, folder: Option<PathBuf>) {
        self.open = true;
        self.user = None;
        match folder {
            Some(folder) => {
                let (tx, rx) = channel();
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    // The picker may have been closed; then no one wants it.
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

    fn collect(&mut self) {
        if let Some(rx) = &self.reading {
            if let Ok(found) = rx.try_recv() {
                self.user = Some(found);
                self.reading = None;
            }
        }
    }
}

impl OpenDraftApp {
    pub(super) fn template_dialog(&mut self, ctx: &egui::Context) {
        if !self.templates.open {
            return;
        }
        self.templates.collect();
        let theme = &self.theme;
        let folder = self.state.templates_dir.clone();
        let mut open = true;
        let mut chosen: Option<Template> = None;
        let mut reveal = false;
        widgets::dialog(ctx, theme, "New from template", &mut open, WIDTH, |ui| {
            ui.set_width(WIDTH);
            egui::Frame::new()
                .inner_margin(egui::Margin::same(16))
                .show(ui, |ui| {
                    ui.set_width(WIDTH - 32.0);
                    widgets::section_header(ui, theme, "Starters");
                    for template in starters() {
                        if template_row(ui, theme, &template).clicked() {
                            chosen = Some(template);
                        }
                    }
                    ui.add_space(14.0);
                    widgets::section_header(ui, theme, "Your templates");
                    match &self.templates.user {
                        None => {
                            ui.label(
                                egui::RichText::new("Looking...").color(theme.colors.text_weak),
                            );
                        }
                        Some(found) if found.is_empty() => {
                            ui.label(
                                egui::RichText::new(
                                    "None yet. Use Save as Template in the application menu \
                                     to keep a drawing here.",
                                )
                                .color(theme.colors.text_weak),
                            );
                        }
                        Some(found) => {
                            for template in found {
                                if template_row(ui, theme, template).clicked() {
                                    chosen = Some(template.clone());
                                }
                            }
                        }
                    }
                    if let Some(folder) = &folder {
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(folder.display().to_string())
                                .color(theme.colors.text_weak),
                        );
                        ui.add_space(6.0);
                        reveal = widgets::secondary_button(ui, theme, "Open folder").clicked();
                    }
                });
        });
        if reveal {
            if let Some(folder) = &folder {
                let _ = std::fs::create_dir_all(folder);
                ctx.open_url(egui::OpenUrl::same_tab(folder.display().to_string()));
            }
        }
        if let Some(template) = chosen {
            self.state.new_from_template(&template);
            open = false;
        }
        if !open {
            self.templates.open = false;
        }
    }
}

/// A template as a clickable line: its name over what it sets up, or why it
/// is unavailable (which cannot be clicked).
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
        egui::pos2(rect.left() + 12.0, rect.top() + 14.0),
        egui::Align2::LEFT_CENTER,
        &template.name,
        egui::FontId::proportional(theme.font_size_body),
        name_colour,
    );
    painter.text(
        egui::pos2(rect.left() + 12.0, rect.top() + 32.0),
        egui::Align2::LEFT_CENTER,
        detail,
        egui::FontId::proportional(theme.font_size_small),
        detail_colour,
    );
    response
}
