use od_core::{Attributes, Document, Entity, EntityId, LayoutId, Object};

use crate::Changes;

/// A reversible change to a [`Document`].
///
/// `revert` must exactly undo the most recent `apply`, and `apply` must be
/// repeatable after a `revert` (that is how redo works).
pub trait Action {
    /// Short human-readable name, e.g. "Add line".
    fn name(&self) -> String;
    fn apply(&mut self, doc: &mut Document);
    fn revert(&mut self, doc: &mut Document);
    /// What the last `apply` changed. It stays valid after `revert`, so undo
    /// can report the same change inverted.
    fn changes(&self) -> Changes;
}

/// Append an entity to the drawing, on the current layer and in the space
/// being edited. It gets its id, layer and space the first time the action
/// runs and keeps them through undo and redo.
pub struct AddEntity {
    entity: Entity,
    added: Option<(EntityId, Attributes, Option<LayoutId>)>,
    /// Put it just before this object in the drawing order (behind it)
    /// instead of at the end, if the object is there.
    behind: Option<EntityId>,
}

impl AddEntity {
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            added: None,
            behind: None,
        }
    }

    /// Like [`new`](Self::new), but drawn behind object `id` (a hatch goes
    /// behind its boundary). Undo and redo keep that place.
    pub fn behind(entity: Entity, id: EntityId) -> Self {
        Self {
            behind: Some(id),
            ..Self::new(entity)
        }
    }
}

impl Action for AddEntity {
    fn name(&self) -> String {
        format!("Add {}", self.entity.kind().display_name())
    }

    fn apply(&mut self, doc: &mut Document) {
        let (id, attrs, layout) = self
            .added
            .get_or_insert_with(|| {
                (
                    doc.allocate_id(),
                    doc.new_object_attributes(),
                    doc.active_layout(),
                )
            })
            .clone();
        let object = Object::new(id, self.entity.clone(), attrs, layout);
        match self.behind.and_then(|b| doc.position(b)) {
            Some(position) => doc.insert(position, object),
            None => doc.push(object),
        }
    }

    fn revert(&mut self, doc: &mut Document) {
        if let Some((id, ..)) = &self.added {
            doc.remove(*id);
        }
    }

    fn changes(&self) -> Changes {
        Changes::added(self.added.iter().map(|(id, ..)| *id).collect())
    }
}

/// Remove objects by id (the erase command).
pub struct RemoveEntities {
    ids: Vec<EntityId>,
    /// Removed objects with their original positions, ascending.
    removed: Vec<(usize, Object)>,
    /// Ids the last apply removed; kept after revert for `changes`.
    applied: Vec<EntityId>,
}

impl RemoveEntities {
    /// `ids` may be unsorted or contain duplicates; unknown ones are ignored
    /// when applied.
    pub fn new(mut ids: Vec<EntityId>) -> Self {
        ids.sort_unstable();
        ids.dedup();
        Self {
            ids,
            removed: Vec::new(),
            applied: Vec::new(),
        }
    }
}

impl Action for RemoveEntities {
    fn name(&self) -> String {
        "Erase".to_owned()
    }

    fn apply(&mut self, doc: &mut Document) {
        // One pass for all of them, keeping the original positions (ascending)
        // for the way back. Removing one at a time would rebuild the
        // document's id lookup for each, which is slow for a big selection.
        self.removed = doc.remove_many(&self.ids);
        self.applied = self.removed.iter().map(|(_, o)| o.id).collect();
    }

    fn revert(&mut self, doc: &mut Document) {
        doc.insert_many(std::mem::take(&mut self.removed));
    }

    fn changes(&self) -> Changes {
        Changes::removed(self.applied.clone())
    }
}

/// Replace the geometry of objects, keeping their ids and positions. Used for
/// property edits and, later, move/rotate/scale.
pub struct ModifyEntities {
    name: String,
    /// Id and the entity to swap in. Applying and reverting both swap, so
    /// each leaves the other state ready to be restored.
    changes: Vec<(EntityId, Entity)>,
}

impl ModifyEntities {
    pub fn new(name: impl Into<String>, changes: Vec<(EntityId, Entity)>) -> Self {
        Self {
            name: name.into(),
            changes,
        }
    }

    fn swap(&mut self, doc: &mut Document) {
        let new = self
            .changes
            .iter()
            .map(|(id, e)| (*id, e.clone()))
            .collect();
        for ((_, entity), old) in self.changes.iter_mut().zip(doc.replace_many(new)) {
            if let Some(old) = old {
                *entity = old;
            }
        }
    }
}

