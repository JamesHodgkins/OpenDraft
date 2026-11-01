//! The Attribute Editor (EATTEDIT), laid out as AutoCAD's: the block
//! and the selected attribute's tag at the top with Select block at the
//! right, then three tabs. Attribute lists the values; Text Options and
//! Properties edit the attribute selected in that list. OK and Apply hand
//! the draft back to the command, which makes the change.

use eframe::egui;
use od_commands::{Reply, Wants};
use od_core::block::attribute_draft::{EditorDraft, Justification};
use od_core::{Document, TextStyleId};
use od_ui::widgets;

use super::attribute_editor_table as table;
use super::OpenDraftApp;
use crate::attr_rows;
use crate::layers::AttrEdit;

const WIDTH: f32 = 460.0;
const FIELD: f32 = 230.0;

/// What the user pressed.
#[derive(Clone, Copy, PartialEq)]
enum Pressed {
    Nothing,
    Ok,
    Apply,
    Cancel,
    SelectBlock,
}

fn unavailable(ui: &mut egui::Ui, label: &str, why: &str) {
    let mut off = false;
    ui.add_enabled(false, egui::Checkbox::new(&mut off, label))
        .on_disabled_hover_text(why);
}

fn number(ui: &mut egui::Ui, value: &mut f64, speed: f64) {
    ui.add(
        egui::DragValue::new(value)
            .speed(speed)
            .fixed_decimals(4)
            .range(-1.0e9..=1.0e9),
    );
}

impl OpenDraftApp {
    pub(super) fn attribute_editor_window(&mut self, ctx: &egui::Context) {
        let wanted = self
            .state
            .waiting()
            .filter(|ask| ask.wants == Wants::AttributeEditorDialog)
            .map(|ask| ask.attribute_editor.clone());
        let Some(start) = wanted else {
            self.state.command_dialogs.editor = None;
            return;
        };
        if self.state.command_dialogs.editor.is_none() {
            self.state.command_dialogs.editor = start.map(|d| *d);
        }
        let Some(mut draft) = self.state.command_dialogs.editor.take() else {
            return;
        };
        let (mut open, mut pressed) = (true, Pressed::Nothing);
        let problem = draft.problem();
        let theme = &self.theme;
        let document = self.state.session().document();
        widgets::dialog(ctx, theme, "Attribute Editor", &mut open, WIDTH, |ui| {
            header(ui, theme, &draft, &mut pressed);
            ui.add_space(6.0);
            widgets::tab_strip(
                ui,
                theme,
                &mut draft.tab,
                &["Attribute", "Text Options", "Properties"],
            );
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.set_min_height(190.0);
                match draft.tab {
                    0 => attribute_tab(ui, theme, &mut draft),
                    1 => text_tab(ui, &mut draft, document),
                    _ => properties_tab(ui, theme, &mut draft, document),
                }
            });
            ui.add_space(12.0);
            if let Some(why) = &problem {
                ui.colored_label(ui.visuals().error_fg_color, why);
            }
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled_ui(false, |ui| {
                        widgets::secondary_button(ui, theme, "Help");
                    });
                    if widgets::secondary_button(ui, theme, "Cancel").clicked() {
                        pressed = Pressed::Cancel;
                    }
                    ui.add_enabled_ui(problem.is_none(), |ui| {
                        if widgets::primary_button(ui, theme, "OK").clicked() {
                            pressed = Pressed::Ok;
                        }
                    });
                    ui.add_enabled_ui(draft.is_changed() && problem.is_none(), |ui| {
                        if widgets::secondary_button(ui, theme, "Apply").clicked() {
                            pressed = Pressed::Apply;
                        }
                    });
                });
            });
        });
        match pressed {
            Pressed::Ok => self
                .state
                .reply(Reply::AttributeEditor(Box::new(draft), true)),
            Pressed::Apply => self
                .state
                .reply(Reply::AttributeEditor(Box::new(draft), false)),
            Pressed::SelectBlock => self
                .state
                .reply(Reply::AttributeEditorSelect(Box::new(draft))),
            Pressed::Cancel => self.state.cancel_command(),
            Pressed::Nothing if !open => self.state.cancel_command(),
            Pressed::Nothing => self.state.command_dialogs.editor = Some(draft),
        }
    }
}

/// The block and tag, and the button that picks another block reference.
fn header(ui: &mut egui::Ui, theme: &od_ui::Theme, draft: &EditorDraft, pressed: &mut Pressed) {
    let weak = |text: &str| egui::RichText::new(text).color(theme.colors.text_weak);
    ui.horizontal(|ui| {
        egui::Grid::new("eattedit_header")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                ui.label(weak("Block"));
                ui.label(&draft.block);
                ui.end_row();
                ui.label(weak("Tag"));
                ui.label(
                    draft
                        .rows
                        .get(draft.selected)
                        .map_or("", |r| r.tag.as_str()),
                );
                ui.end_row();
            });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            if widgets::secondary_button(ui, theme, "Select block").clicked() {
                *pressed = Pressed::SelectBlock;
            }
        });
    });
}

