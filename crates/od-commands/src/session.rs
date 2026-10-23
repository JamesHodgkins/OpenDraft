use od_core::{Document, LayerId, Layout, LayoutId};

use crate::transaction::Batch;
use crate::{Action, Changes, History, ModifyEntities, Provenance, Step, StepInfo, Transaction};

/// Why a [`Session::checked_transaction`] did not commit. Either way the
/// drawing is as it was.
#[derive(Debug, PartialEq, Eq)]
pub enum CheckedError<E> {
    /// The edit itself failed; it was rolled back.
    Edit(E),
    /// The edit left the drawing invalid; it was reverted before being
    /// recorded.
    Invalid(String),
}

/// Why an undo or redo asked for by token did not happen.
#[derive(Debug, PartialEq, Eq)]
pub enum HistoryError {
    /// There is nothing to undo or redo.
    Empty,
    /// The top step is not the one the caller saw.
    Moved { actual: u64 },
    /// The top step was made by someone the caller may not undo.
    NotPermitted(Provenance),
}

/// A document together with its undo history. All edits go through
/// [`Session::perform`] so they can be undone.
#[derive(Default)]
pub struct Session {
    pub document: Document,
    pub history: History,
    /// Where the history stood when the drawing was last saved (or opened).
    saved_at: u64,
    /// Counts effective changes (edits, undo, redo). Unlike the history
    /// position it never returns to an earlier value, so it can tell an
    /// external reader that the drawing moved on since it was read.
    revision: u64,
}

impl Session {
    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    /// Reserve an id for a layer about to be added.
    pub fn allocate_layer_id(&mut self) -> LayerId {
        self.document.allocate_layer_id()
    }

    /// Tell the document which space the view is in, and which layers the
    /// viewport being worked through has frozen. Not an edit: no revision,
    /// no history.
    pub fn set_active_space(&mut self, layout: Option<LayoutId>, frozen: Vec<LayerId>) {
        self.document.set_active_layout(layout);
        self.document.set_active_frozen(frozen);
    }

    /// Store layouts changed by panning or zooming a viewport. The drawing
    /// differs afterwards (the revision moves) but, like AutoCAD's zoom, it is
    /// not an undo step.
    pub fn store_view_layouts(&mut self, layouts: Vec<Layout>) {
        self.document.set_layouts(layouts);
        self.revision += 1;
    }

    /// A token that changes whenever an edit, undo or redo alters the drawing.
    /// It is not a file version and starts again from 0 for a new session.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// A session for a drawing just opened from a file: unmodified.
    pub fn with_document(document: Document) -> Self {
        Self {
            document,
            ..Self::default()
        }
    }

    /// A session for a drawing restored from an autosave: its content is in
    /// no saved file, so it counts as modified until it is saved.
    pub fn recovered(document: Document) -> Self {
        Self {
            document,
            saved_at: u64::MAX,
            ..Self::default()
        }
    }

    /// Record that the drawing as it is now is what is on disk.
    pub fn mark_saved(&mut self) {
        self.saved_at = self.history.position();
    }

    /// Record that what is on disk is the drawing as it stood at history
    /// position `position`: a save writes a copy taken then, and edits made
    /// while it was being written must still count as unsaved.
    pub fn mark_saved_at(&mut self, position: u64) {
        self.saved_at = position;
    }

    /// Whether the drawing differs from what was last saved or opened.
    /// Undoing back to the saved state makes it unmodified again.
    pub fn is_modified(&self) -> bool {
        self.history.position() != self.saved_at
    }

    /// Run `action` and report what it changed.
    pub fn perform(&mut self, action: impl Action + 'static) -> Changes {
        let mut action: Box<dyn Action> = Box::new(action);
        action.apply(&mut self.document);
        // Associative hatches that follow what it changed join its undo step.
        let follow = crate::associations::updates(&self.document, &action.changes());
        let changes = if follow.is_empty() {
            self.history.record(action)
        } else {
            let mut update = ModifyEntities::new("Update associations", follow);
            update.apply(&mut self.document);
            let name = action.name();
            self.history.record(Box::new(Batch {
                name,
                actions: vec![action, Box::new(update)],
            }))
        };
        self.revision += 1;
        self.debug_check();
        changes
    }

    /// In debug builds and tests, fail loudly the moment an edit leaves the
    /// drawing inconsistent, rather than when a file later fails to save.
    fn debug_check(&self) {
        #[cfg(debug_assertions)]
        if let Err(error) = self.document.validate() {
            panic!("an edit left the drawing invalid: {error}");
        }
    }

    /// Make several edits that undo as one step named `name`. If `edit`
    /// returns an error, every edit it made is reverted and nothing is
    /// recorded, so a cancelled command leaves no trace.
    pub fn transaction<T, E>(
        &mut self,
        name: &str,
        edit: impl FnOnce(&mut Transaction) -> Result<T, E>,
    ) -> Result<T, E> {
        let mut tx = Transaction::new(&mut self.document);
        match edit(&mut tx) {
            Ok(value) => {
                if let Some(batch) = tx.finish(name) {
                    self.history.record(batch);
                    self.revision += 1;
                }
                self.debug_check();
                Ok(value)
            }
            Err(error) => {
                tx.rollback();
                Err(error)
            }
        }
    }

