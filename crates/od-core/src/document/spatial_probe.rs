//! Ways into the spatial index for the tests that check it.

use std::collections::{HashMap, HashSet};

use super::spatial::{Config, SpaceKey};
use super::spatial_query::Broad;
use super::*;
use crate::spatial_index::is_indexable;

/// What the tests need to see inside the index.
impl Document {
    /// Whether the rectangle query finds the same as a scan, for tests.
    pub(super) fn scan_ids(&self, space: SpaceKey, rect: &Bounds) -> Vec<EntityId> {
        self.objects
            .iter()
            .filter(|o| o.layout == space && o.bounds().intersects(rect))
            .map(|o| o.id)
            .collect()
    }

    pub(super) fn set_index_config(&mut self, config: Config) {
        self.spatial.config = config;
        let spaces: HashSet<SpaceKey> = self.objects.iter().map(|o| o.layout).collect();
        self.spatial.slots.clear();
        self.spatial.rebuild_spaces(&spaces, &self.objects);
    }

    /// Panic unless the index agrees with the objects.
    pub(super) fn check_spatial_index(&self) {
        let mut counts: HashMap<SpaceKey, usize> = HashMap::new();
        for o in &self.objects {
            *counts.entry(o.layout).or_default() += 1;
        }
        assert_eq!(self.spatial.slots.len(), counts.len(), "spaces");
        for (space, slot) in &self.spatial.slots {
            assert_eq!(counts.get(space), Some(&slot.len), "len of {space:?}");
            let Some(tree) = &slot.tree else { continue };
            assert!(tree.agrees_with_map(), "map and tree of {space:?}");
            let mut ids = tree.ids();
            ids.sort_unstable();
            let before = ids.len();
            ids.dedup();
            assert_eq!(before, ids.len(), "an id twice in {space:?}");
            assert_eq!(ids.len(), slot.len, "entries of {space:?}");
            let mut root: Option<Bounds> = None;
            for id in ids {
                let object = self.object(id).expect("an entry names a live object");
                assert_eq!(object.layout, *space);
                let b = object.bounds();
                if is_indexable(&b) {
                    assert_eq!(tree.stored(id), Some(b), "box of {id:?}");
                    root = Some(root.map_or(b, |r| r.union(b)));
                } else {
                    assert!(tree.exceptional().contains(&id), "{id:?} is exceptional");
                }
            }
            assert_eq!(tree.root(), root, "root of {space:?}");
        }
    }

    pub(super) fn indexed_spaces(&self) -> usize {
        self.spatial
            .slots
            .values()
            .filter(|s| s.tree.is_some())
            .count()
    }

    pub(super) fn corrupt_index(&mut self, space: SpaceKey, id: EntityId) {
        if let Some(tree) = self
            .spatial
            .slots
            .get_mut(&space)
            .and_then(|s| s.tree.as_mut())
        {
            tree.corrupt_drop_from_tree(id);
        }
    }

    pub(super) fn broad(&self, space: SpaceKey, rect: &Bounds) -> Broad<'_> {
        self.tree_candidates(space, rect)
    }
}
