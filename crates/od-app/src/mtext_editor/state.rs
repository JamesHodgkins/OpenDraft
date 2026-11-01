//! The text editor's draft: the text being edited, the selection, and every
//! operation on them. No egui here; the window turns keys and clicks into
//! these calls.
//!
//! The draft is a copy. Nothing reaches the drawing until the editor is
//! accepted, and its undo history is its own.

use std::ops::Range;
use std::rc::Rc;

use od_core::block::FieldContext;
use od_core::text::{plain_text_of, CharEdit, TextLayout};
use od_core::{MText, TextStyle};

use super::history::{History, Kind, Snapshot};

/// Where the caret stands: a byte of the contents, and which side of a wrap
/// when the end of one line and the start of the next are the same byte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pos {
    pub source: usize,
    pub upstream: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Left,
    Right,
    WordLeft,
    WordRight,
    Up,
    Down,
    Home,
    End,
    DocStart,
    DocEnd,
}

/// The longest pasted text kept (the contents' own limit still applies).
const MAX_PASTE_BYTES: usize = 1 << 20;

pub struct MTextEditor {
    original: MText,
    working: MText,
    style: TextStyle,
    layout: Rc<TextLayout>,
    anchor: Pos,
    active: Pos,
    /// The x a run of Up and Down keeps while the caret passes short lines.
    preferred_x: Option<f64>,
    history: History<Pos>,
    /// Why the last edit was refused, until the next one.
    pub notice: Option<String>,
    /// Text an input method is composing and has not committed yet. Shown
    /// beside the caret; it is not part of the draft or its history.
    pub composing: Option<String>,
    /// The Paragraph dialog is open.
    pub paragraph_dialog: bool,
    /// The Field dialog is open.
    pub field_dialog: bool,
    /// What a field may know of the drawing, as of when the editor opened.
    pub field_context: FieldContext,
    /// The drawing's text object this edit replaces, left out of the drawing
    /// while the draft is shown in its place.
    pub replaces: Option<od_core::EntityId>,
    /// A format chosen at a bare caret, taken by the next typed text.
    pending: CharEdit,
    /// Just before the closing brace of the group that text made, so more
    /// typing goes into the same group.
    open_group: Option<usize>,
    /// When the caret last moved or text last changed (seconds), so it shows
    /// solid for a moment after.
    pub active_since: f64,
}

impl MTextEditor {
    pub fn new(text: MText, style: TextStyle) -> Self {
        let layout = text.layout(&style);
        let end = Pos {
            source: text.contents.len(),
            upstream: false,
        };
        Self {
            original: text.clone(),
            working: text,
            style,
            layout,
            anchor: end,
            active: end,
            preferred_x: None,
            history: History::default(),
            notice: None,
            composing: None,
            paragraph_dialog: false,
            field_dialog: false,
            field_context: FieldContext::default(),
            replaces: None,
            pending: CharEdit::default(),
            open_group: None,
            active_since: 0.0,
        }
    }

    // ---- reading ----

    pub fn text(&self) -> &MText {
        &self.working
    }

    pub fn layout(&self) -> &Rc<TextLayout> {
        &self.layout
    }

    #[cfg(test)]
    pub fn contents(&self) -> &str {
        &self.working.contents
    }

    /// Whether the draft differs from what the editor was opened on.
    pub fn is_changed(&self) -> bool {
        self.working != self.original
    }

    /// The style the text is written in.
    pub fn style_id(&self) -> od_core::TextStyleId {
        self.working.style
    }

    /// Write the text in `style` instead: the layout is made again, the
    /// contents and the caret stay.
    pub fn set_style(&mut self, style: &TextStyle) {
        if style.id == self.working.style {
            return;
        }
        self.working.style = style.id;
        self.style = style.clone();
        self.layout = self.working.layout(&self.style);
        self.preferred_x = None;
    }

