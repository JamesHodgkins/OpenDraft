//! Paragraph formatting in the editor: what the caret's paragraph looks like
//! (the ruler shows it) and a change to it.

use od_core::text::{
    apply_list, apply_paragraph_edit, break_list_item, list_kind_in, paragraph_view_in, ListKind,
    ParagraphEdit, ParagraphView,
};

use super::MTextEditor;
use crate::mtext_editor::history::Kind;

impl MTextEditor {
    /// The paragraph the selection starts in.
    pub fn paragraph(&self) -> ParagraphView {
        paragraph_view_in(&self.working.contents, self.selected_range().start)
    }

    /// Apply `edit` to the paragraphs the selection touches. `drag` marks a
    /// value being dragged, a run of which is one undo step. Returns whether
    /// anything changed; if it was refused, `notice` says why.
    pub fn set_paragraph(&mut self, edit: &ParagraphEdit, drag: bool, now: f64) -> bool {
        match apply_paragraph_edit(&self.working.contents, self.selected_range(), edit) {
            Ok(done) if done.contents == self.working.contents => false,
            Ok(done) => {
                let kind = if drag { Kind::Format } else { Kind::Other };
                self.adopt(done, kind, now);
                true
            }
            Err(why) => {
                self.notice = Some(why.to_string());
                false
            }
        }
    }

    /// Set the wrapping width (the ruler's handle), as one undo step, or as
    /// part of a drag. Returns whether it changed.
    pub fn set_width(&mut self, width: f64, drag: bool, now: f64) -> bool {
        if !width.is_finite() || width <= 0.0 || (width - self.working.reference_width).abs() < 1e-9
        {
            return false;
        }
        let kind = if drag { Kind::Format } else { Kind::Other };
        let before = self.snapshot();
        self.history.record(before, kind, now, false);
        self.working.reference_width = width;
        self.layout = self.working.layout(&self.style);
        self.preferred_x = None;
        true
    }

    /// Enter in a list item: the next item, or the end of the list from an
    /// empty one. `None` when the caret is not in a list, so the break is an
    /// ordinary one.
    pub(super) fn break_list_item(&mut self, now: f64) -> Option<bool> {
        match break_list_item(&self.working.contents, self.selected_range()) {
            Ok(Some(done)) => {
                self.adopt(done, Kind::Other, now);
                Some(true)
            }
            Ok(None) => None,
            Err(why) => {
                self.notice = Some(why.to_string());
                Some(false)
            }
        }
    }

    /// The list the selected paragraphs are in, if they are all in one of
    /// the same kind.
    pub fn list_kind(&self) -> Option<ListKind> {
        list_kind_in(&self.working.contents, self.selected_range())
    }

    /// Put the selected paragraphs in a list of `kind`, or take them out of
    /// one (`None`). The hanging indent is three text heights, as in
    /// AutoCAD's sample for text of height 1.
    pub fn set_list(&mut self, kind: Option<ListKind>, now: f64) -> bool {
        let indent = LIST_INDENT * self.working.nominal_height;
        match apply_list(&self.working.contents, self.selected_range(), kind, indent) {
            Ok(done) if done.contents == self.working.contents => false,
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
}

/// The hanging indent of a list, in text heights.
const LIST_INDENT: f64 = 3.0;
