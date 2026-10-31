use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

use od_core::{Attributes, Document, Entity, EntityId};

use crate::{Action, AddCopies, AddEntity, Changes, ModifyEntities, RemoveEntities};

/// Several actions that undo and redo as one step.
pub(crate) struct Batch {
    pub(crate) name: String,
    pub(crate) actions: Vec<Box<dyn Action>>,
}

impl Action for Batch {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn apply(&mut self, doc: &mut Document) {
        for action in &mut self.actions {
            action.apply(doc);
        }
    }

    fn revert(&mut self, doc: &mut Document) {
        for action in self.actions.iter_mut().rev() {
            action.revert(doc);
        }
    }

    fn changes(&self) -> Changes {
        Changes::merged(self.actions.iter().map(|a| a.changes()))
    }
}

/// Edits made inside [`Session::transaction`](crate::Session::transaction).
/// Each edit is applied to the document straight away, so later steps see the
/// earlier ones; the session records them all as one undo step when the
/// transaction commits, or reverts them if it fails.
pub struct Transaction<'a> {
    document: &'a mut Document,
    done: Vec<Box<dyn Action>>,
}

impl<'a> Transaction<'a> {
    pub(crate) fn new(document: &'a mut Document) -> Self {
        Self {
            document,
            done: Vec::new(),
        }
    }

    /// The drawing as it is now, including this transaction's edits so far.
    pub fn document(&self) -> &Document {
        self.document
    }

    /// Reserve an id for a layer about to be added with [`AddLayer`](crate::AddLayer).
    pub fn allocate_layer_id(&mut self) -> od_core::LayerId {
        self.document.allocate_layer_id()
    }

    /// Apply `action` now; it becomes part of the transaction's undo step.
    pub fn perform(&mut self, mut action: impl Action + 'static) {
        action.apply(self.document);
        self.done.push(Box::new(action));
    }

    /// Append an entity on the current layer.
    pub fn add(&mut self, entity: Entity) {
        self.perform(AddEntity::new(entity));
    }

    /// Add an entity on the current layer, drawn behind object `id`.
    pub fn add_behind(&mut self, entity: Entity, id: EntityId) {
        self.perform(AddEntity::behind(entity, id));
    }

    /// Append entities with the given attributes (layer, colour, ...), as
    /// copies of existing objects are.
    pub fn add_with(&mut self, copies: Vec<(Entity, Attributes)>) {
        self.perform(AddCopies::new("Add", copies));
    }

    /// Replace the geometry of object `id` with what `change` makes of it.
    /// Does nothing if there is no such object.
    pub fn modify(&mut self, id: EntityId, change: impl FnOnce(&Entity) -> Entity) {
        if let Some(entity) = self.document.get(id) {
            let next = change(entity);
            self.perform(ModifyEntities::new("Modify", vec![(id, next)]));
        }
    }

    /// Delete objects. Unknown ids are ignored.
    pub fn remove(&mut self, ids: Vec<EntityId>) {
        self.perform(RemoveEntities::new(ids));
    }

    /// Undo everything done so far, newest first.
    pub(crate) fn rollback(mut self) {
        self.revert_all();
    }

    fn revert_all(&mut self) {
        while let Some(mut action) = self.done.pop() {
            action.revert(self.document);
        }
    }

    /// Run `edit` on this transaction. Should it panic, the edits made so
    /// far are reverted before the panic carries on, so the drawing is not
    /// left changed with no undo step to take the change back.
    pub(crate) fn run<T, E>(
        &mut self,
        edit: impl FnOnce(&mut Transaction) -> Result<T, E>,
    ) -> Result<T, E> {
        match catch_unwind(AssertUnwindSafe(|| edit(self))) {
            Ok(result) => result,
            Err(payload) => {
                self.revert_all();
                resume_unwind(payload)
            }
        }
    }

    /// The edits as one action named `name`, or `None` if nothing was done.
    pub(crate) fn finish(mut self, name: &str) -> Option<Box<dyn Action>> {
        if self.done.is_empty() {
            return None;
        }
        // Hatches that follow what this changed are updated in the same step.
        let changes = Changes::merged(self.done.iter().map(|a| a.changes()));
        let follow = crate::associations::updates(self.document, &changes);
        if !follow.is_empty() {
            self.perform(ModifyEntities::new("Update associations", follow));
        }
        Some(Box::new(Batch {
            name: name.to_owned(),
            actions: self.done,
        }))
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Entity, Point};

    use crate::{AddEntity, ModifyEntities, Session};

    fn circle(r: f64) -> Entity {
        Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: r,
        }
    }

    #[test]
    fn a_transaction_is_one_undo_step() {
        let mut session = Session::default();
        session
            .transaction("Two circles", |tx| {
                tx.perform(AddEntity::new(circle(1.0)));
                tx.perform(AddEntity::new(circle(2.0)));
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(session.document.len(), 2);

        let step = session.undo().unwrap();
        assert_eq!(step.name, "Two circles");
        assert_eq!(step.changes.removed.len(), 2);
        assert!(session.document.is_empty());
        assert!(session.undo().is_none(), "both edits were one step");

        session.redo();
        assert_eq!(session.document.len(), 2);
    }

    #[test]
    fn later_steps_see_earlier_ones() {
        let mut session = Session::default();
        session
            .transaction("Add and grow", |tx| {
                tx.perform(AddEntity::new(circle(1.0)));
                let id = tx.document().objects()[0].id;
                tx.perform(ModifyEntities::new("Grow", vec![(id, circle(5.0))]));
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(session.document.objects()[0].entity, circle(5.0));
        // The object was added and changed in one step: reported as added only.
        session.undo();
        assert!(session.document.is_empty());
    }

    #[test]
    fn an_error_reverts_everything_and_leaves_no_history() {
        let mut session = Session::default();
        session.perform(AddEntity::new(circle(9.0)));
        let before = session.document.objects().to_vec();
        let result = session.transaction("Doomed", |tx| {
            tx.perform(AddEntity::new(circle(1.0)));
            tx.perform(AddEntity::new(circle(2.0)));
            Err::<(), _>("cancelled")
        });
        assert_eq!(result, Err("cancelled"));
        assert_eq!(session.document.objects(), before);
        assert_eq!(session.undo().unwrap().name, "Add circle");
        assert!(!session.history.can_undo());
    }

    #[test]
    fn an_empty_transaction_records_nothing() {
        let mut session = Session::default();
        session.transaction("Nothing", |_| Ok::<_, ()>(())).unwrap();
        assert!(!session.history.can_undo());
    }
}
