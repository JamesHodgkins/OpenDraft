//! The Block Definition dialog BLOCK opens, laid out as AutoCAD's: the name
//! on top; the base point, the objects and the behaviour side by side; the
//! settings and the description below. The pick buttons and OK hand the draft
//! back to the command, which closes the dialog while it asks in the drawing.

use eframe::egui;
use od_commands::{BlockDialogButton, Reply, Wants};
use od_core::block::block_draft::{BlockDraft, ObjectsAction};
use od_core::InsertUnits;
use od_ui::widgets;

use super::OpenDraftApp;
use crate::text::plural;

const WIDTH: f32 = 690.0;
const COLUMN: f32 = 206.0;

/// A control AutoCAD has that OpenDraft cannot act on yet, shown switched off.
fn unavailable(ui: &mut egui::Ui, checked: bool, label: &str, why: &str) {
    let mut value = checked;
    ui.add_enabled(false, egui::Checkbox::new(&mut value, label))
        .on_disabled_hover_text(why);
}

fn coordinate(ui: &mut egui::Ui, label: &str, value: &mut f64, enabled: bool) {
    ui.add_enabled(enabled, egui::Label::new(label));
    ui.add_enabled(
        enabled,
        egui::DragValue::new(value)
            .speed(0.1)
            .fixed_decimals(4)
            .range(-1.0e9..=1.0e9),
    );
}

impl OpenDraftApp {
    pub(super) fn block_dialog_window(&mut self, ctx: &egui::Context) {
        let wanted = self
            .state
            .waiting()
            .filter(|ask| ask.wants == Wants::BlockDialog)
            .map(|ask| ask.block_dialog.clone());
        let Some(start) = wanted else {
            self.state.command_dialogs.block = None;
            return;
        };
        if self.state.command_dialogs.block.is_none() {
            self.state.command_dialogs.block = start.map(|d| *d);
        }
        let Some(mut draft) = self.state.command_dialogs.block.take() else {
            return;
        };
        let document = self.state.session().document();
        let names: Vec<String> = document.blocks().iter().map(|b| b.name.clone()).collect();
        let in_use = !draft.name.trim().is_empty()
            && names
                .iter()
                .any(|n| n.eq_ignore_ascii_case(draft.name.trim()));
        let problem = draft.problem();
        let (mut open, mut pressed) = (true, None);
        let mut cancel = false;
        let theme = &self.theme;
        widgets::dialog(ctx, theme, "Block Definition", &mut open, WIDTH, |ui| {
            name_row(ui, &mut draft, &names);
            if in_use {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    "A block with this name exists: OK will ask before redefining it.",
                );
            }
            ui.add_space(4.0);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(COLUMN);
                    base_point(ui, theme, &mut draft, &mut pressed);
                });
                ui.vertical(|ui| {
                    ui.set_width(COLUMN + 10.0);
                    objects(ui, theme, &mut draft, &mut pressed);
                });
                ui.vertical(|ui| {
                    ui.set_width(COLUMN - 6.0);
                    behavior(ui, theme);
                });
            });
            ui.add_space(4.0);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(COLUMN);
                    settings(ui, theme, &mut draft);
                });
                ui.vertical(|ui| {
                    ui.set_width(2.0 * COLUMN + 4.0);
                    widgets::group_box(ui, theme, "Description", |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut draft.description)
                                .desired_rows(5)
                                .desired_width(2.0 * COLUMN - 24.0),
                        );
                    });
                });
            });
            ui.add_space(4.0);
            if let Some(why) = problem
                .as_ref()
                .filter(|_| !draft.name.trim().is_empty() && why_is_shown(&draft))
            {
                ui.colored_label(ui.visuals().error_fg_color, why);
            }
            ui.horizontal(|ui| {
                ui.checkbox(&mut draft.open_in_editor, "Open in block editor");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled_ui(false, |ui| {
                        widgets::secondary_button(ui, theme, "Help");
                    });
                    cancel = widgets::secondary_button(ui, theme, "Cancel").clicked();
                    ui.add_enabled_ui(problem.is_none(), |ui| {
                        if widgets::primary_button(ui, theme, "OK").clicked() {
                            pressed = Some(BlockDialogButton::Ok);
                        }
                    });
                });
            });
        });
        match pressed {
            Some(button) => self
                .state
                .reply(Reply::BlockDialog(Box::new(draft), button)),
            None if cancel || !open => self.state.cancel_command(),
            None => self.state.command_dialogs.block = Some(draft),
        }
    }
}

/// Whether the problem is something to tell the user (not just "No objects
/// selected", which the Objects group already says).
fn why_is_shown(draft: &BlockDraft) -> bool {
    draft.objects_on_screen || !draft.objects.is_empty()
}

