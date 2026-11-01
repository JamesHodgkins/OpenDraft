//! Recording an edit of the contents in the draft and its history.

use std::ops::Range;

use od_core::text::{replace_text, EditRefusal, Edited};

use super::{MTextEditor, Pos};
use crate::mtext_editor::history::Kind;

impl MTextEditor {
    /// Replace `range` with plain `text`, recording the step. Returns whether
    /// it was done; if not, `notice` says why.
    pub(super) fn apply(&mut self, range: Range<usize>, text: &str, kind: Kind, now: f64) -> bool {
        let edit = replace_text(&self.working.contents, range, text);
        self.apply_edit(edit, kind, now)
    }

    /// Put in the result of an edit of the contents, recording the step.
    pub(super) fn apply_edit(
        &mut self,
        edit: Result<Edited, EditRefusal>,
        kind: Kind,
        now: f64,
    ) -> bool {
        match edit {
            Ok(edited) => {
                let before = self.snapshot();
                let collapsed = !self.has_selection();
                self.history.record(before, kind, now, collapsed);
                self.working.contents = edited.contents;
                self.layout = self.working.layout(&self.style);
                let at = Pos {
                    source: edited.caret,
                    upstream: false,
                };
                self.anchor = at;
                self.active = at;
                self.preferred_x = None;
                self.notice = None;
                self.clear_pending();
                true
            }
            Err(why) => {
                self.notice = Some(why.to_string());
                false
            }
        }
    }
}
