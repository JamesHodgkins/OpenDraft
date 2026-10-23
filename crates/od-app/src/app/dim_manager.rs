//! The Dimension Style Manager (DIMSTYLE): the drawing's named dimension
//! styles with a preview of the selected one, and Set Current, New, Modify,
//! Rename and Delete. Each button is one undo step on its own; Modify and New
//! open the Modify Dimension Style dialog.

use eframe::egui::{self, Sense};
use od_core::DimStyle;
use od_ui::widgets;

use super::{dim_preview, OpenDraftApp};
use crate::dim_style_draft::sample;
use crate::dim_style_table::{name_problem, suggested_name, DimEdit};

const WIDTH: f32 = 700.0;
const LIST_WIDTH: f32 = 220.0;
const LIST_HEIGHT: f32 = 300.0;
const ROW_HEIGHT: f32 = 22.0;
const PREVIEW: egui::Vec2 = egui::vec2(280.0, 210.0);
const BUTTON_WIDTH: f32 = 110.0;

/// What the manager remembers while it is open.
pub(in crate::app) struct Ui {
    selected: String,
    in_use_only: bool,
    prompt: Option<Prompt>,
    /// Why the last Delete or Rename did not happen.
    problem: Option<String>,
}

enum Prompt {
    New { name: String, start: String },
    Rename { from: String, name: String },
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
    /// The window opened by `dim.style`.
    pub(super) fn dim_manager_window(&mut self, ctx: &egui::Context) {
        if !self.state.show_dim_manager {
            self.dim_manager = None;
            return;
        }
        let mut ui_state = self.dim_manager.take().unwrap_or_else(|| Ui {
            selected: self.state.dim_style().name,
            in_use_only: false,
            prompt: None,
            problem: None,
        });
        let styles = self.state.dim_styles();
        let current = self.state.dim_style().name;
        if !styles.iter().any(|s| s.name == ui_state.selected) {
            ui_state.selected = current.clone();
        }
        let (mut open, mut choice) = (true, None);
        let covered = ui_state.prompt.is_some() || self.state.show_dim_style;
        let can_delete = self.state.can_delete_dim_style(&ui_state.selected);
        let doc = self.state.session().document();
        let in_use: Vec<String> = styles
            .iter()
            .filter(|s| doc.dim_style_in_use(&s.name) || s.name == current)
            .map(|s| s.name.clone())
            .collect();
        let shown = styles
            .iter()
            .find(|s| s.name == ui_state.selected)
            .cloned()
            .unwrap_or_default();
        let entities = sample(&shown, shown.text_height * shown.scale);
        let theme = &self.theme;
        widgets::dialog(
            ctx,
            theme,
            "Dimension Style Manager",
            &mut open,
            WIDTH,
            |ui| {
                ui.add_enabled_ui(!covered, |ui| {
                    widgets::field_label(
                        ui,
                        theme,
                        &format!("Current dimension style:  {current}"),
                    );
                    ui.add_space(6.0);
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(LIST_WIDTH);
                            widgets::field_label(ui, theme, "Styles:");
                            ui.add_space(2.0);
                            list(ui, theme, &styles, &mut ui_state, &in_use);
                            ui.add_space(6.0);
                            let label = if ui_state.in_use_only {
                                "Styles in use"
                            } else {
                                "All styles"
                            };
                            widgets::dropdown(
                                ui,
                                theme,
                                "dim_manager_filter",
                                label,
                                LIST_WIDTH,
                                |ui| {
                                    for (text, only) in
                                        [("All styles", false), ("Styles in use", true)]
                                    {
                                        if ui
                                            .selectable_label(ui_state.in_use_only == only, text)
                                            .clicked()
                                        {
                                            ui_state.in_use_only = only;
                                        }
                                    }
                                },
                            );
                        });
                        ui.add_space(14.0);
                        ui.vertical(|ui| {
                            widgets::field_label(
                                ui,
                                theme,
                                &format!("Preview of:  {}", shown.name),
                            );
                            ui.add_space(2.0);
                            dim_preview::show(ui, theme, PREVIEW, &entities, doc);
                            if let Some(problem) = &ui_state.problem {
                                ui.add_space(6.0);
                                widgets::field_label(ui, theme, problem);
                            }
                        });
                        ui.add_space(14.0);
                        ui.vertical(|ui| {
                            ui.set_width(BUTTON_WIDTH);
                            let is_current = ui_state.selected == current;
                            let standard = ui_state.selected.eq_ignore_ascii_case("Standard");
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
                .dim_prompt_window(ctx, &mut ui_state, &styles)
                .or(choice);
        } else if !open && !covered {
            choice = Some(Choice::Close);
        }
        self.apply_manager_choice(choice, ui_state, styles);
    }

    fn apply_manager_choice(&mut self, choice: Option<Choice>, mut ui: Ui, styles: Vec<DimStyle>) {
        ui.problem = None;
        match choice {
            Some(Choice::Close) => {
                self.state.show_dim_manager = false;
                return;
            }
            Some(Choice::SetCurrent) => self.state.set_current_dim_style(&ui.selected),
            Some(Choice::New) => {
                ui.prompt = Some(Prompt::New {
                    name: suggested_name(&styles),
                    start: self.state.dim_style().name,
                });
            }
            Some(Choice::Modify) => {
                self.state.dim_edit = DimEdit::Named(ui.selected.clone());
                self.state.show_dim_style = true;
            }
            Some(Choice::Rename) => {
                ui.prompt = Some(Prompt::Rename {
                    from: ui.selected.clone(),
                    name: ui.selected.clone(),
                });
            }
            Some(Choice::Delete) => {
                ui.problem = self.state.delete_dim_style(&ui.selected).err();
                ui.selected = self.state.dim_style().name;
            }
            Some(Choice::PromptOk) => match ui.prompt.take() {
                Some(Prompt::New { name, start }) => {
                    let base = styles
                        .iter()
                        .find(|s| s.name == start)
                        .cloned()
                        .unwrap_or_default();
                    let name = name.trim().to_owned();
                    ui.selected = name.clone();
                    self.state.dim_edit = DimEdit::New(Box::new(DimStyle { name, ..base }));
                    self.state.show_dim_style = true;
                }
                Some(Prompt::Rename { from, name }) => {
                    match self.state.rename_dim_style(&from, &name) {
                        Ok(()) => ui.selected = name.trim().to_owned(),
                        Err(problem) => ui.problem = Some(problem),
                    }
                }
                None => {}
            },
            Some(Choice::PromptCancel) => ui.prompt = None,
            None => {}
        }
        self.dim_manager = Some(ui);
    }

    /// New (a name and what to start from) or Rename (a name).
    fn dim_prompt_window(
        &mut self,
        ctx: &egui::Context,
        ui_state: &mut Ui,
        styles: &[DimStyle],
    ) -> Option<Choice> {
        let (mut open, mut choice) = (true, None);
        let theme = &self.theme;
        let title = match ui_state.prompt {
            Some(Prompt::Rename { .. }) => "Rename Dimension Style",
            _ => "Create New Dimension Style",
        };
        widgets::dialog(ctx, theme, title, &mut open, 320.0, |ui| {
            let (label, name, ignoring) = match ui_state.prompt.as_mut()? {
                Prompt::New { name, .. } => ("New style name:", name, None),
                Prompt::Rename { from, name } => ("New name:", name, Some(from.clone())),
            };
            widgets::field_label(ui, theme, label);
            ui.add_space(2.0);
            let typed = widgets::text_field(ui, theme, name, "");
            let problem = name_problem(styles, name, ignoring.as_deref());
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
                widgets::dropdown(
                    ui,
                    theme,
                    "dim_new_start",
                    start.clone().as_str(),
                    280.0,
                    |ui| {
                        for style in styles {
                            ui.selectable_value(start, style.name.clone(), &style.name);
                        }
                    },
                );
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

fn list(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    styles: &[DimStyle],
    state: &mut Ui,
    in_use: &[String],
) {
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0_f32, theme.colors.border))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(LIST_HEIGHT)
                .min_scrolled_height(LIST_HEIGHT)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for style in styles
                        .iter()
                        .filter(|s| !state.in_use_only || in_use.contains(&s.name))
                    {
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), ROW_HEIGHT),
                            Sense::click(),
                        );
                        if response.clicked() {
                            state.selected = style.name.clone();
                        }
                        if state.selected == style.name {
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
