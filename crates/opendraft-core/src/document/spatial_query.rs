//! Rectangle queries over the drawing, answered through the spatial index when
//! that pays and by a plain scan otherwise. Either way the answer is the
//! objects of the space whose box touches the rectangle, in draw order.

use std::sync::atomic::Ordering;

use super::spatial::{SpaceKey, SpatialStats};
use super::{Document, Object};
use crate::layer::LayerId;
use crate::layout::LayoutId;
use crate::spatial_index::is_indexable;
use crate::{Bounds, EntityId};

impl Document {
    /// Start putting in many objects (reading a file): the index is left
    /// alone and queries scan, until [`Document::finish_bulk_load`] builds it
    /// once. Cheaper than updating a tree for every object.
    pub fn begin_bulk_load(&mut self) {
        self.spatial.begin_bulk_load();
    }

    pub fn finish_bulk_load(&mut self) {
        self.spatial.finish_bulk_load(&self.objects);
    }

    /// Objects of a space whose box touches `rect` (edges count), in draw
    /// order, whatever their layer. The broad phase: the exact tests come
    /// after. Answers as a scan of the space would.
    pub fn objects_intersecting(&self, space: Option<LayoutId>, rect: Bounds) -> Vec<&Object> {
        match self.tree_candidates(space, &rect) {
            Broad::Found(found) => {
                self.verify(space, &rect, &found);
                found
            }
            Broad::Scan => self.scan(space, &rect),
            Broad::Stale => {
                debug_assert!(false, "the spatial index disagrees with the objects");
                self.scan(space, &rect)
            }
        }
    }

    /// The visible ones among them, leaving out layers frozen in the viewport
    /// being looked through (`frozen`).
    pub fn visible_in_rect(
        &self,
        space: Option<LayoutId>,
        frozen: &[LayerId],
        rect: Bounds,
    ) -> Vec<&Object> {
        let mut found = self.objects_intersecting(space, rect);
        found.retain(|o| self.is_visible(o) && !frozen.contains(&o.attrs.layer));
        found
    }

    /// The visible and unlocked ones among them.
    pub fn selectable_in_rect(
        &self,
        space: Option<LayoutId>,
        frozen: &[LayerId],
        rect: Bounds,
    ) -> Vec<&Object> {
        let mut found = self.objects_intersecting(space, rect);
        found.retain(|o| self.is_selectable(o) && !frozen.contains(&o.attrs.layer));
        found
    }

    /// [`Document::visible_in_rect`] for the space being edited.
    pub fn visible_here_in_rect(&self, rect: Bounds) -> Vec<&Object> {
        self.visible_in_rect(self.active_layout, &self.active_frozen, rect)
    }

    /// [`Document::selectable_in_rect`] for the space being edited.
    pub fn selectable_here_in_rect(&self, rect: Bounds) -> Vec<&Object> {
        self.selectable_in_rect(self.active_layout, &self.active_frozen, rect)
    }

    /// Check every query against a scan (debug builds only; otherwise one in
    /// a thousand is checked).
    pub fn set_verify_all_queries(&self, on: bool) {
        self.spatial.verify_all.store(on, Ordering::Relaxed);
    }

    pub fn spatial_stats(&self) -> SpatialStats {
        let s = &self.spatial;
        SpatialStats {
            tree_queries: s.tree_queries.load(Ordering::Relaxed),
            scans: s.scans.load(Ordering::Relaxed),
            visited: s.visited.load(Ordering::Relaxed),
            builds: s.builds.load(Ordering::Relaxed),
        }
    }

    /// The reference: look at every object of the space.
    fn scan(&self, space: SpaceKey, rect: &Bounds) -> Vec<&Object> {
        let s = &self.spatial;
        s.scans.fetch_add(1, Ordering::Relaxed);
        s.visited.fetch_add(self.objects.len(), Ordering::Relaxed);
        self.objects
            .iter()
            .filter(|o| o.layout == space && o.bounds().intersects(rect))
            .collect()
    }

    /// Candidates from the tree, in draw order, unless the plain scan should
    /// answer (no tree, an unusable rectangle, nothing in the tree, the
    /// rectangle covering all of it) or the index is found to disagree with
    /// the objects.
    pub(super) fn tree_candidates(&self, space: SpaceKey, rect: &Bounds) -> Broad<'_> {
        let Some(tree) = self.spatial.tree(space) else {
            return Broad::Scan;
        };
        let Some(root) = tree.root() else {
            return Broad::Scan;
        };
        if !is_indexable(rect) || covers(rect, &root) {
            return Broad::Scan;
        }
        let mut ids = Vec::new();
        tree.query(rect, &mut ids);
        let mut hits: Vec<(usize, &Object)> = Vec::with_capacity(ids.len());
        for id in ids {
            match self.resolve(id, space) {
                Some(hit) => hits.push(hit),
                None => return Broad::Stale,
            }
        }
        for &id in tree.exceptional() {
            let Some((at, object)) = self.resolve(id, space) else {
                return Broad::Stale;
            };
            if object.bounds().intersects(rect) {
                hits.push((at, object));
            }
        }
        let s = &self.spatial;
        s.tree_queries.fetch_add(1, Ordering::Relaxed);
        s.visited.fetch_add(hits.len(), Ordering::Relaxed);
        hits.sort_unstable_by_key(|(at, _)| *at);
        if hits.windows(2).any(|w| w[0].0 == w[1].0) {
            return Broad::Stale;
        }
        Broad::Found(hits.into_iter().map(|(_, o)| o).collect())
    }

    /// The object an index entry names, if it is still there and in `space`.
    fn resolve(&self, id: EntityId, space: SpaceKey) -> Option<(usize, &Object)> {
        let at = *self.positions.get(&id)?;
        let object = self.objects.get(at)?;
        (object.layout == space).then_some((at, object))
    }

    fn verify(&self, space: SpaceKey, rect: &Bounds, found: &[&Object]) {
        if !cfg!(debug_assertions) {
            return;
        }
        let every = self.spatial.verify_all.load(Ordering::Relaxed);
        let n = self.spatial.sampled.fetch_add(1, Ordering::Relaxed);
        if !every && !n.is_multiple_of(1024) {
            return;
        }
        let want: Vec<EntityId> = self
            .objects
            .iter()
            .filter(|o| o.layout == space && o.bounds().intersects(rect))
            .map(|o| o.id)
            .collect();
        let got: Vec<EntityId> = found.iter().map(|o| o.id).collect();
        assert_eq!(got, want, "spatial index differs from a scan for {rect:?}");
    }
}

/// What the tree made of a query.
pub(super) enum Broad<'a> {
    /// Not worth asking the tree: scan.
    Scan,
    Found(Vec<&'a Object>),
    /// The index and the objects disagree.
    Stale,
}

/// Whether `rect` wholly contains `inner`.
fn covers(rect: &Bounds, inner: &Bounds) -> bool {
    rect.min.x <= inner.min.x
        && rect.min.y <= inner.min.y
        && rect.max.x >= inner.max.x
        && rect.max.y >= inner.max.y
}