    /// As [`transaction`](Self::transaction) for callers that cannot trust
    /// the edit to leave a valid drawing (automation). The result is
    /// validated in every build before it is recorded, and reverted if it
    /// fails. Returns the closure's value and what the step changed; a
    /// transaction that did nothing records nothing and changes no revision.
    pub fn checked_transaction<T, E>(
        &mut self,
        name: &str,
        provenance: Provenance,
        edit: impl FnOnce(&mut Transaction) -> Result<T, E>,
    ) -> Result<(T, Changes), CheckedError<E>> {
        let mut tx = Transaction::new(&mut self.document);
        let value = match edit(&mut tx) {
            Ok(value) => value,
            Err(error) => {
                tx.rollback();
                return Err(CheckedError::Edit(error));
            }
        };
        let Some(mut batch) = tx.finish(name) else {
            return Ok((value, Changes::default()));
        };
        if let Err(error) = self.document.validate() {
            batch.revert(&mut self.document);
            return Err(CheckedError::Invalid(error.to_string()));
        }
        let changes = self.history.record_by(batch, provenance);
        self.revision += 1;
        Ok((value, changes))
    }

    /// Undo the top step only if it is the one with `token` and `allowed`
    /// accepts who made it.
    pub fn undo_checked(
        &mut self,
        token: u64,
        allowed: impl Fn(Provenance) -> bool,
    ) -> Result<Step, HistoryError> {
        verify(self.history.top_undo(), token, allowed)?;
        self.undo().ok_or(HistoryError::Empty)
    }

    /// As [`undo_checked`](Self::undo_checked), for redo.
    pub fn redo_checked(
        &mut self,
        token: u64,
        allowed: impl Fn(Provenance) -> bool,
    ) -> Result<Step, HistoryError> {
        verify(self.history.top_redo(), token, allowed)?;
        self.redo().ok_or(HistoryError::Empty)
    }

    pub fn undo(&mut self) -> Option<Step> {
        let step = self.history.undo(&mut self.document);
        self.revision += u64::from(step.is_some());
        self.debug_check();
        step
    }

    pub fn redo(&mut self) -> Option<Step> {
        let step = self.history.redo(&mut self.document);
        self.revision += u64::from(step.is_some());
        self.debug_check();
        step
    }
}

