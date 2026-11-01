//! Taking objects out of the drawing, putting them back, and swapping them
//! for others, a pass at a time. Each call is one pass over the objects from
//! the first one it touches and one update of the id lookup, so the cost does
//! not grow with how many objects are named. Edits that touch many objects
//! (erase, the undo of an add, exploding for a file) use these in bulk and
//! never a loop of the single-object calls, which is quadratic.

use super::{after, Document, Object};
use crate::{Attributes, Entity, EntityId};

impl Document {
    /// Insert at `position` in draw order (clamped to the end). To put back
    /// several, use [`Document::insert_many`]: this costs a pass over the
    /// objects after `position`, so calling it in a loop is quadratic.
    pub fn insert(&mut self, position: usize, object: Object) {
        self.insert_many(vec![(position, object)]);
    }

    /// Remove an object, returning where it was and what it was. To remove
    /// several, use [`Document::remove_many`]: this costs a pass over the
    /// objects after it, so calling it in a loop is quadratic.
    pub fn remove(&mut self, id: EntityId) -> Option<(usize, Object)> {
        self.remove_many(&[id]).pop()
    }

    /// Remove objects, returning each with the position it had, in ascending
    /// order of position. Unknown and repeated ids are ignored. One pass over
    /// the objects from the first one removed, whatever the number, which is
    /// why edits that touch many objects (erase, undo of an add) use this and
    /// never a loop of [`Document::remove`].
    pub fn remove_many(&mut self, ids: &[EntityId]) -> Vec<(usize, Object)> {
        let mut found: Vec<usize> = ids
            .iter()
            .filter_map(|id| self.positions.get(id).copied())
            .collect();
        found.sort_unstable();
        found.dedup();
        let Some(&first) = found.first() else {
            return Vec::new();
        };
        if found.len() == 1 {
            // The common case, without the machinery for several.
            let object = self.objects.remove(first);
            self.positions.remove(&object.id);
            self.reindex_from(first);
            self.spatial.removed(&[&object], &self.objects);
            return vec![(first, object)];
        }
        let mut doomed = found.iter().copied().peekable();
        let mut at = first;
        let taken: Vec<Object> = self
            .objects
            .extract_if(first.., |_| {
                let hit = doomed.peek() == Some(&at);
                if hit {
                    doomed.next();
                }
                at += 1;
                hit
            })
            .collect();
        for object in &taken {
            self.positions.remove(&object.id);
        }
        self.reindex_from(first);
        let gone: Vec<&Object> = taken.iter().collect();
        self.spatial.removed(&gone, &self.objects);
        found.into_iter().zip(taken).collect()
    }

    /// Put back objects taken out by [`Document::remove_many`], each at its
    /// old position (a position past the end appends). One pass from the first
    /// position, whatever the number, like `remove_many`.
    pub fn insert_many(&mut self, mut items: Vec<(usize, Object)>) {
        if items.len() == 1 {
            // The common case, without the machinery for several.
            if let Some((position, object)) = items.pop() {
                let position = position.min(self.objects.len());
                self.next_id = self.next_id.max(after(object.id.0));
                self.note_attribute_id(&object.entity);
                self.objects.insert(position, object);
                self.reindex_from(position);
                self.spatial
                    .added(&[&self.objects[position]], &self.objects);
            }
            return;
        }
        items.sort_by_key(|(position, _)| *position);
        let Some(&(first, _)) = items.first() else {
            return;
        };
        let first = first.min(self.objects.len());
        let total = self.objects.len() + items.len();
        let ids: Vec<EntityId> = items.iter().map(|(_, o)| o.id).collect();
        let mut rest = self.objects.split_off(first).into_iter();
        let mut items = items.into_iter().peekable();
        self.objects.reserve(total - first);
        while self.objects.len() < total {
            let next_is_item = items
                .peek()
                .is_some_and(|(position, _)| *position <= self.objects.len());
            let object = if next_is_item {
                items.next().map(|(_, object)| object)
            } else {
                rest.next()
                    .or_else(|| items.next().map(|(_, object)| object))
            };
            let Some(object) = object else { break };
            self.next_id = self.next_id.max(after(object.id.0));
            self.note_attribute_id(&object.entity);
            self.objects.push(object);
        }
        self.reindex_from(first);
        let added: Vec<&Object> = ids
            .iter()
            .filter_map(|id| self.positions.get(id).map(|&i| &self.objects[i]))
            .collect();
        self.spatial.added(&added, &self.objects);
    }

    /// Replace each object `pieces` says to (it returns the entities and
    /// attributes to put in its place, or `None` to keep the object) in one
    /// pass. The pieces take the object's place in draw order and its layout,
    /// each under a fresh id. Returns how many objects were replaced. This is
    /// for turning many objects into several each (exploding blocks and
    /// dimensions for a file that cannot hold them): doing it with
    /// [`Document::remove`] and [`Document::insert`] for each is quadratic.
    pub fn replace_each(
        &mut self,
        mut pieces: impl FnMut(&Object) -> Option<Vec<(Entity, Attributes)>>,
    ) -> usize {
        let old = std::mem::take(&mut self.objects);
        let mut objects = Vec::with_capacity(old.len());
        let mut replaced = 0;
        let mut spaces = std::collections::HashSet::new();
        for object in old {
            let Some(parts) = pieces(&object) else {
                objects.push(object);
                continue;
            };
            replaced += 1;
            spaces.insert(object.layout);
            for (entity, attrs) in parts {
                let id = self.allocate_id();
                objects.push(Object::new(id, entity, attrs, object.layout));
            }
        }
        self.objects = objects;
        self.positions.clear();
        self.reindex_from(0);
        self.spatial.rebuild_spaces(&spaces, &self.objects);
        replaced
    }

    /// Record where the objects from `from` on are. Everything before it is
    /// unchanged by an insert or remove at `from`, so it is left alone.
    fn reindex_from(&mut self, from: usize) {
        for (i, object) in self.objects.iter().enumerate().skip(from) {
            self.positions.insert(object.id, i);
        }
    }
}
