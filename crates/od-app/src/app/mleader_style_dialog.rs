//! The Modify Multileader Style dialog, laid out as AutoCAD's: three tabs on
//! the left, a preview of a sample multileader on the right. It edits a
//! draft of one style; OK commits it as one undo step, Cancel / Escape / X
//! drop it.

use eframe::egui;
use od_core::mleader::{self, MLeaderStyle};
use od_ui::widgets;

use super::{dim_preview, mleader_style_tabs as tabs, OpenDraftApp};
use crate::mleader_style_table::MLeaderEdit;

const WIDTH: f32 = 740.0;
const TAB_WIDTH: f32 = 400.0;
const TAB_HEIGHT: f32 = 500.0;
const PREVIEW: egui::Vec2 = egui::vec2(290.0, 330.0);
const BUTTON_WIDTH: f32 = 90.0;

/// The working copy while the dialog is open.
pub(in crate::app) struct Draft {
    pub style: MLeaderStyle,
    pub tab: usize,
    /// The "..." beside the text style was clicked.
    pub want_text_styles: bool,
}

#[derive(Clone, Copy)]
enum Choice {
    Ok,
    Cancel,
}

impl OpenDraftApp {
    /// The window opened from the Multileader Style Manager.
    pub(super) fn mleader_style_window(&mut self, ctx: &egui::Context) {
        if !self.state.mleader_dialogs.style {
            self.mleader_windows.draft = None;
            return;
        }
        let edit = self.state.mleader_dialogs.edit.clone();
        let mut draft = self.mleader_windows.draft.take().unwrap_or_else(|| {
            let doc = self.state.session().document();
            let style = match &edit {
                MLeaderEdit::Current => self.state.mleader_style(),
                MLeaderEdit::Named(id) => doc
                    .mleader_style(*id)
                    .cloned()
                    .unwrap_or_else(|| self.state.mleader_style()),
                MLeaderEdit::New(style) => (**style).clone(),
            };
            Draft {
                style,
                tab: 0,
                want_text_styles: false,
            }
        });
        let (mut open, mut choice) = (true, None);
        let theme = &self.theme;
        let document = self.state.session().document();
        let verb = if matches!(edit, MLeaderEdit::New(_)) {
            "Create"
        } else {
            "Modify"
        };
        let title = format!("{verb} Multileader Style: {}", draft.style.name);
        let entities = mleader::sample(&draft.style, tabs::preview_text(&draft.style));
        widgets::dialog(ctx, theme, &title, &mut open, WIDTH, |ui| {
            widgets::tab_strip(ui, theme, &mut draft.tab, &tabs::TABS);
            ui.add_space(8.0);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(TAB_WIDTH);
                    ui.set_min_height(TAB_HEIGHT);
                    match draft.tab {
                        0 => tabs::leader_format(ui, theme, &mut draft, document),
                        1 => tabs::leader_structure(ui, theme, &mut draft),
                        _ => tabs::content(ui, theme, &mut draft, document),
                    }
                });
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    dim_preview::show_plain(ui, theme, PREVIEW, &entities, document);
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
                    MLeaderEdit::Current => {
                        let id = self.state.mleader_style().id;
                        self.state.modify_mleader_style(id, draft.style);
                    }
                    MLeaderEdit::Named(id) => self.state.modify_mleader_style(id, draft.style),
                    MLeaderEdit::New(_) => {
                        if let Err(problem) = self.state.add_mleader_style(draft.style) {
                            self.state.message = problem;
                        }
                    }
                }
                self.close_mleader_style();
            }
            Some(Choice::Cancel) => self.close_mleader_style(),
            None => self.mleader_windows.draft = Some(draft),
        }
    }

    fn close_mleader_style(&mut self) {
        self.state.mleader_dialogs.style = false;
        self.state.mleader_dialogs.edit = MLeaderEdit::Current;
    }
}
