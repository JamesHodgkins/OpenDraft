//! The Attribute Definition dialog ATTDEF opens, laid out as AutoCAD's: the
//! modes and the insertion point on the left, the attribute and its text
//! settings on the right. OK hands the draft back to the command, which asks
//! for the point; Cancel ends the command.

use eframe::egui;
use od_commands::{Reply, Wants};
use od_core::block::attribute_draft::{DefinitionDraft, Justification};
use od_core::TextStyleId;
use od_ui::widgets;

use super::OpenDraftApp;

const WIDTH: f32 = 560.0;
const LEFT: f32 = 215.0;
const RIGHT: f32 = 310.0;
const FIELD: f32 = 150.0;

/// A control AutoCAD has that OpenDraft cannot act on yet, shown switched off.
fn unavailable(ui: &mut egui::Ui, label: &str, why: &str) {
    let mut off = false;
    ui.add_enabled(false, egui::Checkbox::new(&mut off, label))
        .on_disabled_hover_text(why);
}

fn number(ui: &mut egui::Ui, value: &mut f64, enabled: bool) {
    ui.add_enabled(
        enabled,
        egui::DragValue::new(value)
            .speed(0.1)
            .fixed_decimals(4)
            .range(-1.0e9..=1.0e9),
    );
}

impl OpenDraftApp {
    pub(super) fn attribute_definition_window(&mut self, ctx: &egui::Context) {
        let wanted = self
            .state
            .waiting()
            .filter(|ask| ask.wants == Wants::AttributeDefinitionDialog)
            .map(|ask| ask.attribute_definition.clone());
        let Some(start) = wanted else {
            self.state.command_dialogs.definition = None;
            return;
        };
        if self.state.command_dialogs.definition.is_none() {
            self.state.command_dialogs.definition = start.map(|d| *d);
        }
        let Some(mut draft) = self.state.command_dialogs.definition.take() else {
            return;
        };
        let styles: Vec<(TextStyleId, String)> = self
            .state
            .session()
            .document()
            .text_styles()
            .iter()
            .map(|s| (s.id, s.name.clone()))
            .collect();
        let problem = draft.problem();
        let (mut open, mut ok, mut cancel) = (true, false, false);
        let theme = &self.theme;
        widgets::dialog(ctx, theme, "Attribute Definition", &mut open, WIDTH, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(LEFT);
                    modes(ui, theme, &mut draft);
                    ui.add_space(6.0);
                    insertion(ui, theme, &mut draft);
                });
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.set_width(RIGHT);
                    attribute(ui, theme, &mut draft);
                    ui.add_space(6.0);
                    text_settings(ui, theme, &mut draft, &styles);
                });
            });
            ui.add_space(4.0);
            unavailable(
                ui,
                "Align below previous attribute definition",
                "OpenDraft does not have this yet.",
            );
            ui.add_space(6.0);
            // Nothing is said about a tag before one is typed.
            if let Some(why) = problem.as_ref().filter(|_| !draft.tag.trim().is_empty()) {
                ui.colored_label(ui.visuals().error_fg_color, why);
            }
            ui.horizontal(|ui| {
                ui.add_enabled_ui(problem.is_none(), |ui| {
                    ok = widgets::primary_button(ui, theme, "OK").clicked();
                });
                cancel = widgets::secondary_button(ui, theme, "Cancel").clicked();
                ui.add_enabled_ui(false, |ui| {
                    widgets::secondary_button(ui, theme, "Help");
                });
            });
        });
        open &= !cancel;
        if ok {
            self.state
                .reply(Reply::AttributeDefinition(Box::new(draft)));
        } else if !open {
            self.state.cancel_command();
        } else {
            self.state.command_dialogs.definition = Some(draft);
        }
    }
}

fn modes(ui: &mut egui::Ui, theme: &od_ui::Theme, draft: &mut DefinitionDraft) {
    widgets::group_box(ui, theme, "Mode", |ui| {
        ui.set_width(LEFT - 24.0);
        ui.checkbox(&mut draft.invisible, "Invisible");
        ui.checkbox(&mut draft.constant, "Constant");
        // A constant attribute is never asked for, so it is not verified or preset.
        ui.add_enabled(
            !draft.constant,
            egui::Checkbox::new(&mut draft.verify, "Verify"),
        );
        ui.add_enabled(
            !draft.constant,
            egui::Checkbox::new(&mut draft.preset, "Preset"),
        );
        ui.checkbox(&mut draft.lock_position, "Lock position");
        unavailable(
            ui,
            "Multiple lines",
            "Multiline attributes are not built yet.",
        );
    });
}

