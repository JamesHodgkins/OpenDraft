//! Keeping an associative hatch in step with the objects it was made from.
//!
//! The hatch's loops remember their source objects. After an edit, `follow`
//! works out the loops those objects make now:
//!
//! - the hatch was not itself edited: it takes the new loops (the boundary
//!   moved, so the hatch moves with it), or is detached when the objects no
//!   longer close;
//! - the hatch was edited too: if its loops still are the ones the objects
//!   make, nothing changes (hatch and boundary were moved together); if not,
//!   the hatch alone was changed and it is detached, keeping its shape.
//!
//! A loop is rebuilt by joining its sources end to end; the outer loop of a
//! hatch made from a picked point can also be traced again from that point
//! when its sources overshoot each other (lines through a corner).

use super::build::loops_from_entities;
use super::trace::trace;
use super::{BoundarySource, HatchEntity, HatchLoop, HatchLoopFlags};
use crate::{Bounds, Entity, EntityId};

/// What an edit does to an associative hatch.
#[derive(Clone, Debug, PartialEq)]
pub enum Follow {
    /// Nothing to change.
    Same,
    /// The hatch with the loops its objects make now.
    Replaced(HatchEntity),
    /// The hatch, no longer associative.
    Detached(HatchEntity),
}

impl HatchEntity {
    /// The hatch kept as it is but no longer following any objects.
    pub fn detached(&self) -> HatchEntity {
        let mut h = self.clone();
        h.associative = false;
        for l in &mut h.loops {
            l.sources.clear();
        }
        h
    }

    /// The ids of every object a loop was made from.
    pub fn source_ids(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.loops
            .iter()
            .flat_map(|l| l.sources.iter())
            .filter_map(|s| match s {
                BoundarySource::Entity(id) => Some(*id),
                BoundarySource::External { .. } => None,
            })
    }
}

/// The loops the sources of `h` make now, or `None` if any is missing or
/// they do not close.
fn fresh_loops(
    h: &HatchEntity,
    lookup: &dyn Fn(EntityId) -> Option<Entity>,
) -> Option<Vec<HatchLoop>> {
    let mut out = Vec::with_capacity(h.loops.len());
    for (i, old) in h.loops.iter().enumerate() {
        if old.sources.is_empty() {
            return None;
        }
        let mut objects = Vec::new();
        for s in &old.sources {
            let BoundarySource::Entity(id) = s else {
                return None;
            };
            let e = lookup(*id)?;
            if matches!(e, Entity::Hatch(_)) {
                return None;
            }
            objects.push(e);
        }
        let size = objects
            .iter()
            .map(Entity::bounds)
            .reduce(Bounds::union)
            .map_or(1.0, |b| (b.max.x - b.min.x).hypot(b.max.y - b.min.y));
        let joined = match loops_from_entities(&objects, 1e-6) {
            Ok((mut loops, _)) if loops.len() == 1 => loops.pop(),
            _ => None,
        };
        let loop_ = match (joined, h.seed_points.first(), i) {
            (Some(l), ..) => l,
            (None, Some(seed), 0) if old.flags.contains(HatchLoopFlags::DERIVED) => {
                let traced = trace(&objects, *seed, (size * 1e-5).max(1e-9)).ok()?;
                traced.loops.into_iter().next()?
            }
            _ => return None,
        };
        out.push(HatchLoop {
            flags: loop_.flags.with(HatchLoopFlags::EXTERNAL, true).with(
                HatchLoopFlags::DERIVED,
                old.flags.contains(HatchLoopFlags::DERIVED),
            ),
            boundary: loop_.boundary,
            sources: old.sources.clone(),
        });
    }
    Some(out)
}

/// Whether two sets of loops enclose the same shapes (same count, areas and
/// extents, within rounding).
fn same_shapes(a: &HatchEntity, b: &HatchEntity) -> bool {
    let size = a
        .bounds()
        .map_or(1.0, |b| (b.max.x - b.min.x).hypot(b.max.y - b.min.y))
        .max(1e-9);
    // Fine enough that following a curve as arcs or as bulges agree.
    let tol = size * 1e-7;
    let (Ok(ca), Ok(cb)) = (a.contours(tol), b.contours(tol)) else {
        return false;
    };
    if ca.len() != cb.len() {
        return false;
    }
    let near = |x: f64, y: f64| (x - y).abs() <= size * 2e-6 + 1e-9;
    ca.iter().zip(&cb).all(|(p, q)| {
        let (ba, bb) = (bounds_of(p), bounds_of(q));
        near(ba.min.x, bb.min.x)
            && near(ba.min.y, bb.min.y)
            && near(ba.max.x, bb.max.x)
            && near(ba.max.y, bb.max.y)
            && (super::contour_area(p) - super::contour_area(q)).abs() <= size * size * 1e-5 + 1e-12
    })
}

fn bounds_of(points: &[crate::Point]) -> Bounds {
    let first = points[0];
    points
        .iter()
        .fold(Bounds::from_points(first, first), |b, p| {
            b.union(Bounds::from_points(*p, *p))
        })
}

/// What an edit does to the associative hatch `h`. `edited` is whether the
/// hatch itself was changed by the edit; `lookup` finds the objects now.
pub fn follow(
    h: &HatchEntity,
    lookup: &dyn Fn(EntityId) -> Option<Entity>,
    edited: bool,
) -> Follow {
    if !h.associative {
        return Follow::Same;
    }
    let Some(loops) = fresh_loops(h, lookup) else {
        return Follow::Detached(h.detached());
    };
    let rebuilt = HatchEntity { loops, ..h.clone() };
    if same_shapes(h, &rebuilt) {
        return Follow::Same;
    }
    if edited {
        Follow::Detached(h.detached())
    } else {
        Follow::Replaced(rebuilt)
    }
}
