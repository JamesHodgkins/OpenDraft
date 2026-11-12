//! The editor's own undo and redo. It is separate from the drawing's: nothing
//! reaches the drawing's history until the editor is accepted.

/// What kind of edit a step was, so a run of the same kind is one step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Typing,
    Backspace,
    Delete,
    /// A number dragged in the toolbar: a whole drag is one step.
    Format,
    /// Anything that is its own step: paste, cut, a selection replaced.
    Other,
}

/// Steps typed this close together (seconds) are one.
const COALESCE_SECONDS: f64 = 0.75;
const MAX_STEPS: usize = 128;
const MAX_BYTES: usize = 16 << 20;

/// The text and selection (as byte offsets and which side of a wrap) before
/// a step.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot<P> {
    pub contents: String,
    /// The wrapping width (0 for none), which the ruler can change.
    pub width: f64,
    pub anchor: P,
    pub active: P,
}

pub struct History<P> {
    undo: Vec<Snapshot<P>>,
    redo: Vec<Snapshot<P>>,
    /// The kind and time of the last step, while it can still be extended.
    last: Option<(Kind, f64)>,
    /// How many of the oldest steps have been dropped to stay in the limits.
    dropped: usize,
}

impl<P> Default for History<P> {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            last: None,
            dropped: 0,
        }
    }
}

impl<P: Clone> History<P> {
    /// Note that an edit of `kind` is about to happen at time `now`, with
    /// the state `before` it. A run of the same kind on a bare caret joins
    /// the step before it instead of adding one.
    pub fn record(&mut self, before: Snapshot<P>, kind: Kind, now: f64, collapsed: bool) {
        let joins = (collapsed || kind == Kind::Format)
            && kind != Kind::Other
            && self
                .last
                .is_some_and(|(k, at)| k == kind && now - at <= COALESCE_SECONDS && now >= at);
        self.last = Some((kind, now));
        self.redo.clear();
        if joins && !self.undo.is_empty() {
            return;
        }
        self.undo.push(before);
        self.trim();
    }

    /// Drop the oldest steps beyond the limits on count and total size.
    fn trim(&mut self) {
        let size = |h: &[Snapshot<P>]| h.iter().map(|s| s.contents.len()).sum::<usize>();
        while self.undo.len() > MAX_STEPS || (self.undo.len() > 1 && size(&self.undo) > MAX_BYTES) {
            self.undo.remove(0);
            self.dropped += 1;
        }
    }

    /// Moving the caret ends the run being extended.
    pub fn break_run(&mut self) {
        self.last = None;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Step back: `now` is the current state, kept for redo.
    pub fn undo(&mut self, now: Snapshot<P>) -> Option<Snapshot<P>> {
        let previous = self.undo.pop()?;
        self.redo.push(now);
        self.last = None;
        Some(previous)
    }

    pub fn redo(&mut self, now: Snapshot<P>) -> Option<Snapshot<P>> {
        let next = self.redo.pop()?;
        self.undo.push(now);
        self.last = None;
        Some(next)
    }

    /// Whether the start of the editing is still reachable by undoing.
    #[cfg(test)]
    pub fn complete(&self) -> bool {
        self.dropped == 0
    }

    /// How many steps can be undone.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.undo.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.undo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(text: &str) -> Snapshot<usize> {
        Snapshot {
            contents: text.to_owned(),
            width: 0.0,
            anchor: 0,
            active: 0,
        }
    }

    #[test]
    fn typing_in_a_burst_is_one_step_and_a_pause_or_other_kind_starts_another() {
        let mut h = History::default();
        h.record(snap(""), Kind::Typing, 0.0, true);
        h.record(snap("a"), Kind::Typing, 0.3, true);
        h.record(snap("ab"), Kind::Typing, 0.9, true);
        assert_eq!(h.len(), 1, "0.6 s apart each time");
        h.record(snap("abc"), Kind::Typing, 2.0, true);
        assert_eq!(h.len(), 2, "a pause");
        h.record(snap("abcd"), Kind::Backspace, 2.1, true);
        assert_eq!(h.len(), 3, "another kind");
        h.record(snap("abc"), Kind::Backspace, 2.2, true);
        assert_eq!(h.len(), 3);
        h.record(snap("ab"), Kind::Other, 2.3, true);
        h.record(snap("a"), Kind::Other, 2.4, true);
        assert_eq!(h.len(), 5, "other edits are never joined");
    }

    #[test]
    fn a_selection_replaced_or_a_moved_caret_is_a_new_step() {
        let mut h = History::default();
        h.record(snap(""), Kind::Typing, 0.0, true);
        h.record(snap("a"), Kind::Typing, 0.1, false);
        assert_eq!(h.len(), 2, "typing over a selection");
        h.break_run();
        h.record(snap("b"), Kind::Typing, 0.2, true);
        assert_eq!(h.len(), 3, "the caret moved in between");
    }

    #[test]
    fn undo_and_redo_swap_states_and_a_new_edit_clears_redo() {
        let mut h = History::default();
        h.record(snap("one"), Kind::Other, 0.0, true);
        h.record(snap("two"), Kind::Other, 1.0, true);
        let back = h.undo(snap("three")).unwrap();
        assert_eq!(back.contents, "two");
        let back = h.undo(snap("two")).unwrap();
        assert_eq!(back.contents, "one");
        assert!(h.undo(snap("one")).is_none());
        let mut h = History::default();
        h.record(snap("a"), Kind::Other, 0.0, true);
        let a = h.undo(snap("b")).unwrap();
        assert_eq!(a.contents, "a");
        assert!(h.can_redo() && !h.can_undo());
        let b = h.redo(snap("a")).unwrap();
        assert_eq!(b.contents, "b");
        assert!(h.can_undo());
        h.undo(snap("b"));
        h.record(snap("a"), Kind::Other, 5.0, true);
        assert!(!h.can_redo(), "a new edit ends the redo line");
    }

    #[test]
    fn history_is_bounded_by_count_and_size() {
        let mut h = History::default();
        for i in 0..300 {
            h.record(snap("x"), Kind::Other, f64::from(i), true);
        }
        assert_eq!(h.len(), 128, "the oldest are dropped");
        let mut h = History::default();
        let big = "y".repeat(5 << 20);
        for i in 0..6 {
            h.record(snap(&big), Kind::Other, f64::from(i), true);
        }
        assert!(
            h.len() <= 3,
            "16 MiB holds three 5 MiB steps, got {}",
            h.len()
        );
        assert!(!h.is_empty());
    }
}