/// The name, with a list of the blocks there are.
fn name_row(ui: &mut egui::Ui, draft: &mut BlockDraft, names: &[String]) {
    ui.label("Name:");
    ui.horizontal(|ui| {
        // What is left of the row, less the list button and the gap before it.
        let room = (ui.available_width() - 46.0).max(120.0);
        let field = ui.add(egui::TextEdit::singleline(&mut draft.name).desired_width(room));
        if !field.has_focus() && draft.name.is_empty() {
            field.request_focus();
        }
        ui.add_enabled_ui(!names.is_empty(), |ui| {
            egui::ComboBox::from_id_salt("block_names")
                .width(14.0)
                .selected_text("")
                .show_ui(ui, |ui| {
                    for name in names {
                        if ui.selectable_label(false, name).clicked() {
                            draft.name.clone_from(name);
                        }
                    }
                });
        });
    });
}

fn base_point(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    draft: &mut BlockDraft,
    pressed: &mut Option<BlockDialogButton>,
) {
    widgets::group_box(ui, theme, "Base point", |ui| {
        ui.set_width(COLUMN - 24.0);
        ui.checkbox(&mut draft.base_on_screen, "Specify On-screen");
        let typed = !draft.base_on_screen;
        if ui
            .add_enabled(typed, egui::Button::new("Pick point"))
            .clicked()
        {
            *pressed = Some(BlockDialogButton::PickPoint);
        }
        egui::Grid::new("block_base").num_columns(2).show(ui, |ui| {
            coordinate(ui, "X:", &mut draft.x, typed);
            ui.end_row();
            coordinate(ui, "Y:", &mut draft.y, typed);
            ui.end_row();
            // The drawing is flat.
            coordinate(ui, "Z:", &mut 0.0, false);
            ui.end_row();
        });
    });
}

fn objects(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    draft: &mut BlockDraft,
    pressed: &mut Option<BlockDialogButton>,
) {
    widgets::group_box(ui, theme, "Objects", |ui| {
        ui.set_width(COLUMN - 14.0);
        ui.checkbox(&mut draft.objects_on_screen, "Specify On-screen");
        ui.horizontal(|ui| {
            let select = ui.add_enabled(
                !draft.objects_on_screen,
                egui::Button::new("Select objects"),
            );
            if select.clicked() {
                *pressed = Some(BlockDialogButton::SelectObjects);
            }
            ui.add_enabled(false, egui::Button::new("\u{26A1}"))
                .on_disabled_hover_text("Quick select is not offered here yet; use POWERSELECT.");
        });
        ui.radio_value(&mut draft.action, ObjectsAction::Retain, "Retain");
        ui.radio_value(
            &mut draft.action,
            ObjectsAction::Convert,
            "Convert to block",
        );
        ui.radio_value(&mut draft.action, ObjectsAction::Delete, "Delete");
        if draft.objects_on_screen {
            ui.label("Objects are chosen after OK");
        } else if draft.objects.is_empty() {
            ui.horizontal(|ui| {
                ui.colored_label(ui.visuals().warn_fg_color, "\u{26A0}");
                ui.label("No objects selected");
            });
        } else {
            ui.label(format!(
                "{} selected",
                plural(draft.objects.len(), "object")
            ));
        }
    });
}

fn behavior(ui: &mut egui::Ui, theme: &od_ui::Theme) {
    widgets::group_box(ui, theme, "Behavior", |ui| {
        ui.set_width(COLUMN - 28.0);
        unavailable(
            ui,
            false,
            "Annotative",
            "Annotative blocks are not built yet.",
        );
        ui.indent("block_orientation", |ui| {
            unavailable(
                ui,
                false,
                "Match block orientation to layout",
                "Annotative blocks are not built yet.",
            );
        });
        unavailable(
            ui,
            false,
            "Scale uniformly",
            "OpenDraft does not keep this setting on a block yet.",
        );
        unavailable(
            ui,
            true,
            "Allow exploding",
            "Every block can be exploded; the setting is not kept yet.",
        );
    });
}

fn settings(ui: &mut egui::Ui, theme: &od_ui::Theme, draft: &mut BlockDraft) {
    widgets::group_box(ui, theme, "Settings", |ui| {
        ui.set_width(COLUMN - 24.0);
        ui.label("Block unit:");
        egui::ComboBox::from_id_salt("block_unit")
            .width(COLUMN - 40.0)
            .selected_text(draft.unit.label())
            .show_ui(ui, |ui| {
                for unit in InsertUnits::all() {
                    ui.selectable_value(&mut draft.unit, unit, unit.label());
                }
            });
        ui.add_enabled(false, egui::Button::new("Hyperlink..."))
            .on_disabled_hover_text("Hyperlinks are not built yet.");
    });
}
