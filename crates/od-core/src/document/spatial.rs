//! The drawing's spatial index: when a space gets a tree and how edits keep it
//! current. The rectangle queries that use it are in `spatial_query`.
//!
//! One R-tree per space (model space and each sheet), built only once a space
//! holds enough objects to repay it and kept current by the same methods that
//! change the object list. It is derived state: it never affects equality,
//! files or undo. A query returns what a plain scan would, in draw order; the
//! tree only decides which objects need looking at (the broad phase).

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::Object;
use crate::layout::LayoutId;
use crate::spatial_index::Space;
use crate::EntityId;

pub(super) type SpaceKey = Option<LayoutId>;

/// When a space gets a tree and how an edit of many objects is handled.
#[derive(Clone, Copy, Debug)]
pub(super) struct Config {
    /// Objects in a space at which a tree is built.
    pub(super) activate: usize,
    /// Objects below which a tree is dropped again (less than `activate`, so
    /// edits near the limit do not build and discard it over and over).
    pub(super) deactivate: usize,
    /// An edit touching at least this many objects, and this fraction of the
    /// space, rebuilds the tree in one go instead of updating it entry by
    /// entry.
    pub(super) min_bulk: usize,
    pub(super) bulk_fraction: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            activate: 4096,
            deactivate: 3072,
            min_bulk: 256,
            bulk_fraction: 0.10,
        }
    }
}

/// What the index did, for tests and benchmarks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpatialStats {
    /// Queries answered through a tree.
    pub tree_queries: usize,
    /// Queries answered by scanning the space.
    pub scans: usize,
    /// Objects looked at to answer them: tree candidates and scanned objects.
    pub visited: usize,
    /// Trees built or rebuilt from scratch.
    pub builds: usize,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Slot {
    /// Objects in the space, tree or not.
    pub(super) len: usize,
    pub(super) tree: Option<Space>,
}

#[derive(Debug, Default)]
pub(super) struct SpatialIndex {
    pub(super) slots: HashMap<SpaceKey, Slot>,
    pub(super) config: Config,
    /// A bulk load is under way: nothing is tracked and every query scans.
    pub(super) deferred: bool,
    pub(super) verify_all: AtomicBool,
    pub(super) sampled: AtomicUsize,
    pub(super) tree_queries: AtomicUsize,
    pub(super) scans: AtomicUsize,
    pub(super) visited: AtomicUsize,
    pub(super) builds: AtomicUsize,
}

impl Clone for SpatialIndex {
    fn clone(&self) -> Self {
        Self {
            slots: self.slots.clone(),
            config: self.config,
            deferred: self.deferred,
            verify_all: AtomicBool::new(self.verify_all.load(Ordering::Relaxed)),
            ..Self::default()
        }
    }
}

/// The index says nothing about the drawing, so two drawings that differ only
/// in it are the same drawing.
impl PartialEq for SpatialIndex {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl SpatialIndex {
    pub(super) fn tree(&self, space: SpaceKey) -> Option<&Space> {
        if self.deferred {
            return None;
        }
        self.slots.get(&space)?.tree.as_ref()
    }

    fn is_bulk(&self, len: usize, affected: usize) -> bool {
        let share = (len as f64 * self.config.bulk_fraction).ceil() as usize;
        affected >= self.config.min_bulk.max(share)
    }

    /// Objects were put in (they are in `objects` already).
    pub(super) fn added(&mut self, new: &[&Object], objects: &[Object]) {
        if self.deferred || new.is_empty() {
            return;
        }
        let mut counts: HashMap<SpaceKey, usize> = HashMap::new();
        for object in new {
            *counts.entry(object.layout).or_default() += 1;
        }
        let mut rebuild = HashSet::new();
        for (&space, &count) in &counts {
            let (activate, bulk) = {
                let len = self.slots.entry(space).or_default().len + count;
                (len >= self.config.activate, self.is_bulk(len, count))
            };
            let slot = self.slots.entry(space).or_default();
            slot.len += count;
            if (slot.tree.is_none() && activate) || (slot.tree.is_some() && bulk) {
                rebuild.insert(space);
            }
        }
        for object in new {
            if rebuild.contains(&object.layout) {
                continue;
            }
            if let Some(tree) = self
                .slots
                .get_mut(&object.layout)
                .and_then(|s| s.tree.as_mut())
            {
                tree.insert(object.id, &object.bounds());
            }
        }
        self.rebuild_spaces(&rebuild, objects);
    }