fn attribute_tab(ui: &mut egui::Ui, theme: &od_ui::Theme, draft: &mut EditorDraft) {
    table::list(ui, theme, draft);
    ui.add_space(8.0);
    let selected = draft.selected;
    if let Some(row) = draft.rows.get_mut(selected) {
        ui.horizontal(|ui| {
            ui.label("Value:");
            let edit = ui.add_enabled(
                !row.is_field,
                egui::TextEdit::singleline(&mut row.value).desired_width(WIDTH - 90.0),
            );
            if row.is_field {
                edit.on_disabled_hover_text(
                    "This value is a data field. Fields are not edited here yet.",
                );
            }
        });
    }
}

fn text_tab(ui: &mut egui::Ui, draft: &mut EditorDraft, document: &Document) {
    let selected = draft.selected;
    let Some(row) = draft.rows.get_mut(selected) else {
        return;
    };
    let styles: Vec<(TextStyleId, String)> = document
        .text_styles()
        .iter()
        .map(|s| (s.id, s.name.clone()))
        .collect();
    let t = &mut row.text;
    egui::Grid::new("eattedit_text")
        .num_columns(4)
        .spacing([10.0, 8.0])
        .show(ui, |ui| {
            ui.label("Text Style:");
            let shown = styles
                .iter()
                .find(|(id, _)| *id == t.style)
                .map_or("?", |(_, n)| n.as_str());
            egui::ComboBox::from_id_salt("eattedit_style")
                .width(140.0)
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for (id, name) in &styles {
                        ui.selectable_value(&mut t.style, *id, name);
                    }
                });
            ui.label("");
            ui.label("");
            ui.end_row();
            ui.label("Justification:");
            egui::ComboBox::from_id_salt("eattedit_justification")
                .width(140.0)
                .selected_text(t.justification.label())
                .show_ui(ui, |ui| {
                    for j in Justification::ALL {
                        ui.selectable_value(&mut t.justification, j, j.label());
                    }
                });
            ui.checkbox(&mut t.backwards, "Backwards");
            ui.checkbox(&mut t.upside_down, "Upside down");
            ui.end_row();
            ui.label("Height:");
            number(ui, &mut t.height, 0.1);
            ui.label("Width Factor:");
            number(ui, &mut t.width_factor, 0.01);
            ui.end_row();
            ui.label("Rotation:");
            number(ui, &mut t.rotation, 1.0);
            ui.label("Oblique Angle:");
            number(ui, &mut t.oblique, 1.0);
            ui.end_row();
            ui.label("");
            unavailable(ui, "Annotative", "Annotative text is not built yet.");
            ui.add_enabled(false, egui::Label::new("Boundary width:"));
            let mut width = 0.0;
            ui.add_enabled(false, egui::DragValue::new(&mut width));
            ui.end_row();
        });
}

fn properties_tab(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    draft: &mut EditorDraft,
    document: &Document,
) {
    let selected = draft.selected;
    let Some(row) = draft.rows.get_mut(selected) else {
        return;
    };
    let size = egui::vec2(FIELD, theme.control_height);
    let mut edit = None;
    egui::Grid::new("eattedit_properties")
        .num_columns(4)
        .spacing([10.0, 10.0])
        .show(ui, |ui| {
            ui.label("Layer:");
            let names: Vec<String> = document.layers().iter().map(|l| l.name.clone()).collect();
            let current = document.layer_position(row.look.layer);
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            let text = current.map_or("?", |i| names[i].as_str());
            if let Some(i) = attr_rows::cell_combo(
                ui,
                theme,
                ui.id().with("eattedit_layer"),
                rect.expand(2.0),
                text,
                &names,
                current,
            ) {
                edit = Some(AttrEdit::Layer(document.layers()[i].id));
            }
            ui.label("");
            ui.label("");
            ui.end_row();
            ui.label("Linetype:");
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            edit = attr_rows::linetype_value_field(
                ui,
                theme,
                ui.id().with("eattedit_linetype"),
                rect,
                document,
                Some(row.look.linetype.clone()),
            )
            .or(edit.take());
            ui.label("");
            ui.label("");
            ui.end_row();
            ui.label("Color:");
            let resolved = document.layer(row.look.layer).map(|l| l.color);
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            edit = attr_rows::color_value_field(
                ui,
                theme,
                ui.id().with("eattedit_color"),
                rect,
                Some(row.look.color.clone()),
                resolved,
            )
            .or(edit.take());
            ui.label("");
            ui.label("");
            ui.end_row();
            ui.label("Lineweight:");
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            edit = attr_rows::lineweight_value_field(
                ui,
                theme,
                ui.id().with("eattedit_lineweight"),
                rect,
                Some(row.look.lineweight.clone()),
            )
            .or(edit.take());
            ui.label("");
            ui.label("");
            ui.end_row();
            ui.add_enabled(false, egui::Label::new("Plot style:"));
            ui.add_enabled(false, egui::Button::new("ByLayer").min_size(size))
                .on_disabled_hover_text("Plot styles are not built yet.");
            ui.end_row();
        });
    match edit {
        Some(AttrEdit::Layer(id)) => row.look.layer = id,
        Some(AttrEdit::Color(c)) => row.look.color = c,
        Some(AttrEdit::Linetype(t)) => row.look.linetype = t,
        Some(AttrEdit::Lineweight(w)) => row.look.lineweight = w,
        _ => {}
    }
}
