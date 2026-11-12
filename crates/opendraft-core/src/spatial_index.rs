//! One space's R-tree: the broad phase of picking, snapping, selecting and
//! painting. It holds ids and boxes only and knows nothing of the document;
//! `document::spatial` decides when it is built, kept and used. The `rstar`
//! types stay in this file so the dependency can be swapped.

use std::collections::HashMap;

use rstar::{RTree, RTreeObject, AABB};

use crate::{Bounds, EntityId, Point};

/// Whether a box can go in the tree: four finite numbers, `min <= max`.
/// Anything else is kept aside (see [`Space::exceptional`]).
pub(crate) fn is_indexable(b: &Bounds) -> bool {
    [b.min.x, b.min.y, b.max.x, b.max.y]
        .iter()
        .all(|v| v.is_finite())
        && b.min.x <= b.max.x
        && b.min.y <= b.max.y
}

/// An object's id and the box around it. Equal when both are, which is what
/// lets the tree find the entry to remove.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Entry {
    id: EntityId,
    envelope: AABB<[f64; 2]>,
}

impl Entry {
    fn new(id: EntityId, b: &Bounds) -> Self {
        Self {
            id,
            envelope: AABB::from_corners([b.min.x, b.min.y], [b.max.x, b.max.y]),
        }
    }
}

impl RTreeObject for Entry {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

/// The boxes of one space: finite ones in a tree, the rest in a list.
#[derive(Clone, Debug, Default)]
pub(crate) struct Space {
    tree: RTree<Entry>,
    /// The entries in the tree, by id: the exact old entry to take out.
    by_id: HashMap<EntityId, Entry>,
    /// Objects whose box is not indexable. A query tests each of them with
    /// [`Bounds::intersects`], so they behave as they do in a plain scan.
    exceptional: Vec<EntityId>,
}

impl Space {
    /// Build from every object's box at once (bulk loading beats inserting
    /// one by one in both time and the quality of the tree).
    pub(crate) fn build(items: impl IntoIterator<Item = (EntityId, Bounds)>) -> Self {
        let mut entries = Vec::new();
        let mut exceptional = Vec::new();
        for (id, b) in items {
            if is_indexable(&b) {
                entries.push(Entry::new(id, &b));
            } else {
                exceptional.push(id);
            }
        }
        Self {
            by_id: entries.iter().map(|e| (e.id, *e)).collect(),
            tree: RTree::bulk_load(entries),
            exceptional,
        }
    }

    pub(crate) fn insert(&mut self, id: EntityId, b: &Bounds) {
        if is_indexable(b) {
            let entry = Entry::new(id, b);
            if let Some(old) = self.by_id.insert(id, entry) {
                // Already there (should not happen): keep one entry per id.
                self.tree.remove(&old);
            }
            self.tree.insert(entry);
        } else {
            self.exceptional.push(id);
        }
    }

    /// Take an id out. `false` when this space did not hold it.
    pub(crate) fn remove(&mut self, id: EntityId) -> bool {
        if let Some(entry) = self.by_id.remove(&id) {
            return self.tree.remove(&entry).is_some();
        }
        match self.exceptional.iter().position(|e| *e == id) {
            Some(i) => {
                self.exceptional.swap_remove(i);
                true
            }
            None => false,
        }
    }

    /// Ids whose tree box touches `rect` (edges and corners count), in no
    /// particular order. `rect` must be indexable.
    pub(crate) fn query(&self, rect: &Bounds, out: &mut Vec<EntityId>) {
        let envelope = AABB::from_corners([rect.min.x, rect.min.y], [rect.max.x, rect.max.y]);
        out.extend(
            self.tree
                .locate_in_envelope_intersecting(&envelope)
                .map(|e| e.id),
        );
    }

    pub(crate) fn exceptional(&self) -> &[EntityId] {
        &self.exceptional
    }

    /// The box around everything in the tree; `None` when it is empty.
    pub(crate) fn root(&self) -> Option<Bounds> {
        if self.tree.size() == 0 {
            return None;
        }
        let root = self.tree.root().envelope();
        let (lo, hi) = (root.lower(), root.upper());
        Some(Bounds {
            min: Point::new(lo[0], lo[1]),
            max: Point::new(hi[0], hi[1]),
        })
    }

    #[cfg(test)]
    pub(crate) fn tree_len(&self) -> usize {
        self.tree.size()
    }

