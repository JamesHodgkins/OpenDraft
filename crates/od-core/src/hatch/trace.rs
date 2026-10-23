//! Finding a hatch boundary from a point inside it (HATCH's Pick Internal
//! Point).
//!
//! The curves are flattened and cut at every crossing into a planar graph.
//! A ray from the point to the right finds the nearest edge; the face on
//! the point's side of it is walked edge by edge (at each node, the first
//! edge clockwise from the one just used keeps the face on the left). Dead
//! ends are dropped from the walk. Then the objects wholly inside that face
//! that touch nothing on its boundary are islands, each found by walking the
//! outside of its connected group.
//!
//! A loop that is one whole closed object (a circle, a rectangle...) comes
//! back as that object's exact loop; any other is a polygon of the flattened
//! pieces.

use std::collections::{HashMap, HashSet};

use super::graph::{build, cross, flatten, is_closed_shape, sub, Seg};

use super::topology::{contains, signed_area};
use super::{build::loops_from_entities, HatchLoop};
use crate::{Entity, Point};

/// Most segments the curves may flatten to.
const MAX_SEGMENTS: usize = 100_000;

/// Why no boundary was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceError {
    /// Nothing closed surrounds the point.
    NoBoundary,
    /// More geometry than can be followed.
    TooLarge,
}

impl std::fmt::Display for TraceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBoundary => write!(f, "valid hatch boundary not found"),
            Self::TooLarge => write!(f, "too much geometry to find a boundary in"),
        }
    }
}

/// The loops found, and the indices (into the objects given) of the objects
/// the outer loop was made from.
#[derive(Clone, Debug)]
pub struct Traced {
    pub loops: Vec<HatchLoop>,
    /// For each loop, the indices of the objects it was made from.
    pub loop_sources: Vec<Vec<usize>>,
    pub outer_sources: Vec<usize>,
}

fn shape_loop(entities: &[Entity], sources: &HashSet<usize>, polygon: &[Point]) -> HatchLoop {
    if let (1, Some(&only)) = (sources.len(), sources.iter().next()) {
        if is_closed_shape(&entities[only]) {
            if let Ok((mut loops, _)) = loops_from_entities(&entities[only..=only], 1e-9) {
                if let Some(l) = loops.pop() {
                    return l;
                }
            }
        }
    }
    HatchLoop {
        flags: super::HatchLoopFlags(
            super::HatchLoopFlags::DERIVED | super::HatchLoopFlags::POLYLINE,
        ),
        boundary: super::HatchBoundary::Polyline(super::HatchPolyline {
            vertices: polygon.to_vec(),
            bulges: Vec::new(),
            closed: true,
        }),
        sources: Vec::new(),
    }
}

