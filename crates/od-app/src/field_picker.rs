//! The Field dialog, laid out like AutoCAD's: a category list, the field
//! names, a preview of the value, the formats that suit the field (for an
//! object field, the properties of the object picked), and the field
//! expression, with OK and Cancel. It only chooses a field; the caller puts
//! it where it belongs (the text being edited, or a new text).

use eframe::egui::{self, vec2, Rect, RichText};
use od_core::block::{DataField, FieldCategory, FieldContext, FieldDraft};
use od_core::Document;
use od_ui::widgets::{dialog, dropdown, field_label, primary_button, secondary_button};
use od_ui::Theme;

const TITLE: &str = "Field";
const WIDTH: f32 = 560.0;
const LEFT: f32 = 210.0;
const LIST_HEIGHT: f32 = 230.0;

/// What the dialog did this frame.
pub(crate) enum Chosen {
    Open,
    Insert(Box<DataField>),
    /// The user asked to pick an object for an object field.
    PickObject,
    Cancel,
}

/// A bordered, scrolling list of `rows` (text, selected); the row clicked.
fn list(
    ui: &mut egui::Ui,
    theme: &Theme,
    id: &str,
    width: f32,
    height: f32,
    rows: &[(String, bool)],
) -> Option<usize> {
    let mut clicked = None;
    egui::Frame::new()
        .fill(theme.colors.input_bg)
        .stroke(egui::Stroke::new(1.0_f32, theme.colors.border))
        .inner_margin(2.0)
        .show(ui, |ui| {
            ui.set_width(width);
            egui::ScrollArea::vertical()
                .id_salt(id)
                .max_height(height)
                .min_scrolled_height(height)
                .show(ui, |ui| {
                    ui.set_width(width);
                    for (i, (text, selected)) in rows.iter().enumerate() {
                        if ui.selectable_label(*selected, text).clicked() {
                            clicked = Some(i);
                        }
                    }
                });
        });
    clicked
}

/// The left column: the category and the names it lists.
fn names_column(ui: &mut egui::Ui, theme: &Theme, draft: &mut FieldDraft) {
    ui.set_width(LEFT);
    field_label(ui, theme, "Field category:");
    let shown = draft.category.map_or("All", FieldCategory::label);
    dropdown(ui, theme, "field_category", shown, LEFT, |ui| {
        if ui
            .selectable_label(draft.category.is_none(), "All")
            .clicked()
        {
            draft.select_category(None);
        }
        for c in draft.categories() {
            if ui
                .selectable_label(draft.category == Some(c), c.label())
                .clicked()
            {
                draft.select_category(Some(c));
            }
        }
    });
    ui.add_space(6.0);
    field_label(ui, theme, "Field names:");
    let listed = draft.listed();
    let rows: Vec<_> = listed
        .iter()
        .map(|k| (k.name.to_owned(), k.name == draft.kind))
        .collect();
    if let Some(i) = list(ui, theme, "field_names", LEFT, LIST_HEIGHT, &rows) {
        draft.select_kind(listed[i].name);
    }
}

/// The right column: the preview, then the formats or properties. Whether
/// "Select object" was pressed.
fn value_column(
    ui: &mut egui::Ui,
    theme: &Theme,
    draft: &mut FieldDraft,
    context: &FieldContext,
    preview: &str,
) -> bool {
    let mut pick = false;
    field_label(ui, theme, &format!("{}:", draft.kind));
    let mut shown = preview.to_owned();
    ui.add_enabled(
        false,
        egui::TextEdit::singleline(&mut shown).desired_width(ui.available_width()),
    );
    ui.add_space(6.0);
    let mut height = LIST_HEIGHT;
    if draft.is_object() {
        ui.horizontal(|ui| {
            pick = ui
                .add(egui::Button::new("Select object"))
                .on_hover_text("Close the dialog and click an object in the drawing")
                .clicked();
            let named = draft
                .object
                .as_ref()
                .map_or("No object selected", |o| &o.label);
            ui.label(RichText::new(named).color(theme.colors.text_weak));
        });
        ui.add_space(4.0);
        height -= 30.0;
    }
    let title = if draft.is_object() {
        "Property:"
    } else {
        "Format:"
    };
    field_label(ui, theme, title);
    let rows: Vec<_> = draft
        .formats(context)
        .iter()
        .enumerate()
        .map(|(i, f)| (f.label.clone(), i == draft.format))
        .collect();
    let width = ui.available_width() - 8.0;
    if let Some(i) = list(ui, theme, "field_formats", width, height, &rows) {
        draft.format = i;
    }
    pick
}

/// Show the dialog. Also gives its rectangle, so a click in it is not taken
/// for a click on the drawing.
pub(crate) fn show(
    ctx: &egui::Context,
    theme: &Theme,
    draft: &mut FieldDraft,
    context: &FieldContext,
) -> (Chosen, Option<Rect>) {
    let mut open = true;
    let (mut ok, mut cancel, mut pick) = (false, false, false);
    let field = draft.field(context);
    let preview = match (&field, draft.is_object()) {
        (_, true) => draft.object_value().unwrap_or_default().to_owned(),
        (Some(f), false) => f.evaluated(&Document::default(), context).display(),
        (None, false) => String::new(),
    };
    dialog(ctx, theme, TITLE, &mut open, WIDTH, |ui| {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| names_column(ui, theme, draft));
            ui.add_space(8.0);
            ui.vertical(|ui| pick = value_column(ui, theme, draft, context, &preview));
        });
        ui.add_space(8.0);
        field_label(ui, theme, "Field expression:");
        let mut expression = field
            .as_ref()
            .map_or_else(String::new, DataField::expression);
        ui.add_enabled(
            false,
            egui::TextEdit::multiline(&mut expression)
                .desired_rows(2)
                .desired_width(ui.available_width()),
        );
        ui.add_space(10.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.add_enabled_ui(field.is_some(), |ui| {
                ok = primary_button(ui, theme, "OK").clicked();
            });
            cancel = secondary_button(ui, theme, "Cancel").clicked();
            ui.add_enabled(
                false,
                egui::Button::new(RichText::new("Help")).min_size(vec2(theme.unit * 9.0, 0.0)),
            );
        });
    });
    let rect = ctx.memory(|m| m.area_rect(egui::Id::new(TITLE)));
    let chosen = match field {
        Some(f) if ok => Chosen::Insert(Box::new(f)),
        _ if pick => Chosen::PickObject,
        _ if cancel || !open => Chosen::Cancel,
        _ => Chosen::Open,
    };
    (chosen, rect)
}
