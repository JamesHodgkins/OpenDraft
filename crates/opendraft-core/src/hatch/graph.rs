//! The planar graph behind boundary tracing: curves flattened to segments,
//! cut at every crossing, merged into nodes with edges around each.

use std::collections::{HashMap, HashSet};

use crate::{arc, bulge, Entity, Point};

pub(super) struct Seg {
    pub a: Point,
    pub b: Point,
    pub src: usize,
}

/// A closed object by itself.
pub(super) fn is_closed_shape(e: &Entity) -> bool {
    match e {
        Entity::Circle { .. } | Entity::Rectangle { .. } => true,
        Entity::Polyline { closed, .. } => *closed,
        Entity::Ellipse(el) => el.is_full(),
        Entity::Spline(s) => s.closed,
        _ => false,
    }
}

/// The object as polylines (points, closed).
pub(super) fn flatten(e: &Entity, tol: f64) -> Vec<(Vec<Point>, bool)> {
    match e {
        Entity::Line { start, end } => vec![(vec![*start, *end], false)],
        Entity::Circle { center, radius } => {
            let mut p = arc::flatten(*center, *radius, 0.0, std::f64::consts::TAU, tol);
            p.pop();
            vec![(p, true)]
        }
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => vec![(arc::flatten(*center, *radius, *start, *end, tol), false)],
        Entity::Ellipse(el) => {
            let mut p = el.outline(tol);
            if el.is_full() {
                p.pop();
            }
            vec![(p, el.is_full())]
        }
        Entity::Spline(s) => {
            let mut p = s.outline(tol);
            if s.closed {
                p.pop();
            }
            vec![(p, s.closed)]
        }
        Entity::Rectangle { min, max } => vec![(
            vec![
                *min,
                Point::new(max.x, min.y),
                *max,
                Point::new(min.x, max.y),
            ],
            true,
        )],
        Entity::Polyline {
            points,
            closed,
            bulges,
            ..
        } => {
            let mut p = bulge::flatten_path(points, bulges, *closed, tol);
            if *closed && p.len() > 1 {
                p.pop();
            }
            vec![(p, *closed)]
        }
        _ => Vec::new(),
    }
}

pub(super) fn cross(a: Point, b: Point) -> f64 {
    a.x * b.y - a.y * b.x
}

pub(super) fn sub(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}

/// Where segments `p..p+r` and `q..q+s` meet, as fractions along each
/// (two for an overlap), within `eps` of length.
pub(super) fn meet(p: Point, r: Point, q: Point, s: Point, eps: f64) -> Vec<(f64, f64)> {
    let (rl, sl) = (r.x.hypot(r.y), s.x.hypot(s.y));
    let d = cross(r, s);
    let qp = sub(q, p);
    let (te, ue) = (eps / rl, eps / sl);
    if d.abs() > 1e-12 * rl * sl {
        let t = cross(qp, s) / d;
        let u = cross(qp, r) / d;
        if (-te..=1.0 + te).contains(&t) && (-ue..=1.0 + ue).contains(&u) {
            return vec![(t.clamp(0.0, 1.0), u.clamp(0.0, 1.0))];
        }
        return Vec::new();
    }
    // Parallel: only a collinear overlap meets.
    if cross(qp, r).abs() > eps * rl {
        return Vec::new();
    }
    let along = |x: Point| ((x.x - p.x) * r.x + (x.y - p.y) * r.y) / (rl * rl);
    let back = |x: Point| ((x.x - q.x) * s.x + (x.y - q.y) * s.y) / (sl * sl);
    let (t0, t1) = (along(q), along(Point::new(q.x + s.x, q.y + s.y)));
    let (lo, hi) = (t0.min(t1).max(0.0), t0.max(t1).min(1.0));
    if hi < lo - te {
        return Vec::new();
    }
    let at = |t: f64| Point::new(p.x + r.x * t, p.y + r.y * t);
    vec![
        (lo, back(at(lo)).clamp(0.0, 1.0)),
        (hi, back(at(hi)).clamp(0.0, 1.0)),
    ]
}

pub(super) struct Graph {
    pub nodes: Vec<Point>,
    /// Neighbours of each node, counter-clockwise by angle.
    pub around: Vec<Vec<usize>>,
    pub source: HashMap<(usize, usize), usize>,
}