    /// Every id held, tree and exceptional (for the invariant check).
    #[cfg(test)]
    pub(crate) fn ids(&self) -> Vec<EntityId> {
        let mut ids: Vec<EntityId> = self.tree.iter().map(|e| e.id).collect();
        ids.extend(&self.exceptional);
        ids
    }

    /// The box stored for an id in the tree.
    #[cfg(test)]
    pub(crate) fn stored(&self, id: EntityId) -> Option<Bounds> {
        let e = self.by_id.get(&id)?;
        let (lo, hi) = (e.envelope.lower(), e.envelope.upper());
        Some(Bounds {
            min: Point::new(lo[0], lo[1]),
            max: Point::new(hi[0], hi[1]),
        })
    }

    /// Whether the map and the tree hold the same entries.
    #[cfg(test)]
    pub(crate) fn agrees_with_map(&self) -> bool {
        self.tree.size() == self.by_id.len()
            && self
                .tree
                .iter()
                .all(|e| self.by_id.get(&e.id).is_some_and(|m| m == e))
    }

    /// Lose an entry from the map only, leaving the tree stale (tests of the
    /// failure path).
    #[cfg(test)]
    pub(crate) fn corrupt_drop_from_tree(&mut self, id: EntityId) {
        if let Some(entry) = self.by_id.get(&id).copied() {
            self.tree.remove(&entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(x0: f64, y0: f64, x1: f64, y1: f64) -> Bounds {
        Bounds {
            min: Point::new(x0, y0),
            max: Point::new(x1, y1),
        }
    }

    fn hits(space: &Space, rect: Bounds) -> Vec<u64> {
        let mut found = Vec::new();
        space.query(&rect, &mut found);
        let mut ids: Vec<u64> = found.iter().map(|i| i.0).collect();
        ids.sort_unstable();
        ids
    }

    #[test]
    fn indexable_means_finite_and_ordered() {
        assert!(is_indexable(&b(0.0, 0.0, 0.0, 0.0)));
        assert!(!is_indexable(&b(f64::NAN, 0.0, 1.0, 1.0)));
        assert!(!is_indexable(&b(0.0, 0.0, f64::INFINITY, 1.0)));
        assert!(!is_indexable(&b(2.0, 0.0, 1.0, 1.0)), "reversed");
    }

    #[test]
    fn touching_edges_and_corners_count_as_contact() {
        let space = Space::build([
            (EntityId(1), b(0.0, 0.0, 1.0, 1.0)),
            (EntityId(2), b(5.0, 5.0, 5.0, 5.0)), // a point
        ]);
        assert_eq!(hits(&space, b(1.0, 0.5, 2.0, 0.6)), [1], "edge");
        assert_eq!(hits(&space, b(1.0, 1.0, 2.0, 2.0)), [1], "corner");
        assert_eq!(hits(&space, b(5.0, 5.0, 6.0, 6.0)), [2], "a zero-area box");
        assert_eq!(hits(&space, b(1.1, 0.0, 2.0, 1.0)), Vec::<u64>::new());
    }

    #[test]
    fn unindexable_boxes_wait_in_the_list_and_come_out_again() {
        let mut space = Space::build([(EntityId(1), b(0.0, 0.0, 1.0, 1.0))]);
        space.insert(EntityId(2), &b(f64::NAN, 0.0, 1.0, 1.0));
        space.insert(EntityId(3), &b(3.0, 3.0, 4.0, 4.0));
        assert_eq!(space.exceptional(), [EntityId(2)]);
        assert_eq!(space.tree_len(), 2);
        assert!(space.remove(EntityId(2)) && space.exceptional().is_empty());
        assert!(!space.remove(EntityId(2)), "already gone");
        assert!(space.remove(EntityId(3)) && !space.remove(EntityId(3)));
        assert_eq!(space.root(), Some(b(0.0, 0.0, 1.0, 1.0)));
        assert!(space.remove(EntityId(1)));
        assert_eq!(space.root(), None, "an empty tree has no root");
    }

    #[test]
    fn inserting_an_id_twice_keeps_one_entry() {
        let mut space = Space::default();
        space.insert(EntityId(1), &b(0.0, 0.0, 1.0, 1.0));
        space.insert(EntityId(1), &b(10.0, 10.0, 11.0, 11.0));
        assert_eq!(space.tree_len(), 1);
        assert_eq!(hits(&space, b(0.0, 0.0, 1.0, 1.0)), Vec::<u64>::new());
        assert_eq!(hits(&space, b(10.0, 10.0, 11.0, 11.0)), [1]);
        assert!(space.agrees_with_map());
    }
}