    /// Objects were taken out (they are no longer in `objects`).
    pub(super) fn removed(&mut self, taken: &[&Object], objects: &[Object]) {
        if self.deferred || taken.is_empty() {
            return;
        }
        let mut counts: HashMap<SpaceKey, usize> = HashMap::new();
        for object in taken {
            *counts.entry(object.layout).or_default() += 1;
        }
        let mut rebuild = HashSet::new();
        for (&space, &count) in &counts {
            let len = self.slots.get(&space).map_or(0, |s| s.len);
            // More taken than counted means the counts are wrong: recount.
            if len < count || self.is_bulk(len, count) {
                rebuild.insert(space);
            }
            if let Some(slot) = self.slots.get_mut(&space) {
                slot.len = len.saturating_sub(count);
            }
        }
        for object in taken {
            if rebuild.contains(&object.layout) {
                continue;
            }
            let gone = self
                .slots
                .get_mut(&object.layout)
                .and_then(|s| s.tree.as_mut())
                .map(|tree| tree.remove(object.id));
            if gone == Some(false) {
                rebuild.insert(object.layout);
            }
        }
        for space in counts.keys().filter(|s| !rebuild.contains(s)) {
            let Some(slot) = self.slots.get_mut(space) else {
                continue;
            };
            // Too few left for a tree to pay, or none at all: forget it.
            if slot.len < self.config.deactivate {
                slot.tree = None;
            }
            if slot.len == 0 {
                self.slots.remove(space);
            }
        }
        self.rebuild_spaces(&rebuild, objects);
    }

    /// Before geometry changes: take the old entries out of the spaces that
    /// will be updated entry by entry. Returns the spaces to rebuild after.
    pub(super) fn begin_replace(&mut self, touched: &[(EntityId, SpaceKey)]) -> HashSet<SpaceKey> {
        let mut rebuild = HashSet::new();
        if self.deferred {
            return rebuild;
        }
        let mut counts: HashMap<SpaceKey, usize> = HashMap::new();
        for (_, space) in touched {
            *counts.entry(*space).or_default() += 1;
        }
        for (&space, &count) in &counts {
            let len = self.slots.get(&space).map_or(0, |s| s.len);
            if self.is_bulk(len, count) {
                rebuild.insert(space);
            }
        }
        for (id, space) in touched {
            if rebuild.contains(space) {
                continue;
            }
            let gone = self
                .slots
                .get_mut(space)
                .and_then(|s| s.tree.as_mut())
                .map(|tree| tree.remove(*id));
            if gone == Some(false) {
                rebuild.insert(*space);
            }
        }
        rebuild
    }

    /// After geometry changed: put the new entries in, or rebuild.
    pub(super) fn finish_replace(
        &mut self,
        touched: &[&Object],
        rebuild: HashSet<SpaceKey>,
        objects: &[Object],
    ) {
        if self.deferred {
            return;
        }
        for object in touched {
            if rebuild.contains(&object.layout) {
                continue;
            }
            if let Some(tree) = self
                .slots
                .get_mut(&object.layout)
                .and_then(|s| s.tree.as_mut())
            {
                tree.insert(object.id, &object.bounds());
            }
        }
        // A space whose old entries were already taken out must come back
        // whole, so a rebuild is also what repairs a failed removal.
        self.rebuild_spaces(&rebuild, objects);
    }

    /// Count and index these spaces again from the objects, in one pass.
    pub(super) fn rebuild_spaces(&mut self, spaces: &HashSet<SpaceKey>, objects: &[Object]) {
        if self.deferred || spaces.is_empty() {
            return;
        }
        let mut groups: HashMap<SpaceKey, Vec<&Object>> = HashMap::new();
        for object in objects {
            if spaces.contains(&object.layout) {
                groups.entry(object.layout).or_default().push(object);
            }
        }
        for &space in spaces {
            let members = groups.remove(&space).unwrap_or_default();
            let had_tree = self.slots.get(&space).is_some_and(|s| s.tree.is_some());
            let len = members.len();
            if len == 0 {
                self.slots.remove(&space);
                continue;
            }
            let wanted = len >= self.config.activate || (had_tree && len >= self.config.deactivate);
            let tree = wanted.then(|| {
                self.builds.fetch_add(1, Ordering::Relaxed);
                Space::build(members.iter().map(|o| (o.id, o.bounds())))
            });
            self.slots.insert(space, Slot { len, tree });
        }
    }

    pub(super) fn begin_bulk_load(&mut self) {
        self.deferred = true;
    }

    /// Index everything from the objects, once.
    pub(super) fn finish_bulk_load(&mut self, objects: &[Object]) {
        self.deferred = false;
        self.slots.clear();
        let spaces: HashSet<SpaceKey> = objects.iter().map(|o| o.layout).collect();
        self.rebuild_spaces(&spaces, objects);
    }
}