impl Action for ModifyEntities {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn apply(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn revert(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn changes(&self) -> Changes {
        Changes::changed(self.changes.iter().map(|(id, _)| *id).collect())
    }
}

/// Change objects' layer, colour, linetype or lineweight, keeping their ids
/// and positions.
pub struct ModifyAttributes {
    name: String,
    /// Applying and reverting both swap, as in [`ModifyEntities`].
    changes: Vec<(EntityId, Attributes)>,
}

impl ModifyAttributes {
    pub fn new(name: impl Into<String>, changes: Vec<(EntityId, Attributes)>) -> Self {
        Self {
            name: name.into(),
            changes,
        }
    }

    fn swap(&mut self, doc: &mut Document) {
        for (id, attrs) in &mut self.changes {
            if let Some(old) = doc.replace_attrs(*id, attrs.clone()) {
                *attrs = old;
            }
        }
    }
}

impl Action for ModifyAttributes {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn apply(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn revert(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn changes(&self) -> Changes {
        Changes::changed(self.changes.iter().map(|(id, _)| *id).collect())
    }
}

/// Add new objects (copies). Each gets its id the first time the action runs
/// and keeps it through undo and redo. They go to the space being edited,
/// unless `in_layout` names another.
pub struct AddCopies {
    name: String,
    copies: Vec<(Entity, Attributes)>,
    added: Vec<EntityId>,
    /// Decided the first time the action runs.
    space: Option<Option<LayoutId>>,
}

impl AddCopies {
    pub fn new(name: impl Into<String>, copies: Vec<(Entity, Attributes)>) -> Self {
        Self {
            name: name.into(),
            copies,
            added: Vec::new(),
            space: None,
        }
    }

    /// Put the copies on a layout's sheet (`Some`) or in model space (`None`),
    /// whatever space is being edited.
    pub fn in_layout(mut self, layout: Option<LayoutId>) -> Self {
        self.space = Some(layout);
        self
    }
}

impl Action for AddCopies {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn apply(&mut self, doc: &mut Document) {
        if self.added.is_empty() {
            self.added = self.copies.iter().map(|_| doc.allocate_id()).collect();
        }
        let layout = *self.space.get_or_insert(doc.active_layout());
        let objects = self
            .added
            .iter()
            .zip(&self.copies)
            .map(|(id, (entity, attrs))| Object::new(*id, entity.clone(), attrs.clone(), layout));
        doc.push_many(objects.collect());
    }

    fn revert(&mut self, doc: &mut Document) {
        doc.remove_many(&self.added);
    }

    fn changes(&self) -> Changes {
        Changes::added(self.added.clone())
    }
}

#[cfg(test)]
mod tests {
    use od_core::Point;

    use super::*;

    fn circle(r: f64) -> Entity {
        Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: r,
        }
    }

    /// A drawing of circles with the given radii, and their ids.
    fn doc_of(radii: &[f64]) -> (Document, Vec<EntityId>) {
        let mut doc = Document::default();
        let ids = radii.iter().map(|r| doc.add(circle(*r))).collect();
        (doc, ids)
    }

    fn radii(doc: &Document) -> Vec<Entity> {
        doc.objects().iter().map(|o| o.entity.clone()).collect()
    }

    fn circles(radii: &[f64]) -> Vec<Entity> {
        radii.iter().map(|r| circle(*r)).collect()
    }

    #[test]
    fn add_keeps_its_id_through_undo_and_redo() {
        let mut doc = Document::default();
        let mut action = AddEntity::new(circle(1.0));
        action.apply(&mut doc);
        let id = doc.objects()[0].id;
        action.revert(&mut doc);
        assert!(doc.is_empty());
        action.apply(&mut doc);
        assert_eq!(doc.objects()[0].id, id);
    }

    #[test]
    fn modify_swaps_in_place_and_reverts() {
        let (mut doc, ids) = doc_of(&[1.0, 2.0, 3.0]);
        let mut action = ModifyEntities::new("Change radius", vec![(ids[1], circle(9.0))]);
        action.apply(&mut doc);
        assert_eq!(radii(&doc), circles(&[1.0, 9.0, 3.0]));
        action.revert(&mut doc);
        assert_eq!(radii(&doc), circles(&[1.0, 2.0, 3.0]));
        action.apply(&mut doc);
        assert_eq!(radii(&doc), circles(&[1.0, 9.0, 3.0]));
    }

    #[test]
    fn remove_then_revert_restores_order_and_ids() {
        let (mut doc, ids) = doc_of(&[1.0, 2.0, 3.0, 4.0]);
        let before = doc.clone();
        let mut action = RemoveEntities::new(vec![ids[3], ids[1], ids[1]]);
        action.apply(&mut doc);
        assert_eq!(radii(&doc), circles(&[1.0, 3.0]));
        action.revert(&mut doc);
        assert_eq!(doc, before);
    }

    #[test]
    fn apply_again_after_revert_works_like_redo() {
        let (mut doc, ids) = doc_of(&[1.0, 2.0, 3.0]);
        let mut action = RemoveEntities::new(vec![ids[0]]);
        action.apply(&mut doc);
        action.revert(&mut doc);
        action.apply(&mut doc);
        assert_eq!(radii(&doc), circles(&[2.0, 3.0]));
    }

    #[test]
    fn unknown_ids_are_ignored() {
        let (mut doc, _) = doc_of(&[1.0]);
        let mut action = RemoveEntities::new(vec![EntityId(99)]);
        action.apply(&mut doc);
        assert_eq!(doc.len(), 1);
        action.revert(&mut doc);
        assert_eq!(doc.len(), 1);
    }

    #[test]
    fn actions_report_what_they_changed() {
        let (mut doc, ids) = doc_of(&[1.0, 2.0]);
        let mut remove = RemoveEntities::new(vec![ids[1], EntityId(99)]);
        remove.apply(&mut doc);
        assert_eq!(remove.changes(), Changes::removed(vec![ids[1]]));
        remove.revert(&mut doc);
        assert_eq!(remove.changes().inverted(), Changes::added(vec![ids[1]]));

        let mut modify = ModifyEntities::new("Change radius", vec![(ids[0], circle(5.0))]);
        modify.apply(&mut doc);
        assert_eq!(modify.changes(), Changes::changed(vec![ids[0]]));

        let mut add = AddEntity::new(circle(3.0));
        add.apply(&mut doc);
        assert_eq!(add.changes().added, vec![doc.objects()[2].id]);
    }

    #[test]
    fn copies_keep_their_ids_through_undo_and_redo() {
        let (mut doc, ids) = doc_of(&[1.0]);
        let attrs = doc.object(ids[0]).unwrap().attrs.clone();
        let mut action = AddCopies::new(
            "Copy",
            vec![(circle(2.0), attrs.clone()), (circle(3.0), attrs)],
        );
        action.apply(&mut doc);
        let copy_ids: Vec<_> = doc.objects()[1..].iter().map(|o| o.id).collect();
        assert_eq!(copy_ids.len(), 2);
        assert!(copy_ids.iter().all(|id| !ids.contains(id)));
        action.revert(&mut doc);
        assert_eq!(doc.len(), 1);
        action.apply(&mut doc);
        let again: Vec<_> = doc.objects()[1..].iter().map(|o| o.id).collect();
        assert_eq!(again, copy_ids);
    }

    /// A drawing of `n` circles, for the scaling guards below.
    fn big(n: usize) -> (Document, Vec<EntityId>) {
        let mut doc = Document::default();
        let ids = (0..n).map(|i| doc.add(circle(i as f64 + 1.0))).collect();
        (doc, ids)
    }

    /// An edit that touches many objects must be a pass over them, not a pass
    /// per object. With the document's id lookup rebuilt for every object,
    /// 50,000 took half a minute in a release build, so a limit of ten
    /// seconds, even for an unoptimised build, is loose and still catches it.
    const SLOW: std::time::Duration = std::time::Duration::from_secs(10);

    #[test]
    fn erasing_and_undoing_a_great_many_objects_scales() {
        let (mut doc, ids) = big(50_000);
        let started = std::time::Instant::now();
        // All of them, then every second one.
        let mut all = RemoveEntities::new(ids.clone());
        all.apply(&mut doc);
        assert!(doc.is_empty());
        all.revert(&mut doc);
        assert_eq!(doc.len(), 50_000);
        let mut half = RemoveEntities::new(ids.iter().step_by(2).copied().collect());
        half.apply(&mut doc);
        assert_eq!(doc.len(), 25_000);
        half.revert(&mut doc);
        assert_eq!(doc.len(), 50_000);
        assert_eq!(doc.position(ids[49_999]), Some(49_999));
        assert!(started.elapsed() < SLOW, "took {:?}", started.elapsed());
    }

    #[test]
    fn undoing_an_add_of_a_great_many_copies_scales() {
        let mut doc = Document::default();
        let attrs = od_core::Attributes::on_layer(od_core::LayerId::DEFAULT);
        let copies = (0..50_000)
            .map(|i| (circle(i as f64 + 1.0), attrs.clone()))
            .collect();
        let started = std::time::Instant::now();
        let mut add = AddCopies::new("Copy", copies);
        add.apply(&mut doc);
        assert_eq!(doc.len(), 50_000);
        add.revert(&mut doc);
        assert!(doc.is_empty());
        assert!(started.elapsed() < SLOW, "took {:?}", started.elapsed());
    }
}
