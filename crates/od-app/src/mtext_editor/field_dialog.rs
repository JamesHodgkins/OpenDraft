//! The Field dialog inside the text editor: choosing a field puts it at the
//! caret, over the selection, as one step of the editor's own history.
//!
//! An object field asks for an object: "Select object" hides the dialog and
//! the next click on an object in the drawing (Esc goes back) is the answer;
//! the dialog then shows again with the object chosen.

use eframe::egui::{self, Rect};
use od_core::block::field_kinds::OBJECT;
use od_core::block::{FieldDraft, FieldStatus, FieldValue};
use od_core::Document;
use od_ui::Theme;

use super::state::MTextEditor;
use crate::field_context::current_time;
use crate::field_picker::{show as show_picker, Chosen};

const PICKING: &str = "Select an object for the field. Esc goes back to the dialog.";

/// Show the dialog when it is open. Returns its rectangle, so a click in it
/// is not taken for a click on the drawing.
pub fn show(
    ctx: &egui::Context,
    theme: &Theme,
    editor: &mut MTextEditor,
    now: f64,
) -> Option<Rect> {
    let id = egui::Id::new("mtext_field_draft");
    if !editor.field_dialog {
        ctx.data_mut(|d| d.remove::<FieldDraft>(id));
        return None;
    }
    let mut draft: FieldDraft = ctx
        .data(|d| d.get_temp(id))
        .unwrap_or_else(|| FieldDraft::new(true));
    // An object picked while the dialog was hidden.
    if let Some(choice) = editor.field_object.take() {
        draft.select_kind(OBJECT);
        draft.pick(choice);
    }
    if editor.field_pick {
        ctx.data_mut(|d| d.insert_temp(id, draft));
        return None;
    }
    let mut context = editor.field_context.clone();
    context.now = Some(current_time());
    let (chosen, rect) = show_picker(ctx, theme, &mut draft, &context);
    match chosen {
        Chosen::Open => ctx.data_mut(|d| d.insert_temp(id, draft)),
        Chosen::PickObject => {
            ctx.data_mut(|d| d.insert_temp(id, draft));
            editor.field_pick = true;
            editor.notice = Some(PICKING.into());
        }
        Chosen::Insert(field) => {
            let mut field = field.evaluated(&Document::default(), &context);
            // An object's value is what it showed when it was picked; the
            // drawing is not at hand here, and `UPDATEFIELD` renews it.
            if let Some(value) = draft.object_value().filter(|_| draft.is_object()) {
                field.cached = Some(FieldValue::Text(value.to_owned()));
                field.status = FieldStatus::Success;
            }
            editor.insert_field(field, now);
            editor.field_dialog = false;
            ctx.data_mut(|d| d.remove::<FieldDraft>(id));
        }
        Chosen::Cancel => {
            editor.field_dialog = false;
            ctx.data_mut(|d| d.remove::<FieldDraft>(id));
        }
    }
    rect
}
