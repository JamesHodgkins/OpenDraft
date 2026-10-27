//! The Paragraph dialog, laid out like AutoCAD's: Tab, Left Indent, Right
//! Indent, Paragraph Alignment, Paragraph Spacing and Paragraph Line Spacing,
//! with OK and Cancel. The values live in a [`Draft`] while it is open; OK
//! applies them as one `ParagraphEdit` (one undo step) and Cancel drops them.
//!
//! Only what the text format is known to carry is live: left tab stops,
//! first line and hanging indent, and left, centre, right and justified
//! alignment. The rest of AutoCAD's dialog is drawn greyed out until there
//! are AutoCAD-made samples of its controls (see `ui_plan.md`, slice 13).

mod draft;
mod parts;

use eframe::egui::{self, vec2, Rect, RichText};
use od_core::text::{ParagraphAlignment, TabKind};
use od_ui::widgets::{dialog, dropdown, group_box, number_field_width, primary_button};
use od_ui::widgets::{field_label, secondary_button};
use od_ui::Theme;

use self::draft::Draft;
use self::parts::{check_group, tab_glyph, NOT_YET};
use super::state::MTextEditor;

const TITLE: &str = "Paragraph";
const WIDTH: f32 = 540.0;
const LEFT_COLUMN: f32 = 290.0;
const FIELD: f32 = 96.0;

/// Show the dialog when it is open. Returns its rectangle, so a click in it
/// is not taken for a click on the drawing.
pub fn show(
    ctx: &egui::Context,
    theme: &Theme,
    editor: &mut MTextEditor,
    now: f64,
) -> Option<Rect> {
    let id = egui::Id::new("mtext_paragraph_draft");
    if !editor.paragraph_dialog {
        ctx.data_mut(|d| d.remove::<Draft>(id));
        return None;
    }
    let view = editor.paragraph();
    // A caret in another paragraph (or an undo) starts the values afresh.
    let mut draft: Draft = ctx
        .data(|d| d.get_temp::<Draft>(id))
        .filter(|d| d.seed == view.style)
        .unwrap_or_else(|| Draft::of(&view.style));
    let (mut open, mut apply, mut cancel) = (true, false, false);
    dialog(ctx, theme, TITLE, &mut open, WIDTH, |ui| {
        if view.protected {
            let note = "This paragraph's formatting cannot be edited yet.";
            ui.label(RichText::new(note).color(theme.colors.text_weak));
            ui.add_space(4.0);
        }
        ui.add_enabled_ui(!view.protected, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(LEFT_COLUMN);
                    tab_group(ui, theme, &mut draft);
                });
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    indent_groups(ui, theme, &mut draft);
                });
            });
            alignment_group(ui, theme, &mut draft);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(LEFT_COLUMN);
                    spacing_group(ui, theme);
                });
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    line_spacing_group(ui, theme);
                });
            });
        });
        ui.add_space(10.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.add_enabled_ui(!view.protected, |ui| {
                apply = primary_button(ui, theme, "OK").clicked();
            });
            cancel = secondary_button(ui, theme, "Cancel").clicked();
        });
    });
    if apply {
        editor.set_paragraph(&draft.edit(), false, now);
    }
    if apply || cancel || !open {
        editor.paragraph_dialog = false;
        ctx.data_mut(|d| d.remove::<Draft>(id));
    } else {
        ctx.data_mut(|d| d.insert_temp(id, draft));
    }
    ctx.memory(|m| m.area_rect(egui::Id::new(TITLE)))
}

fn tab_group(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft) {
    group_box(ui, theme, "Tab", |ui| {
        let colour = theme.colors.text;
        ui.horizontal(|ui| {
            let mut kind = TabKind::Left;
            for (this, live) in [
                (TabKind::Left, true),
                (TabKind::Center, false),
                (TabKind::Right, false),
                (TabKind::Decimal, false),
            ] {
                ui.add_enabled_ui(live, |ui| {
                    ui.radio_value(&mut kind, this, "");
                    tab_glyph(ui, this, colour);
                })
                .response
                .on_disabled_hover_text(NOT_YET);
                ui.add_space(6.0);
            }
        });
        ui.add_space(4.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                number_field_width(ui, theme, &mut draft.tab_position, "", 0.1, 120.0);
                ui.add_space(4.0);
                tab_list(ui, theme, draft);
            });
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                let size = vec2(theme.unit * 9.0, theme.control_height);
                let add = ui.add_sized(size, egui::Button::new("Add")).clicked();
                let selected = draft.selected_tab.is_some();
                let modify = ui
                    .add_enabled(selected, egui::Button::new("Modify").min_size(size))
                    .clicked();
                let remove = ui
                    .add_enabled(selected, egui::Button::new("Remove").min_size(size))
                    .clicked();
                if add {
                    draft.add_tab();
                } else if modify {
                    draft.modify_tab();
                } else if remove {
                    draft.remove_tab();
                }
            });
        });
        ui.add_space(6.0);
        field_label(ui, theme, "Specify Decimal Style:");
        ui.add_enabled_ui(false, |ui| {
            dropdown(
                ui,
                theme,
                "mtext_decimal_style",
                "'.' period",
                140.0,
                |_| {},
            );
        })
        .response
        .on_disabled_hover_text(NOT_YET);
    });
}