    /// Whether undoing can still reach the start (no step was dropped).
    #[cfg(test)]
    pub fn history_complete(&self) -> bool {
        self.history.complete()
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    fn index(&self, p: Pos) -> usize {
        self.layout.caret_at(p.source, p.upstream).unwrap_or(0)
    }

    fn pos_of(&self, index: usize) -> Pos {
        Pos {
            source: self.layout.carets.get(index).map_or(0, |c| c.source),
            upstream: self.layout.is_upstream(index),
        }
    }

    /// The caret's index in the layout's carets.
    pub fn caret(&self) -> usize {
        self.index(self.active)
    }

    /// The selection's two ends as caret indexes, anchor first.
    pub fn selection(&self) -> (usize, usize) {
        (self.index(self.anchor), self.index(self.active))
    }

    pub fn has_selection(&self) -> bool {
        self.anchor.source != self.active.source
    }

    /// The selected bytes of the contents.
    pub fn selected_range(&self) -> Range<usize> {
        let (a, b) = (self.anchor.source, self.active.source);
        a.min(b)..a.max(b)
    }

    // ---- selection and caret ----

    fn set_active(&mut self, active: Pos, extend: bool) {
        self.clear_pending();
        self.active = active;
        if !extend {
            self.anchor = active;
        }
        self.history.break_run();
    }

    pub fn move_caret(&mut self, to: Move, extend: bool) {
        let layout = Rc::clone(&self.layout);
        let here = self.caret();
        // Moving over a selection without extending first collapses it.
        if !extend && self.has_selection() && matches!(to, Move::Left | Move::Right) {
            let (a, b) = self.selection();
            let edge = if to == Move::Left { a.min(b) } else { a.max(b) };
            self.preferred_x = None;
            let at = self.pos_of(edge);
            self.set_active(at, false);
            return;
        }
        let vertical = matches!(to, Move::Up | Move::Down);
        if vertical && self.preferred_x.is_none() {
            self.preferred_x = layout.carets.get(here).map(|c| c.x);
        } else if !vertical {
            self.preferred_x = None;
        }
        let next = match to {
            Move::Left => layout.step(here, false),
            Move::Right => layout.step(here, true),
            Move::WordLeft => layout.word_step(here, false),
            Move::WordRight => layout.word_step(here, true),
            Move::Home => layout.line_start(here),
            Move::End => layout.line_end(here),
            Move::DocStart => 0,
            Move::DocEnd => layout.carets.len().saturating_sub(1),
            Move::Up | Move::Down => {
                let across = self.preferred_x.unwrap_or(0.0);
                match layout.vertical(here, across, to == Move::Up) {
                    Some(i) => i,
                    // On the first or last line it goes to that line's end.
                    None if to == Move::Up => layout.line_start(here),
                    None => layout.line_end(here),
                }
            }
        };
        let at = self.pos_of(next);
        self.set_active(at, extend);
    }

    pub fn select_all(&mut self) {
        self.clear_pending();
        // The whole source, so closing braces and trailing controls after the
        // last caret stop go with it.
        self.anchor = Pos {
            source: 0,
            upstream: false,
        };
        self.active = Pos {
            source: self.working.contents.len(),
            upstream: false,
        };
        self.preferred_x = None;
        self.history.break_run();
    }

    // ---- editing ----

    fn snapshot(&self) -> Snapshot<Pos> {
        Snapshot {
            contents: self.working.contents.clone(),
            width: self.working.reference_width,
            anchor: self.anchor,
            active: self.active,
        }
    }

    fn restore(&mut self, snapshot: Snapshot<Pos>) {
        self.working.contents = snapshot.contents;
        self.working.reference_width = snapshot.width;
        self.layout = self.working.layout(&self.style);
        self.anchor = snapshot.anchor;
        self.active = snapshot.active;
        self.preferred_x = None;
        self.notice = None;
        self.clear_pending();
    }

    /// Type `text` over the selection (or at the caret). A paragraph break
    /// is a newline in `text`.
    pub fn insert(&mut self, text: &str, now: f64) -> bool {
        if text.is_empty() {
            return false;
        }
        if text == "\n" {
            if let Some(done) = self.break_list_item(now) {
                return done;
            }
        }
        if self.has_pending_format() {
            return self.insert_formatted(text, now);
        }
        let kind = if self.has_selection() {
            Kind::Other
        } else {
            Kind::Typing
        };
        self.apply(self.selected_range(), text, kind, now)
    }

    /// Backspace: the selection, or the cluster (or word) before the caret.
    pub fn backspace(&mut self, word: bool, now: f64) -> bool {
        self.remove(word, false, now)
    }

    /// Delete: the selection, or the cluster (or word) after the caret.
    pub fn delete(&mut self, word: bool, now: f64) -> bool {
        self.remove(word, true, now)
    }

    fn remove(&mut self, word: bool, forward: bool, now: f64) -> bool {
        if self.has_selection() {
            return self.apply(self.selected_range(), "", Kind::Other, now);
        }
        let here = self.caret();
        let range = if word {
            let there = self.layout.word_step(here, forward);
            let (a, b) = (self.pos_of(here).source, self.pos_of(there).source);
            a.min(b)..a.max(b)
        } else {
            self.cluster_range(here, forward)
        };
        if range.is_empty() {
            return false;
        }
        let kind = if forward {
            Kind::Delete
        } else {
            Kind::Backspace
        };
        self.apply(range, "", kind, now)
    }

    /// The bytes of the cluster after (or before) caret `here`: just the
    /// cluster, so a formatting control standing between it and the caret
    /// stays. At the end of a line it is the paragraph break there.
    fn cluster_range(&self, here: usize, forward: bool) -> Range<usize> {
        let carets = &self.layout.carets;
        let (stop, other) = if forward {
            (here, self.layout.step(here, true))
        } else {
            (self.layout.step(here, false), here)
        };
        let (Some(s), Some(o)) = (carets.get(stop), carets.get(other)) else {
            return 0..0;
        };
        if stop == other {
            return 0..0;
        }
        if s.end > s.source {
            s.source..s.end
        } else {
            s.source..o.source
        }
    }

    /// The selection as plain text, for the clipboard.
    pub fn copy_text(&self) -> Option<String> {
        self.has_selection()
            .then(|| plain_text_of(&self.working.contents, self.selected_range()))
    }

    /// Remove the selection and return it as plain text.
    pub fn cut_text(&mut self, now: f64) -> Option<String> {
        let text = self.copy_text()?;
        self.apply(self.selected_range(), "", Kind::Other, now)
            .then_some(text)
    }

    /// Paste plain text over the selection. Control characters other than
    /// tab and line breaks are dropped; text over the limit is refused.
    pub fn paste(&mut self, text: &str, now: f64) -> bool {
        if text.len() > MAX_PASTE_BYTES {
            self.notice = Some("the pasted text is too long".to_owned());
            return false;
        }
        let clean: String = text
            .chars()
            .filter(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r'))
            .collect();
        if clean.is_empty() {
            return false;
        }
        self.apply(self.selected_range(), &clean, Kind::Other, now)
    }

    pub fn undo(&mut self) -> bool {
        match self.history.undo(self.snapshot()) {
            Some(before) => {
                self.restore(before);
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.history.redo(self.snapshot()) {
            Some(after) => {
                self.restore(after);
                true
            }
            None => false,
        }
    }
}

mod apply;
mod field;
mod formatting;
#[cfg(test)]
mod formatting_tests;
mod paragraph;
#[cfg(test)]
mod paragraph_tests;
mod pointer;
#[cfg(test)]
mod roman_tests;
#[cfg(test)]
mod stress_model;
#[cfg(test)]
mod stress_ops;
#[cfg(test)]
mod stress_session;
#[cfg(test)]
mod stress_steps;
#[cfg(test)]
mod stress_tests;
#[cfg(test)]
mod tests;