pub(super) fn build(segs: &[Seg], eps: f64) -> Graph {
    // Cut every segment where the others meet it.
    let mut cuts: Vec<Vec<f64>> = vec![vec![0.0, 1.0]; segs.len()];
    let mut order: Vec<usize> = (0..segs.len()).collect();
    let lo_x = |s: &Seg| s.a.x.min(s.b.x);
    let hi_x = |s: &Seg| s.a.x.max(s.b.x);
    order.sort_by(|&i, &j| lo_x(&segs[i]).total_cmp(&lo_x(&segs[j])));
    for (n, &i) in order.iter().enumerate() {
        let (si, ri) = (&segs[i], sub(segs[i].b, segs[i].a));
        for &j in &order[n + 1..] {
            let sj = &segs[j];
            if lo_x(sj) > hi_x(si) + eps {
                break;
            }
            if sj.a.y.min(sj.b.y) > si.a.y.max(si.b.y) + eps
                || sj.a.y.max(sj.b.y) < si.a.y.min(si.b.y) - eps
            {
                continue;
            }
            for (t, u) in meet(si.a, ri, sj.a, sub(sj.b, sj.a), eps) {
                cuts[i].push(t);
                cuts[j].push(u);
            }
        }
    }
    // Nodes, merging points closer than `eps`.
    let mut nodes: Vec<Point> = Vec::new();
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    let mut node_at = |p: Point| -> usize {
        let key = (
            (p.x / (4.0 * eps)).floor() as i64,
            (p.y / (4.0 * eps)).floor() as i64,
        );
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &n in grid.get(&(key.0 + dx, key.1 + dy)).into_iter().flatten() {
                    if (nodes[n].x - p.x).hypot(nodes[n].y - p.y) <= eps {
                        return n;
                    }
                }
            }
        }
        nodes.push(p);
        grid.entry(key).or_default().push(nodes.len() - 1);
        nodes.len() - 1
    };
    let mut edges: HashMap<(usize, usize), usize> = HashMap::new();
    for (i, s) in segs.iter().enumerate() {
        let mut t = std::mem::take(&mut cuts[i]);
        t.sort_by(f64::total_cmp);
        let at = |t: f64| Point::new(s.a.x + (s.b.x - s.a.x) * t, s.a.y + (s.b.y - s.a.y) * t);
        let mut last = node_at(s.a);
        for &x in t.iter().skip(1) {
            let n = node_at(if x >= 1.0 { s.b } else { at(x) });
            if n != last {
                edges.entry((last.min(n), last.max(n))).or_insert(s.src);
            }
            last = n;
        }
    }
    let mut around: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for &(u, v) in edges.keys() {
        around[u].push(v);
        around[v].push(u);
    }
    for (u, list) in around.iter_mut().enumerate() {
        let origin = nodes[u];
        list.sort_by(|&a, &b| {
            let ang = |n: usize| (nodes[n].y - origin.y).atan2(nodes[n].x - origin.x);
            ang(a).total_cmp(&ang(b))
        });
    }
    Graph {
        nodes,
        around,
        source: edges,
    }
}

impl Graph {
    pub(super) fn src(&self, a: usize, b: usize) -> usize {
        self.source.get(&(a.min(b), a.max(b))).copied().unwrap_or(0)
    }

    /// The face on the left of the half-edge `u -> v`, as nodes, with dead
    /// ends cancelled; `None` if the walk does not come back.
    pub(super) fn walk(&self, u: usize, v: usize) -> Option<Vec<usize>> {
        let limit = 2 * self.source.len() + 2;
        let (mut a, mut b) = (u, v);
        let mut path: Vec<usize> = Vec::new();
        for _ in 0..limit {
            path.push(a);
            let ring = &self.around[b];
            let at = ring.iter().position(|&n| n == a)?;
            let next = ring[(at + ring.len() - 1) % ring.len()];
            a = b;
            b = next;
            if (a, b) == (u, v) {
                return Some(cancel_spurs(path));
            }
        }
        None
    }

    /// Every node joined to `start` by edges.
    pub(super) fn component(&self, start: usize) -> HashSet<usize> {
        let mut seen: HashSet<usize> = HashSet::from([start]);
        let mut todo = vec![start];
        while let Some(n) = todo.pop() {
            for &m in &self.around[n] {
                if seen.insert(m) {
                    todo.push(m);
                }
            }
        }
        seen
    }

    pub(super) fn polygon(&self, ring: &[usize]) -> Vec<Point> {
        ring.iter().map(|&n| self.nodes[n]).collect()
    }
}

/// Removes out-and-back stretches (an edge walked both ways).
pub(super) fn cancel_spurs(path: Vec<usize>) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for n in path {
        if out.len() >= 2 && out[out.len() - 2] == n {
            out.pop();
        } else {
            out.push(n);
        }
    }
    while out.len() >= 3 && out[out.len() - 2] == out[0] {
        out.pop();
        out.remove(0);
    }
    out
}
