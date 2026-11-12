//! Tidying one polyline: repeated vertices and straight runs.

use super::equal::near;
use crate::{bulge, Entity, Point};

/// A polyline vertex with the segment that leaves it.
struct Vertex {
    at: Point,
    bulge: f64,
    width: (f64, f64),
}

/// The polyline without repeated vertices or the middle vertices of straight
/// runs; `None` when there is nothing to take out or `entity` is not one.
pub(super) fn optimized(entity: &Entity, tol: f64) -> Option<Entity> {
    let Entity::Polyline {
        points,
        closed,
        bulges,
        widths,
    } = entity
    else {
        return None;
    };
    let n = points.len();
    let mut verts: Vec<Vertex> = (0..n)
        .map(|i| Vertex {
            at: points[i],
            bulge: bulge::at(bulges, i),
            width: widths.get(i).copied().unwrap_or((0.0, 0.0)),
        })
        .collect();
    let least = if *closed { 3 } else { 2 };
    let mut changed = false;
    let mut i = 0;
    while i < verts.len() && verts.len() > least {
        let len = verts.len();
        let next = (i + 1) % len;
        let prev = (i + len - 1) % len;
        let has_next = *closed || i + 1 < len;
        let has_prev = *closed || i > 0;
        let zero = has_next && near(verts[i].at, verts[next].at, tol);
        let straight = has_next
            && has_prev
            && widths.is_empty()
            && verts[prev].bulge == 0.0
            && verts[i].bulge == 0.0
            && on_the_way(verts[prev].at, verts[i].at, verts[next].at, tol);
        if zero || straight {
            verts.remove(i);
            changed = true;
            i = i.saturating_sub(1);
        } else {
            i += 1;
        }
    }
    changed.then(|| Entity::Polyline {
        points: verts.iter().map(|v| v.at).collect(),
        closed: *closed,
        bulges: if bulges.is_empty() {
            Vec::new()
        } else {
            verts.iter().map(|v| v.bulge).collect()
        },
        widths: if widths.is_empty() {
            Vec::new()
        } else {
            verts.iter().map(|v| v.width).collect()
        },
    })
}

/// Whether `b` lies on the straight way from `a` to `c`.
fn on_the_way(a: Point, b: Point, c: Point, tol: f64) -> bool {
    let (dx, dy) = (c.x - a.x, c.y - a.y);
    let len = dx.hypot(dy);
    if len <= tol {
        return false;
    }
    let off = ((b.x - a.x) * dy - (b.y - a.y) * dx).abs() / len;
    let along = ((b.x - a.x) * dx + (b.y - a.y) * dy) / len;
    off <= tol && along > 0.0 && along < len
}
