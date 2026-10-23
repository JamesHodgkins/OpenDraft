//! The Multileader Style Manager (MLEADERSTYLE): the drawing's named
//! multileader styles with a preview of the selected one, and Set Current,
//! New, Modify, Rename and Delete. Each button is one undo step on its own;
//! Modify and New open the Modify Multileader Style dialog.

use eframe::egui::{self, Sense};
use od_core::mleader::{self, MLeaderStyle};
use od_core::MLeaderStyleId;
use od_ui::widgets;

use super::{dim_preview, mleader_style_tabs::preview_text, OpenDraftApp};
use crate::mleader_style_table::{name_problem, suggested_name, MLeaderEdit};

const WIDTH: f32 = 700.0;
const LIST_WIDTH: f32 = 220.0;
const LIST_HEIGHT: f32 = 300.0;
const ROW_HEIGHT: f32 = 22.0;
const PREVIEW: egui::Vec2 = egui::vec2(280.0, 250.0);
const BUTTON_WIDTH: f32 = 110.0;

/// The open multileader-style windows' own state.
#[derive(Default)]
pub(in crate::app) struct Windows {
    pub(super) manager: Option<Ui>,
    pub(super) draft: Option<super::mleader_style_dialog::Draft>,
}

/// What the manager remembers while it is open.
pub(in crate::app) struct Ui {
    selected: MLeaderStyleId,
    prompt: Option<Prompt>,
    /// Why the last Delete or Rename did not happen.
    problem: Option<String>,
}

enum Prompt {
    New { name: String, start: MLeaderStyleId },
    Rename { id: MLeaderStyleId, name: String },
}

#[derive(Clone)]
enum Choice {
    Close,
    SetCurrent,
    New,
    Modify,
    Rename,
    Delete,
    PromptOk,
    PromptCancel,
}

impl OpenDraftApp {
    /// The manager and the Modify dialog it opens.
    pub(super) fn mleader_windows(&mut self, ctx: &egui::Context) {
        self.mleader_manager_window(ctx);
        self.mleader_style_window(ctx);
    }

