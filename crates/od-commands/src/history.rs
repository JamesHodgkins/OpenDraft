use od_core::Document;

use crate::{Action, Changes};

const MAX_DEPTH: usize = 1000;

/// An undo or redo that happened: the action's name and what it changed.
#[derive(Debug, PartialEq, Eq)]
pub struct Step {
    pub name: String,
    pub changes: Changes,
}

/// An action with the number it was given when first performed. Undoing and
/// redoing it keeps the number, so "where the history is" can be compared.
struct Entry {
    serial: u64,
    provenance: Provenance,
    action: Box<dyn Action>,
}

/// Who made an edit: the user in the app, or an automation connection
/// (identified by its grant). Kept for audit and for deciding who may undo it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Provenance {
    #[default]
    User,
    Automation(u64),
}

/// The step at the top of the undo or redo stack. `token` is the entry's
/// serial: it stays the same through undo and redo, and no other edit ever
/// gets it, so it identifies the step a caller saw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepInfo {
    pub token: u64,
    pub name: String,
    pub provenance: Provenance,
}

/// Undo/redo stacks of performed [`Action`]s.
#[derive(Default)]
pub struct History {
    undo_stack: Vec<Entry>,
    redo_stack: Vec<Entry>,
    last_serial: u64,
}

impl History {
    /// Apply `action` to `doc` and record it. Clears the redo stack. Returns
    /// what the action changed.
    pub fn perform(&mut self, doc: &mut Document, mut action: Box<dyn Action>) -> Changes {
        action.apply(doc);
        self.record(action)
    }

    /// Record an action that has already been applied. Clears the redo stack.
    pub(crate) fn record(&mut self, action: Box<dyn Action>) -> Changes {
        self.record_by(action, Provenance::User)
    }

    /// As [`record`](Self::record), noting who made the edit.
    pub(crate) fn record_by(&mut self, action: Box<dyn Action>, provenance: Provenance) -> Changes {
        let changes = action.changes();
        self.redo_stack.clear();
        self.last_serial += 1;
        self.undo_stack.push(Entry {
            serial: self.last_serial,
            provenance,
            action,
        });
        if self.undo_stack.len() > MAX_DEPTH {
            self.undo_stack.remove(0);
        }
        changes
    }

    /// Revert the last action. `None` if there is nothing to undo.
    pub fn undo(&mut self, doc: &mut Document) -> Option<Step> {
        let mut entry = self.undo_stack.pop()?;
        entry.action.revert(doc);
        let step = Step {
            name: entry.action.name(),
            changes: entry.action.changes().inverted(),
        };
        self.redo_stack.push(entry);
        Some(step)
    }

    /// Re-apply the last undone action. `None` if there is nothing to redo.
    pub fn redo(&mut self, doc: &mut Document) -> Option<Step> {
        let mut entry = self.redo_stack.pop()?;
        entry.action.apply(doc);
        let step = Step {
            name: entry.action.name(),
            changes: entry.action.changes(),
        };
        self.undo_stack.push(entry);
        Some(step)
    }

    /// Identifies where the history stands: the same value comes back when
    /// undo and redo return to the same step, and never for a different set
    /// of edits. 0 is the start, before any edit. Used to tell whether a
    /// drawing differs from what was saved.
    pub fn position(&self) -> u64 {
        self.undo_stack.last().map_or(0, |e| e.serial)
    }

    /// The step undo would revert next.
    pub fn top_undo(&self) -> Option<StepInfo> {
        self.undo_stack.last().map(Self::info)
    }

    /// The step redo would re-apply next.
    pub fn top_redo(&self) -> Option<StepInfo> {
        self.redo_stack.last().map(Self::info)
    }

    fn info(entry: &Entry) -> StepInfo {
        StepInfo {
            token: entry.serial,
            name: entry.action.name(),
            provenance: entry.provenance,
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_name(&self) -> Option<String> {
        self.undo_stack.last().map(|e| e.action.name())
    }

    pub fn redo_name(&self) -> Option<String> {
        self.redo_stack.last().map(|e| e.action.name())
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Entity, Point};

    use super::*;
    use crate::AddEntity;

    fn line() -> Box<dyn Action> {
        Box::new(AddEntity::new(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 1.0),
        }))
    }

    #[test]
    fn perform_undo_redo_round_trip() {
        let mut doc = Document::default();
        let mut history = History::default();

        history.perform(&mut doc, line());
        assert_eq!(doc.len(), 1);
        assert!(history.can_undo());
        assert!(!history.can_redo());
        assert_eq!(history.undo_name().as_deref(), Some("Add line"));

        let undone = history.undo(&mut doc).unwrap();
        assert_eq!(undone.name, "Add line");
        assert!(doc.is_empty());
        assert!(history.can_redo());

        let redone = history.redo(&mut doc).unwrap();
        assert_eq!(redone.name, "Add line");
        assert_eq!(doc.len(), 1);
        // Undo reports the object as removed, redo as added again.
        assert_eq!(undone.changes.removed, redone.changes.added);
        assert_eq!(redone.changes.added.len(), 1);
    }

    #[test]
    fn the_position_returns_when_undo_and_redo_return() {
        let mut doc = Document::default();
        let mut history = History::default();
        assert_eq!(history.position(), 0);
        history.perform(&mut doc, line());
        let one = history.position();
        history.perform(&mut doc, line());
        let two = history.position();
        assert!(one != 0 && two != one);
        history.undo(&mut doc);
        assert_eq!(history.position(), one);
        history.redo(&mut doc);
        assert_eq!(history.position(), two);
        history.undo(&mut doc);
        history.undo(&mut doc);
        assert_eq!(history.position(), 0);
    }

    #[test]
    fn a_different_edit_after_undo_is_a_different_position() {
        let mut doc = Document::default();
        let mut history = History::default();
        history.perform(&mut doc, line());
        let before = history.position();
        history.undo(&mut doc);
        history.perform(&mut doc, line());
        assert_ne!(history.position(), before, "not the state that was saved");
    }

    #[test]
    fn new_action_clears_redo() {
        let mut doc = Document::default();
        let mut history = History::default();
        history.perform(&mut doc, line());
        history.undo(&mut doc);
        assert!(history.can_redo());

        history.perform(&mut doc, line());
        assert!(!history.can_redo());
    }

    #[test]
    fn undo_and_redo_on_empty_history_are_none() {
        let mut doc = Document::default();
        let mut history = History::default();
        assert_eq!(history.undo(&mut doc), None);
        assert_eq!(history.redo(&mut doc), None);
    }

    #[test]
    fn history_depth_is_capped() {
        let mut doc = Document::default();
        let mut history = History::default();
        for _ in 0..MAX_DEPTH + 10 {
            history.perform(&mut doc, line());
        }
        let mut undone = 0;
        while history.undo(&mut doc).is_some() {
            undone += 1;
        }
        assert_eq!(undone, MAX_DEPTH);
    }
}
