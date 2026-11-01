//! The Field dialog inside the text editor: choosing a field puts it at the
//! caret, over the selection, as one step of the editor's own history.

use eframe::egui::{self, Rect};
use od_core::block::FieldDraft;
use od_core::Document;
use od_ui::Theme;

use super::state::MTextEditor;
use crate::field_context::current_time;
use crate::field_picker::{show as show_picker, Chosen};

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
        .unwrap_or_else(|| FieldDraft::new(false));
    let mut context = editor.field_context.clone();
    context.now = Some(current_time());
    let (chosen, rect) = show_picker(ctx, theme, &mut draft, &context);
    match chosen {
        // An editor-hosted dialog never offers objects, so never asks to pick.
        Chosen::Open | Chosen::PickObject => ctx.data_mut(|d| d.insert_temp(id, draft)),
        Chosen::Insert(field) => {
            let field = field.evaluated(&Document::default(), &context);
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