fn insertion(ui: &mut egui::Ui, theme: &od_ui::Theme, draft: &mut DefinitionDraft) {
    widgets::group_box(ui, theme, "Insertion Point", |ui| {
        ui.set_width(LEFT - 24.0);
        ui.checkbox(&mut draft.on_screen, "Specify on-screen");
        let typed = !draft.on_screen;
        egui::Grid::new("attdef_point")
            .num_columns(2)
            .show(ui, |ui| {
                ui.add_enabled(typed, egui::Label::new("X:"));
                number(ui, &mut draft.x, typed);
                ui.end_row();
                ui.add_enabled(typed, egui::Label::new("Y:"));
                number(ui, &mut draft.y, typed);
                ui.end_row();
                // The drawing is flat.
                ui.add_enabled(false, egui::Label::new("Z:"));
                let mut z = 0.0;
                number(ui, &mut z, false);
                ui.end_row();
            });
    });
}

fn attribute(ui: &mut egui::Ui, theme: &od_ui::Theme, draft: &mut DefinitionDraft) {
    widgets::group_box(ui, theme, "Attribute", |ui| {
        ui.set_width(RIGHT - 24.0);
        egui::Grid::new("attdef_attribute")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("Tag:");
                let tag =
                    ui.add(egui::TextEdit::singleline(&mut draft.tag).desired_width(FIELD + 60.0));
                if !tag.has_focus() && draft.tag.is_empty() {
                    tag.request_focus();
                }
                ui.end_row();
                ui.add_enabled(!draft.constant, egui::Label::new("Prompt:"));
                ui.add_enabled(
                    !draft.constant,
                    egui::TextEdit::singleline(&mut draft.prompt).desired_width(FIELD + 60.0),
                );
                ui.end_row();
                ui.label("Default:");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut draft.default_value)
                            .desired_width(FIELD + 28.0),
                    );
                    ui.add_enabled(false, egui::Button::new("\u{2630}"))
                        .on_disabled_hover_text("Fields are not evaluated yet.");
                });
                ui.end_row();
            });
    });
}

fn text_settings(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    draft: &mut DefinitionDraft,
    styles: &[(TextStyleId, String)],
) {
    widgets::group_box(ui, theme, "Text Settings", |ui| {
        ui.set_width(RIGHT - 24.0);
        egui::Grid::new("attdef_text")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("Justification:");
                egui::ComboBox::from_id_salt("attdef_justification")
                    .width(FIELD)
                    .selected_text(draft.justification.label())
                    .show_ui(ui, |ui| {
                        for j in Justification::ALL {
                            ui.selectable_value(&mut draft.justification, j, j.label());
                        }
                    });
                ui.end_row();
                ui.label("Text style:");
                let shown = styles
                    .iter()
                    .find(|(id, _)| *id == draft.style)
                    .map_or("?", |(_, name)| name.as_str());
                egui::ComboBox::from_id_salt("attdef_style")
                    .width(FIELD)
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        for (id, name) in styles {
                            ui.selectable_value(&mut draft.style, *id, name);
                        }
                    });
                ui.end_row();
                ui.label("");
                unavailable(ui, "Annotative", "Annotative text is not built yet.");
                ui.end_row();
                ui.label("Text height:");
                ui.horizontal(|ui| {
                    number(ui, &mut draft.height, true);
                    pick_button(ui);
                });
                ui.end_row();
                ui.label("Rotation:");
                ui.horizontal(|ui| {
                    number(ui, &mut draft.rotation, true);
                    pick_button(ui);
                });
                ui.end_row();
                ui.add_enabled(false, egui::Label::new("Boundary width:"));
                let mut width = 0.0;
                ui.horizontal(|ui| {
                    number(ui, &mut width, false);
                    pick_button(ui);
                });
                ui.end_row();
            });
    });
}

/// The button that picks a size or angle on the screen.
fn pick_button(ui: &mut egui::Ui) {
    ui.add_enabled(false, egui::Button::new("\u{271B}"))
        .on_disabled_hover_text("Picking on the screen from this dialog is not built yet.");
}
