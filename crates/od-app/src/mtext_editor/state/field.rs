//! Data fields in the editor's draft.

use od_core::block::{DataField, MAX_FIELDS_PER_TEXT};
use od_core::text::replace_with_control;

use super::MTextEditor;
use crate::mtext_editor::history::Kind;

impl MTextEditor {
    /// Put a data field at the caret, over the selection: its marker goes in
    /// the contents and the field with the text's others. One undo step.
    pub fn insert_field(&mut self, field: DataField, now: f64) -> bool {
        if self.working.fields.len() >= MAX_FIELDS_PER_TEXT {
            self.notice = Some("This text holds as many fields as it may.".into());
            return false;
        }
        let marker = od_core::mtext::marker(self.working.fields.len());
        let edit = replace_with_control(&self.working.contents, self.selected_range(), &marker);
        // The layout reads the field, so it goes in before the step is applied.
        self.working.fields.push(field);
        let done = self.apply_edit(edit, Kind::Other, now);
        if !done {
            self.working.fields.pop();
        }
        done
    }
}
