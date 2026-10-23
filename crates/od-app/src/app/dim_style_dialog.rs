//! The Dimension Style dialog (DIMSTYLE), laid out as AutoCAD's Modify
//! Dimension Style: the tabs on the left, a preview of a sample part and its
//! dimensions on the right. It edits a draft of the drawing's one dimension
//! style; OK commits it as one undo step, Cancel / Escape / X drop it.

use eframe::egui;
use od_ui::widgets;

use super::{dim_preview, dim_style_tabs as tabs, OpenDraftApp};
use crate::dim_style_draft::{sample, Draft};
use crate::dim_style_table::DimEdit;

use tabs::more;

const WIDTH: f32 = 760.0;
const TAB_WIDTH: f32 = 420.0;
const TAB_HEIGHT: f32 = 545.0;
const PREVIEW: egui::Vec2 = egui::vec2(280.0, 210.0);
const BUTTON_WIDTH: f32 = 90.0;

#[derive(Clone, Copy)]
enum Choice {
    Ok,
    Cancel,
}

impl OpenDraftApp {
    /// The window opened by `dim.style`.
    pub(super) fn dim_style_window(&mut self, ctx: &egui::Context) {
        if !self.state.show_dim_style {
            self.dim_style_draft = None;
            return;
        }
        let edit = self.state.dim_edit.clone();
        let mut draft = self.dim_style_draft.take().unwrap_or_else(|| {
            let doc = self.state.session().document();
            Draft::new(&match &edit {
                DimEdit::Current => doc.dim_style().clone(),
                DimEdit::Named(name) => doc
                    .dim_style_named(name)
                    .unwrap_or_else(|| doc.dim_style())
                    .clone(),
                DimEdit::New(style) => (**style).clone(),
            })
        });
        let (mut open, mut choice) = (true, None);
        let theme = &self.theme;
        let document = self.state.session().document();
        let verb = if matches!(edit, DimEdit::New(_)) {
            "New"
        } else {
            "Modify"
        };
        let title = format!("{verb} Dimension Style: {}", draft.style.name);
        let entities = sample(
            &draft.style,
            draft.original.text_height * draft.original.scale,
        );
        widgets::dialog(ctx, theme, &title, &mut open, WIDTH, |ui| {
            widgets::tab_strip(ui, theme, &mut draft.tab, &tabs::TABS);
            ui.add_space(8.0);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(TAB_WIDTH);
                    ui.set_min_height(TAB_HEIGHT);
                    match draft.tab {
                        0 => tabs::lines(ui, theme, &mut draft),
                        1 => tabs::symbols(ui, theme, &mut draft),
                        2 => tabs::text(ui, theme, &mut draft, document),
                        3 => tabs::fit(ui, theme, &mut draft),
                        4 => tabs::primary(ui, theme, &mut draft),
                        5 => more::alternate(ui, theme, &mut draft),
                        _ => more::tolerances(ui, theme, &mut draft),
                    }
                });
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    dim_preview::show(ui, theme, PREVIEW, &entities, document);
                });
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(WIDTH - 2.0 * BUTTON_WIDTH - 24.0);
                if widgets::primary_button(ui, theme, "OK").clicked() {
                    choice = Some(Choice::Ok);
                }
                ui.add_space(6.0);
                if widgets::secondary_button(ui, theme, "Cancel").clicked() {
                    choice = Some(Choice::Cancel);
                }
            });
        });
        if !open {
            choice = Some(Choice::Cancel);
        }
        if std::mem::take(&mut draft.want_text_styles) {
            self.state.show_text_styles = true;
        }
        match choice {
            Some(Choice::Ok) => {
                match edit {
                    DimEdit::Current => {
                        let name = self.state.dim_style().name;
                        self.state.modify_dim_style(&name, draft.style);
                    }
                    DimEdit::Named(name) => self.state.modify_dim_style(&name, draft.style),
                    DimEdit::New(_) => {
                        if let Err(problem) = self.state.add_dim_style(draft.style) {
                            self.state.message = problem;
                        }
                    }
                }
                self.state.show_dim_style = false;
                self.state.dim_edit = DimEdit::Current;
            }
            Some(Choice::Cancel) => {
                self.state.show_dim_style = false;
                self.state.dim_edit = DimEdit::Current;
            }
            None => self.dim_style_draft = Some(draft),
        }
    }
}
