//! Character formatting in the editor: what the selection looks like, a
//! change to it, and a change made at a bare caret, which the next typed text
//! takes.

use od_core::text::{
    apply_char_edit, format_in, replace_text, toggle_stack, CharEdit, EditRefusal, Formatted,
    SelectionFormat,
};

use super::{MTextEditor, Pos};
use crate::mtext_editor::history::Kind;

impl MTextEditor {
    /// What the selection looks like (or the text before a bare caret), with
    /// any pending change laid over it.
    pub fn format(&self) -> SelectionFormat {
        format_in(
            &self.working.contents,
            self.working.nominal_height,
            self.selected_range(),
        )
        .with(&self.pending)
    }

    /// The family bold and italic are written for when the text has no font
    /// control of its own: the style's, or the bundled one.
    fn family(&self) -> String {
        match &self.style.font {
            od_core::FontDescriptor::TrueType { family, .. } => family.clone(),
            od_core::FontDescriptor::OdRoman { .. } => od_core::OD_ROMAN_FAMILY.to_owned(),
            _ => "DejaVu Sans".to_owned(),
        }
    }

    /// Whether the text's style draws in OD Roman, whose pen width the
    /// toolbar can set.
    pub fn uses_stroke_font(&self) -> bool {
        matches!(self.style.font, od_core::FontDescriptor::OdRoman { .. })
    }

    /// The style's pen width, as a fraction of the text height.
    pub fn style_weight(&self) -> f64 {
        self.style.stroke_weight
    }

    /// Whether a change is waiting for the next typed text.
    pub fn has_pending_format(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Forget a pending change: the caret moved, or the text changed in a
    /// way the change does not belong to.
    pub(super) fn clear_pending(&mut self) {
        self.pending = CharEdit::default();
        self.open_group = None;
    }

    /// Apply `edit` to the selection; at a bare caret, keep it for the next
    /// typed text.
    pub fn set_format(&mut self, edit: CharEdit, now: f64) -> bool {
        self.change_format(edit, now, Kind::Other)
    }

    /// Like [`MTextEditor::set_format`], for a value being dragged: a run
    /// of these is one undo step.
    pub fn adjust_format(&mut self, edit: CharEdit, now: f64) -> bool {
        self.change_format(edit, now, Kind::Format)
    }

    fn change_format(&mut self, edit: CharEdit, now: f64, kind: Kind) -> bool {
        if edit.is_empty() {
            return false;
        }
        if !self.has_selection() {
            self.pending.merge(&edit);
            self.open_group = None;
            self.notice = None;
            return true;
        }
        let range = self.selected_range();
        match apply_char_edit(&self.working.contents, range, &edit, &self.family()) {
            Ok(done) => {
                self.adopt(done, kind, now);
                true
            }
            Err(why) => {
                self.notice = Some(why.to_string());
                false
            }
        }
    }

    /// Take a result that rewrote the selection: one history step, with the
    /// same text still selected, whichever way it was selected.
    pub(super) fn adopt(&mut self, done: Formatted, kind: Kind, now: f64) {
        let before = self.snapshot();
        self.history.record(before, kind, now, false);
        let forward = self.anchor.source <= self.active.source;
        let (start, end) = (done.range.start, done.range.end);
        let at = |source| Pos {
            source,
            upstream: false,
        };
        (self.anchor, self.active) = if forward {
            (at(start), at(end))
        } else {
            (at(end), at(start))
        };
        self.working.contents = done.contents;
        self.layout = self.working.layout(&self.style);
        self.preferred_x = None;
        self.notice = None;
    }

    /// Stack the selected text (`1/2`, `1^2`, `1#2`), or unstack a selected
    /// stack.
    pub fn toggle_stack(&mut self, now: f64) -> bool {
        if !self.has_selection() {
            self.notice = Some(EditRefusal::NoDivider.to_string());
            return false;
        }
        match toggle_stack(&self.working.contents, self.selected_range()) {
            Ok(done) => {
                self.adopt(done, Kind::Other, now);
                true
            }
            Err(why) => {
                self.notice = Some(why.to_string());
                false
            }
        }
    }

    /// Type `text` with the pending change: inside the group it began, if
    /// the caret is still just after it, else in a new one.
    pub(super) fn insert_formatted(&mut self, text: &str, now: f64) -> bool {
        let collapsed = !self.has_selection();
        let result = match self.open_group {
            Some(p) if collapsed && self.active.source == p + 1 => {
                // Before the closing brace of the group the last text made.
                replace_text(&self.working.contents, p..p, text)
                    .map(|edited| (edited.contents, edited.caret, edited.caret))
            }
            _ => self.wrap_insert(text),
        };
        match result {
            Ok((contents, end, brace)) => {
                let before = self.snapshot();
                let kind = if collapsed { Kind::Typing } else { Kind::Other };
                self.history.record(before, kind, now, collapsed);
                self.working.contents = contents;
                self.layout = self.working.layout(&self.style);
                let at = Pos {
                    source: end + 1,
                    upstream: false,
                };
                (self.anchor, self.active) = (at, at);
                self.open_group = Some(brace);
                self.preferred_x = None;
                self.notice = None;
                true
            }
            Err(why) => {
                self.notice = Some(why.to_string());
                false
            }
        }
    }

    /// The contents with `text` typed over the selection and wrapped in the
    /// pending change; where the text ends and where the group's closing
    /// brace is (the same place, in the group this makes).
    fn wrap_insert(&self, text: &str) -> Result<(String, usize, usize), EditRefusal> {
        let edited = replace_text(&self.working.contents, self.selected_range(), text)?;
        let start = self.selected_range().start;
        let done = apply_char_edit(
            &edited.contents,
            start..edited.caret,
            &self.pending,
            &self.family(),
        )?;
        let end = done.range.end;
        // The text is now followed by the closing brace of its group.
        debug_assert_eq!(done.contents.as_bytes().get(end), Some(&b'}'));
        Ok((done.contents, end, end))
    }
}