    /// The window opened by `annotate.mleaderstyle`.
    pub(super) fn mleader_manager_window(&mut self, ctx: &egui::Context) {
        if !self.state.mleader_dialogs.manager {
            self.mleader_windows.manager = None;
            return;
        }
        let current = self.state.mleader_style();
        let mut ui_state = self.mleader_windows.manager.take().unwrap_or(Ui {
            selected: current.id,
            prompt: None,
            problem: None,
        });
        let styles = self.state.mleader_styles();
        if !styles.iter().any(|s| s.id == ui_state.selected) {
            ui_state.selected = current.id;
        }
        let (mut open, mut choice) = (true, None);
        let covered = ui_state.prompt.is_some() || self.state.mleader_dialogs.style;
        let can_delete = self.state.can_delete_mleader_style(ui_state.selected);
        let shown = styles
            .iter()
            .find(|s| s.id == ui_state.selected)
            .cloned()
            .unwrap_or_else(MLeaderStyle::standard);
        let entities = mleader::sample(&shown, preview_text(&shown));
        let doc = self.state.session().document();
        let theme = &self.theme;
        widgets::dialog(
            ctx,
            theme,
            "Multileader Style Manager",
            &mut open,
            WIDTH,
            |ui| {
                ui.add_enabled_ui(!covered, |ui| {
                    widgets::field_label(
                        ui,
                        theme,
                        &format!("Current multileader style:  {}", current.name),
                    );
                    ui.add_space(6.0);
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(LIST_WIDTH);
                            widgets::field_label(ui, theme, "Styles:");
                            ui.add_space(2.0);
                            list(ui, theme, &styles, &mut ui_state);
                        });
                        ui.add_space(14.0);
                        ui.vertical(|ui| {
                            widgets::field_label(
                                ui,
                                theme,
                                &format!("Preview of:  {}", shown.name),
                            );
                            ui.add_space(2.0);
                            dim_preview::show_plain(ui, theme, PREVIEW, &entities, doc);
                            if let Some(problem) = &ui_state.problem {
                                ui.add_space(6.0);
                                widgets::field_label(ui, theme, problem);
                            }
                        });
                        ui.add_space(14.0);
                        ui.vertical(|ui| {
                            ui.set_width(BUTTON_WIDTH);
                            let is_current = ui_state.selected == current.id;
                            let standard = ui_state.selected == MLeaderStyleId::STANDARD;
                            let mut button =
                                |ui: &mut egui::Ui, on: bool, label: &str, what: Choice| {
                                    let clicked = ui
                                        .add_enabled_ui(on, |ui| {
                                            widgets::secondary_button(ui, theme, label)
                                        })
                                        .inner
                                        .clicked();
                                    ui.add_space(4.0);
                                    if clicked {
                                        choice = Some(what);
                                    }
                                };
                            button(ui, !is_current, "Set Current", Choice::SetCurrent);
                            button(ui, true, "New...", Choice::New);
                            button(ui, true, "Modify...", Choice::Modify);
                            button(ui, !standard, "Rename...", Choice::Rename);
                            button(ui, can_delete, "Delete", Choice::Delete);
                        });
                    });
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(WIDTH - BUTTON_WIDTH - 40.0);
                        if widgets::primary_button(ui, theme, "Close").clicked() {
                            choice = Some(Choice::Close);
                        }
                    });
                });
            },
        );
        if ui_state.prompt.is_some() {
            choice = self
                .mleader_prompt_window(ctx, &mut ui_state, &styles)
                .or(choice);
        } else if !open && !covered {
            choice = Some(Choice::Close);
        }
        self.apply_mleader_manager_choice(choice, ui_state, styles);
    }

    fn apply_mleader_manager_choice(
        &mut self,
        choice: Option<Choice>,
        mut ui: Ui,
        styles: Vec<MLeaderStyle>,
    ) {
        ui.problem = None;
        match choice {
            Some(Choice::Close) => {
                self.state.mleader_dialogs.manager = false;
                return;
            }
            Some(Choice::SetCurrent) => self.state.set_current_mleader_style(ui.selected),
            Some(Choice::New) => {
                ui.prompt = Some(Prompt::New {
                    name: suggested_name(&styles),
                    start: self.state.mleader_style().id,
                });
            }
            Some(Choice::Modify) => {
                self.state.mleader_dialogs.edit = MLeaderEdit::Named(ui.selected);
                self.state.mleader_dialogs.style = true;
            }
            Some(Choice::Rename) => {
                let name = styles
                    .iter()
                    .find(|s| s.id == ui.selected)
                    .map(|s| s.name.clone())
                    .unwrap_or_default();
                ui.prompt = Some(Prompt::Rename {
                    id: ui.selected,
                    name,
                });
            }
            Some(Choice::Delete) => {
                ui.problem = self.state.delete_mleader_style(ui.selected).err();
                ui.selected = self.state.mleader_style().id;
            }
            Some(Choice::PromptOk) => match ui.prompt.take() {
                Some(Prompt::New { name, start }) => {
                    let base = styles
                        .iter()
                        .find(|s| s.id == start)
                        .cloned()
                        .unwrap_or_else(MLeaderStyle::standard);
                    let name = name.trim().to_owned();
                    self.state.mleader_dialogs.edit =
                        MLeaderEdit::New(Box::new(MLeaderStyle { name, ..base }));
                    self.state.mleader_dialogs.style = true;
                }
                Some(Prompt::Rename { id, name }) => {
                    if let Err(problem) = self.state.rename_mleader_style(id, &name) {
                        ui.problem = Some(problem);
                    }
                }
                None => {}
            },
            Some(Choice::PromptCancel) => ui.prompt = None,
            None => {}
        }
        self.mleader_windows.manager = Some(ui);
    }

    /// New (a name and what to start from) or Rename (a name).
    fn mleader_prompt_window(
        &mut self,
        ctx: &egui::Context,
        ui_state: &mut Ui,
        styles: &[MLeaderStyle],
    ) -> Option<Choice> {
        let (mut open, mut choice) = (true, None);
        let theme = &self.theme;
        let title = match ui_state.prompt {
            Some(Prompt::Rename { .. }) => "Rename Multileader Style",
            _ => "Create New Multileader Style",
        };
        widgets::dialog(ctx, theme, title, &mut open, 320.0, |ui| {
            let (label, name, ignoring) = match ui_state.prompt.as_mut()? {
                Prompt::New { name, .. } => ("New style name:", name, None),
                Prompt::Rename { id, name } => ("New name:", name, Some(*id)),
            };
            widgets::field_label(ui, theme, label);
            ui.add_space(2.0);
            let typed = widgets::text_field(ui, theme, name, "");
            let problem = name_problem(styles, name, ignoring);
            if let Some(problem) = &problem {
                ui.add_space(4.0);
                widgets::field_label(ui, theme, problem);
            }
            if typed.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                choice = problem.is_none().then_some(Choice::PromptOk);
            }
            if let Some(Prompt::New { start, .. }) = ui_state.prompt.as_mut() {
                ui.add_space(8.0);
                widgets::field_label(ui, theme, "Start with:");
                ui.add_space(2.0);
                let shown = styles
                    .iter()
                    .find(|s| s.id == *start)
                    .map_or("Standard", |s| s.name.as_str());
                widgets::dropdown(ui, theme, "mleader_new_start", shown, 280.0, |ui| {
                    for style in styles {
                        ui.selectable_value(start, style.id, &style.name);
                    }
                });
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let ok = ui.add_enabled_ui(problem.is_none(), |ui| {
                    let text = if ignoring.is_some() { "OK" } else { "Continue" };
                    widgets::primary_button(ui, theme, text)
                });
                if ok.inner.clicked() {
                    choice = Some(Choice::PromptOk);
                }
                if widgets::secondary_button(ui, theme, "Cancel").clicked() {
                    choice = Some(Choice::PromptCancel);
                }
            });
            Some(())
        });
        if !open {
            choice = Some(Choice::PromptCancel);
        }
        choice
    }
}

fn list(ui: &mut egui::Ui, theme: &od_ui::Theme, styles: &[MLeaderStyle], state: &mut Ui) {
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0_f32, theme.colors.border))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(LIST_HEIGHT)
                .min_scrolled_height(LIST_HEIGHT)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for style in styles {
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), ROW_HEIGHT),
                            Sense::click(),
                        );
                        if response.clicked() {
                            state.selected = style.id;
                        }
                        if state.selected == style.id {
                            ui.painter().rect_filled(rect, 2.0, theme.colors.accent);
                        } else if response.hovered() {
                            ui.painter().rect_filled(rect, 2.0, theme.colors.hover);
                        }
                        ui.painter().text(
                            egui::pos2(rect.left() + 8.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &style.name,
                            egui::FontId::proportional(theme.font_size_body),
                            theme.colors.text,
                        );
                    }
                });
        });
}