/// The boundary around `seed` made by `entities`, curves followed within
/// `tolerance`, with the islands inside it.
pub fn trace(entities: &[Entity], seed: Point, tolerance: f64) -> Result<Traced, TraceError> {
    let mut segs: Vec<Seg> = Vec::new();
    for (src, e) in entities.iter().enumerate() {
        for (points, closed) in flatten(e, tolerance) {
            let n = points.len();
            for i in 0..n.saturating_sub(1) + usize::from(closed && n > 2) {
                let (a, b) = (points[i], points[(i + 1) % n]);
                if (a.x - b.x).hypot(a.y - b.y) > tolerance * 1e-3 {
                    segs.push(Seg { a, b, src });
                }
            }
        }
        if segs.len() > MAX_SEGMENTS {
            return Err(TraceError::TooLarge);
        }
    }
    let size = entities
        .iter()
        .map(|e| e.bounds())
        .filter(crate::Bounds::is_finite)
        .reduce(|a, b| a.union(b))
        .map_or(1.0, |b| (b.max.x - b.min.x).hypot(b.max.y - b.min.y))
        .max(1e-9);
    let eps = (size * 1e-9).max(tolerance * 1e-3);
    let g = build(&segs, eps);
    // The nearest edge to the right of the seed. If it belongs to an island
    // (the walk comes back inside out), that whole group is skipped and the
    // ray goes on to what surrounds it.
    let mut skipped: HashSet<usize> = HashSet::new();
    let (ring, outer) = loop {
        let mut best: Option<(f64, usize, usize)> = None;
        for &(u, v) in g.source.keys() {
            if skipped.contains(&u) {
                continue;
            }
            let (a, b) = (g.nodes[u], g.nodes[v]);
            if (a.y > seed.y) != (b.y > seed.y) {
                let x = a.x + (seed.y - a.y) / (b.y - a.y) * (b.x - a.x);
                if x > seed.x && best.is_none_or(|(bx, ..)| x < bx) {
                    best = Some((x, u, v));
                }
            }
        }
        let (_, u, v) = best.ok_or(TraceError::NoBoundary)?;
        let (a, b) = (g.nodes[u], g.nodes[v]);
        let (from, to) = if cross(sub(b, a), sub(seed, a)) > 0.0 {
            (u, v)
        } else {
            (v, u)
        };
        let ring = g.walk(from, to).ok_or(TraceError::NoBoundary)?;
        let polygon = g.polygon(&ring);
        if polygon.len() >= 3 && signed_area(&polygon) > eps * eps && contains(&polygon, seed) {
            break (ring, polygon);
        }
        skipped.extend(g.component(u));
    };
    let outer_sources: HashSet<usize> = ring
        .iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(&a, &b)| g.src(a, b))
        .collect();
    let on_outer: HashSet<usize> = ring.iter().copied().collect();
    let sorted = |set: &HashSet<usize>| {
        let mut v: Vec<usize> = set.iter().copied().collect();
        v.sort_unstable();
        v
    };
    let mut loops = vec![shape_loop(entities, &outer_sources, &outer)];
    let mut loop_sources = vec![sorted(&outer_sources)];
    // Islands: groups of edges wholly inside that touch nothing of the outer.
    let inside: Vec<bool> = g
        .nodes
        .iter()
        .enumerate()
        .map(|(n, p)| !on_outer.contains(&n) && contains(&outer, *p))
        .collect();
    let mut group: Vec<usize> = (0..g.nodes.len()).collect();
    fn find(group: &mut [usize], mut x: usize) -> usize {
        while group[x] != x {
            group[x] = group[group[x]];
            x = group[x];
        }
        x
    }
    for &(u, v) in g.source.keys() {
        if inside[u] && inside[v] {
            let (ru, rv) = (find(&mut group, u), find(&mut group, v));
            group[ru] = rv;
        }
    }
    let mut members: HashMap<usize, Vec<usize>> = HashMap::new();
    for n in (0..g.nodes.len()).filter(|&n| inside[n] && !g.around[n].is_empty()) {
        let root = find(&mut group, n);
        members.entry(root).or_default().push(n);
    }
    let mut roots: Vec<usize> = members.keys().copied().collect();
    roots.sort_unstable();
    for root in roots {
        let nodes = &members[&root];
        // A group that touches the outer boundary is not an island.
        if nodes
            .iter()
            .any(|&n| g.around[n].iter().any(|m| !inside[*m]))
        {
            continue;
        }
        let Some(&first) = nodes.iter().min_by(|&&a, &&b| {
            g.nodes[a]
                .x
                .total_cmp(&g.nodes[b].x)
                .then(g.nodes[a].y.total_cmp(&g.nodes[b].y))
        }) else {
            continue;
        };
        // The wedge facing west is on the left of the last edge by angle.
        let Some(&next) = g.around[first].last() else {
            continue;
        };
        let Some(ring) = g.walk(first, next) else {
            continue;
        };
        let polygon = g.polygon(&ring);
        if polygon.len() < 3 || signed_area(&polygon).abs() <= eps * eps {
            continue;
        }
        let sources: HashSet<usize> = ring
            .iter()
            .zip(ring.iter().cycle().skip(1))
            .map(|(&a, &b)| g.src(a, b))
            .collect();
        loops.push(shape_loop(entities, &sources, &polygon));
        loop_sources.push(sorted(&sources));
    }
    let mut outer_sources: Vec<usize> = outer_sources.into_iter().collect();
    outer_sources.sort_unstable();
    Ok(Traced {
        loops,
        loop_sources,
        outer_sources,
    })
}