/// The tab stops, one per row, in a box of their own.
fn tab_list(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft) {
    let mut picked = None;
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0_f32, theme.colors.border))
        .inner_margin(egui::Margin::same(2))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("mtext_tab_list")
                .max_height(64.0)
                .min_scrolled_height(64.0)
                .show(ui, |ui| {
                    ui.set_width(116.0);
                    ui.set_min_height(60.0);
                    for (i, at) in draft.tabs.iter().enumerate() {
                        let on = draft.selected_tab == Some(i);
                        if ui.selectable_label(on, format!("{at:.2}")).clicked() {
                            picked = Some(i);
                        }
                    }
                });
        });
    if let Some(i) = picked {
        draft.select_tab(i);
    }
}

fn indent_groups(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft) {
    group_box(ui, theme, "Left Indent", |ui| {
        egui::Grid::new("mtext_left_indent")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("First line:");
                position(ui, theme, &mut draft.first_line);
                ui.end_row();
                ui.label("Hanging:");
                position(ui, theme, &mut draft.hanging);
                ui.end_row();
            });
    });
    group_box(ui, theme, "Right Indent", |ui| {
        ui.add_enabled_ui(false, |ui| {
            ui.horizontal(|ui| {
                ui.label("Right:");
                number_field_width(ui, theme, &mut 0.0, "", 0.1, FIELD);
            });
        })
        .response
        .on_disabled_hover_text(NOT_YET);
    });
}

/// A distance from the left edge: not below zero.
fn position(ui: &mut egui::Ui, theme: &Theme, value: &mut f64) {
    if number_field_width(ui, theme, value, "", 0.1, FIELD).changed() {
        *value = value.clamp(0.0, 1e6);
    }
}

fn alignment_group(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft) {
    let mut on = draft.align_on;
    check_group(ui, theme, "Paragraph Alignment", &mut on, true, |ui| {
        ui.horizontal(|ui| {
            let kinds = [
                ("Left", ParagraphAlignment::Left, true),
                ("Center", ParagraphAlignment::Center, true),
                ("Right", ParagraphAlignment::Right, true),
                ("Justified", ParagraphAlignment::Justified, true),
                ("Distributed", ParagraphAlignment::Distributed, false),
            ];
            for (label, kind, live) in kinds {
                ui.add_enabled_ui(live, |ui| {
                    ui.radio_value(&mut draft.alignment, kind, label);
                })
                .response
                .on_disabled_hover_text(NOT_YET);
                ui.add_space(8.0);
            }
        });
    });
    draft.align_on = on;
}

fn spacing_group(ui: &mut egui::Ui, theme: &Theme) {
    check_group(ui, theme, "Paragraph Spacing", &mut false, false, |ui| {
        egui::Grid::new("mtext_spacing")
            .num_columns(2)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                field_label(ui, theme, "Before:");
                field_label(ui, theme, "After:");
                ui.end_row();
                number_field_width(ui, theme, &mut 0.0, "", 0.1, FIELD);
                number_field_width(ui, theme, &mut 0.0, "", 0.1, FIELD);
                ui.end_row();
            });
    });
}

fn line_spacing_group(ui: &mut egui::Ui, theme: &Theme) {
    check_group(
        ui,
        theme,
        "Paragraph Line Spacing",
        &mut false,
        false,
        |ui| {
            egui::Grid::new("mtext_line_spacing")
                .num_columns(2)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    field_label(ui, theme, "Line Spacing:");
                    field_label(ui, theme, "At:");
                    ui.end_row();
                    dropdown(
                        ui,
                        theme,
                        "mtext_line_spacing_kind",
                        "Multiple",
                        80.0,
                        |_| {},
                    );
                    number_field_width(ui, theme, &mut 1.0, "x", 0.1, 60.0);
                    ui.end_row();
                });
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use od_core::{MText, Point, TextStyle, TextStyleId};

    fn open_editor(contents: &str) -> MTextEditor {
        let text = MText::new(Point::new(0.0, 0.0), 2.0, contents, TextStyleId::STANDARD);
        let mut e = MTextEditor::new(text, TextStyle::standard());
        e.paragraph_dialog = true;
        e
    }

    fn frame(ctx: &egui::Context, editor: &mut MTextEditor) -> Option<Rect> {
        let theme = Theme::default();
        theme.apply(ctx);
        let mut rect = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            rect = show(ctx, &theme, editor, 0.0);
        });
        rect
    }

    #[test]
    fn it_draws_while_open_and_not_when_closed() {
        let ctx = egui::Context::default();
        let mut e = open_editor("hello");
        frame(&ctx, &mut e);
        assert!(frame(&ctx, &mut e).is_some_and(|r| r.width() > 400.0));
        e.paragraph_dialog = false;
        assert!(frame(&ctx, &mut e).is_none());
    }

    #[test]
    fn it_draws_for_a_paragraph_it_cannot_edit() {
        let ctx = egui::Context::default();
        let mut e = open_editor("\\pxsa2;hello");
        assert!(e.paragraph().protected);
        frame(&ctx, &mut e);
        assert!(frame(&ctx, &mut e).is_some());
        assert_eq!(e.contents(), "\\pxsa2;hello", "opening changes nothing");
    }
}
