//! The Field dialog, laid out like AutoCAD's: a category list, the field
//! names, a preview of the value, the formats that suit the field, and the
//! field expression, with OK and Cancel. It only chooses a field; the caller
//! puts it where it belongs (the text being edited, or an attribute).

use eframe::egui::{self, vec2, Rect, RichText};
use od_core::block::field_kinds::{formats, KINDS};
use od_core::block::{DataField, FieldCategory, FieldContext, FieldKind};
use od_core::Document;
use od_ui::widgets::{dialog, dropdown, field_label, primary_button, secondary_button};
use od_ui::Theme;

const TITLE: &str = "Field";
const WIDTH: f32 = 560.0;
const LEFT: f32 = 210.0;
const LIST_HEIGHT: f32 = 230.0;

/// What is chosen so far.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FieldDraft {
    /// `None` is "All".
    pub category: Option<FieldCategory>,
    pub kind: &'static str,
    /// An index into [`formats`] for the kind; 0 is `(none)`.
    pub format: usize,
}

impl Default for FieldDraft {
    fn default() -> Self {
        Self {
            category: None,
            kind: KINDS[0].name,
            format: 0,
        }
    }
}

impl FieldDraft {
    /// The fields the category lists.
    pub fn listed(&self) -> Vec<&'static FieldKind> {
        KINDS
            .iter()
            .filter(|k| self.category.is_none_or(|c| c == k.category))
            .collect()
    }

    /// Show another category; the field stays chosen if it is still listed.
    pub fn select_category(&mut self, category: Option<FieldCategory>) {
        self.category = category;
        if !self.listed().iter().any(|k| k.name == self.kind) {
            if let Some(first) = self.listed().first() {
                self.kind = first.name;
            }
            self.format = 0;
        }
    }

    pub fn select_kind(&mut self, name: &'static str) {
        if self.kind != name {
            self.kind = name;
            self.format = 0;
        }
    }

    /// The field as chosen, not yet evaluated.
    pub fn field(&self, context: &FieldContext) -> Option<DataField> {
        let code = formats(self.kind, context)
            .get(self.format)
            .map(|f| f.code.clone())?;
        DataField::of_kind(self.kind, &code)
    }
}

/// What the dialog did this frame.
pub(crate) enum Chosen {
    Open,
    Insert(Box<DataField>),
    Cancel,
}

/// A bordered, scrolling list of `rows` (text, selected); the row clicked.
fn list(
    ui: &mut egui::Ui,
    theme: &Theme,
    id: &str,
    width: f32,
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
                .max_height(LIST_HEIGHT)
                .min_scrolled_height(LIST_HEIGHT)
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

/// Show the dialog. Also gives its rectangle, so a click in it is not taken
/// for a click on the drawing.
pub(crate) fn show(
    ctx: &egui::Context,
    theme: &Theme,
    draft: &mut FieldDraft,
    context: &FieldContext,
) -> (Chosen, Option<Rect>) {
    let mut open = true;
    let (mut ok, mut cancel) = (false, false);
    let field = draft.field(context);
    let evaluated = field
        .as_ref()
        .map(|f| f.evaluated(&Document::default(), context));
    dialog(ctx, theme, TITLE, &mut open, WIDTH, |ui| {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
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
                    for c in FieldCategory::ALL {
                        let picked = draft.category == Some(c);
                        if ui.selectable_label(picked, c.label()).clicked() {
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
                if let Some(i) = list(ui, theme, "field_names", LEFT, &rows) {
                    draft.select_kind(listed[i].name);
                }
            });
            ui.add_space(8.0);
            ui.vertical(|ui| {
                field_label(ui, theme, &format!("{}:", draft.kind));
                let value = evaluated
                    .as_ref()
                    .map_or_else(String::new, DataField::display);
                let mut value = value;
                ui.add_enabled(
                    false,
                    egui::TextEdit::singleline(&mut value).desired_width(ui.available_width()),
                );
                ui.add_space(6.0);
                field_label(ui, theme, "Format:");
                let choices = formats(draft.kind, context);
                let rows: Vec<_> = choices
                    .iter()
                    .enumerate()
                    .map(|(i, f)| (f.label.clone(), i == draft.format))
                    .collect();
                if let Some(i) = list(
                    ui,
                    theme,
                    "field_formats",
                    ui.available_width() - 8.0,
                    &rows,
                ) {
                    draft.format = i;
                }
            });
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
            ok = primary_button(ui, theme, "OK").clicked();
            cancel = secondary_button(ui, theme, "Cancel").clicked();
            ui.add_enabled(
                false,
                egui::Button::new(RichText::new("Help")).min_size(vec2(theme.unit * 9.0, 0.0)),
            );
        });
    });
    let rect = ctx.memory(|m| m.area_rect(egui::Id::new(TITLE)));
    let chosen = if ok {
        field.map_or(Chosen::Cancel, |f| Chosen::Insert(Box::new(f)))
    } else if cancel || !open {
        Chosen::Cancel
    } else {
        Chosen::Open
    };
    (chosen, rect)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_category_lists_only_its_fields_and_keeps_a_field_it_still_lists() {
        let mut d = FieldDraft::default();
        assert_eq!(d.listed().len(), KINDS.len());
        d.select_kind("Filename");
        d.format = 2;
        d.select_category(Some(FieldCategory::Document));
        assert_eq!((d.kind, d.format), ("Filename", 2), "still listed, kept");
        d.select_category(Some(FieldCategory::DateTime));
        assert_eq!((d.kind, d.format), ("Date", 0), "moved to the first listed");
        assert!(d
            .listed()
            .iter()
            .all(|k| k.category == FieldCategory::DateTime));
    }

    #[test]
    fn choosing_another_field_starts_its_format_afresh() {
        let mut d = FieldDraft {
            format: 3,
            ..FieldDraft::default()
        };
        d.select_kind("Date");
        assert_eq!(d.format, 3, "the same field keeps its format");
        d.select_kind("Filename");
        assert_eq!(d.format, 0);
    }

    #[test]
    fn the_field_built_carries_the_format_chosen() {
        let context = FieldContext::default();
        let mut d = FieldDraft::default();
        assert_eq!(d.field(&context).unwrap().format_code(), None);
        d.select_kind("Filename");
        d.format = 1;
        let field = d.field(&context).unwrap();
        assert_eq!(field.expression(), "%<\\AcVar Filename \\f \"%tc1\">%");
        d.format = 99;
        assert!(d.field(&context).is_none());
    }
}