fn verify(
    top: Option<StepInfo>,
    token: u64,
    allowed: impl Fn(Provenance) -> bool,
) -> Result<(), HistoryError> {
    let top = top.ok_or(HistoryError::Empty)?;
    if top.token != token {
        return Err(HistoryError::Moved { actual: top.token });
    }
    if !allowed(top.provenance) {
        return Err(HistoryError::NotPermitted(top.provenance));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use od_core::{Entity, Point};

    use super::*;
    use crate::AddEntity;

    fn add(session: &mut Session) {
        session.perform(AddEntity::new(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 1.0),
        }));
    }

    #[test]
    fn a_new_or_opened_session_is_unmodified() {
        assert!(!Session::default().is_modified());
        assert!(!Session::with_document(Document::default()).is_modified());
    }

    #[test]
    fn a_recovered_session_is_modified_until_saved() {
        let mut session = Session::recovered(Document::default());
        assert!(session.is_modified());
        session.mark_saved();
        assert!(!session.is_modified());
    }

    #[test]
    fn editing_modifies_and_saving_clears_it() {
        let mut session = Session::default();
        add(&mut session);
        assert!(session.is_modified());
        session.mark_saved();
        assert!(!session.is_modified());
        add(&mut session);
        assert!(session.is_modified());
    }

    #[test]
    fn undoing_back_to_the_saved_state_is_unmodified_again() {
        let mut session = Session::default();
        add(&mut session);
        session.mark_saved();
        add(&mut session);
        session.undo();
        assert!(!session.is_modified());
        session.redo();
        assert!(session.is_modified());
        // Undoing past the saved state is a change too.
        session.undo();
        session.undo();
        assert!(session.is_modified());
        session.redo();
        assert!(!session.is_modified());
    }

    #[test]
    fn a_different_edit_in_place_of_an_undone_one_is_modified() {
        let mut session = Session::default();
        add(&mut session);
        session.mark_saved();
        session.undo();
        add(&mut session);
        assert!(session.is_modified(), "same length, different edit");
    }

    #[test]
    fn a_transaction_counts_as_one_edit() {
        let mut session = Session::default();
        session
            .transaction("Two", |tx| {
                tx.add(Entity::Line {
                    start: Point::new(0.0, 0.0),
                    end: Point::new(1.0, 0.0),
                });
                Ok::<_, ()>(())
            })
            .unwrap();
        assert!(session.is_modified());
        session.mark_saved();
        session.undo();
        assert!(session.is_modified());
    }

    #[test]
    fn the_revision_only_moves_forward_on_effective_changes() {
        let mut session = Session::default();
        assert_eq!(session.revision(), 0);
        add(&mut session);
        assert_eq!(session.revision(), 1);
        session.undo();
        assert_eq!(session.revision(), 2);
        session.redo();
        assert_eq!(session.revision(), 3);
        // Nothing left to redo: no change, no new revision.
        session.redo();
        assert_eq!(session.revision(), 3);
        // Branching after an undo reaches a history position seen before,
        // but never an earlier revision.
        session.undo();
        let before = session.revision();
        add(&mut session);
        assert!(session.revision() > before);
    }

    #[test]
    fn an_empty_or_rolled_back_transaction_keeps_the_revision() {
        let mut session = Session::default();
        session.transaction("Nothing", |_| Ok::<_, ()>(())).unwrap();
        assert_eq!(session.revision(), 0);
        let failed = session.transaction("Fails", |tx| {
            tx.add(Entity::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(1.0, 0.0),
            });
            Err::<(), _>("no")
        });
        assert!(failed.is_err());
        assert_eq!(session.revision(), 0);
        session
            .transaction("One", |tx| {
                tx.add(Entity::Line {
                    start: Point::new(0.0, 0.0),
                    end: Point::new(1.0, 0.0),
                });
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(session.revision(), 1);
    }

    fn line() -> Entity {
        Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 0.0),
        }
    }

    #[test]
    fn a_checked_transaction_records_provenance_and_returns_changes() {
        let mut session = Session::default();
        let ((), changes) = session
            .checked_transaction("Draw", Provenance::Automation(7), |tx| {
                tx.add(line());
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(changes.added.len(), 1);
        assert_eq!(session.revision(), 1);
        let top = session.history().top_undo().unwrap();
        assert_eq!(top.provenance, Provenance::Automation(7));
        assert_eq!(top.name, "Draw");
    }

    #[test]
    fn a_checked_transaction_that_does_nothing_records_nothing() {
        let mut session = Session::default();
        let result = session.checked_transaction("Nothing", Provenance::User, |_| Ok::<_, ()>(()));
        assert_eq!(result.unwrap().1, Changes::default());
        assert_eq!(session.revision(), 0);
        assert!(!session.history().can_undo());
    }

    #[test]
    fn an_edit_that_leaves_the_drawing_invalid_is_reverted_before_recording() {
        let mut session = Session::default();
        add(&mut session);
        let before = session.document().objects().to_vec();
        let id = before[0].id;
        let mut attrs = before[0].attrs.clone();
        attrs.layer = od_core::LayerId(9_999);
        let result = session.checked_transaction("Bad layer", Provenance::User, |tx| {
            tx.add(line());
            tx.perform(crate::ModifyAttributes::new("x", vec![(id, attrs)]));
            Ok::<_, ()>(())
        });
        assert!(matches!(result, Err(CheckedError::Invalid(_))));
        assert_eq!(session.document().objects(), before);
        assert_eq!(session.revision(), 1, "only the first add counted");
        assert_eq!(session.history().top_undo().unwrap().name, "Add line");
    }

    #[test]
    fn a_failing_edit_is_rolled_back() {
        let mut session = Session::default();
        let result = session.checked_transaction("Fails", Provenance::User, |tx| {
            tx.add(line());
            Err::<(), _>("no")
        });
        assert_eq!(result, Err(CheckedError::Edit("no")));
        assert!(session.document().is_empty());
        assert_eq!(session.revision(), 0);
    }

    #[test]
    fn undo_by_token_checks_the_step_and_who_made_it() {
        let mut session = Session::default();
        session
            .checked_transaction("Mine", Provenance::Automation(1), |tx| {
                tx.add(line());
                Ok::<_, ()>(())
            })
            .unwrap();
        let mine = session.history().top_undo().unwrap().token;
        let own = |p| p == Provenance::Automation(1);
        // The user draws after the client looked: its token is now stale.
        add(&mut session);
        let theirs = session.history().top_undo().unwrap().token;
        assert_eq!(
            session.undo_checked(mine, own),
            Err(HistoryError::Moved { actual: theirs })
        );
        // The user's own step is not the client's to undo.
        assert_eq!(
            session.undo_checked(theirs, own),
            Err(HistoryError::NotPermitted(Provenance::User))
        );
        assert_eq!(session.document().len(), 2, "nothing was undone");
        session.undo();
        let step = session.undo_checked(mine, own).unwrap();
        assert_eq!(step.name, "Mine");
        // A restarted sidecar has a new grant, so cannot redo the old one.
        let successor = |p| p == Provenance::Automation(2);
        assert_eq!(
            session.redo_checked(mine, successor),
            Err(HistoryError::NotPermitted(Provenance::Automation(1)))
        );
        assert!(session.redo_checked(mine, own).is_ok());
        // The user's undone step is next on the redo stack now.
        assert!(matches!(
            session.redo_checked(mine, own),
            Err(HistoryError::Moved { .. })
        ));
    }
}
