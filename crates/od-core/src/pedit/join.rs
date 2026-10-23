//! PEDIT's Join: chaining objects whose ends meet, or nearly meet, onto an
//! open polyline.

use super::{distance, PolyData};
use crate::{bulge, Point};

/// The most objects PEDIT's Join takes in one go.
pub const MAX_JOIN_CANDIDATES: usize = 10_000;

/// What Join does with ends that do not quite meet (within the fuzz
/// distance), as in AutoCAD's Jointype option.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JoinType {
    /// Extend or trim the two end segments to where their lines cross.
    /// Only straight end segments can be extended.
    #[default]
    Extend,
    /// Bridge the gap with a straight segment.
    Add,
    /// Extend where that can be done, else add a segment.
    Both,
}

/// For each way two ends can meet: whether it is the chain's tail, and
/// whether it is the candidate's tail, that touch.
const WAYS: [(bool, bool); 4] = [(true, false), (true, true), (false, true), (false, false)];

/// The tolerance below which two ends are one point: grows with the size of
/// the coordinates so far-from-origin drawings still join.
fn point_epsilon(shapes: &[&PolyData]) -> f64 {
    let scale = shapes
        .iter()
        .flat_map(|s| s.points.iter())
        .fold(1.0_f64, |m, p| m.max(p.x.abs()).max(p.y.abs()));
    (32.0 * f64::EPSILON * scale).max(1e-9)
}

/// Where the lines of two straight end segments cross, if that is beyond
/// the inner end of each and within `reach` of both ends. Each segment is
/// `(inner point, end point)`.
fn meet(chain: (Point, Point), other: (Point, Point), reach: f64) -> Option<Point> {
    let (p, q) = chain;
    let (s, r) = other;
    let (d1, d2) = ((q.x - p.x, q.y - p.y), (s.x - r.x, s.y - r.y));
    let cross = |a: (f64, f64), b: (f64, f64)| a.0 * b.1 - a.1 * b.0;
    let den = cross(d1, d2);
    let (n1, n2) = (d1.0.hypot(d1.1), d2.0.hypot(d2.1));
    if n1 == 0.0 || n2 == 0.0 || den.abs() <= 1e-12 * n1 * n2 {
        return None;
    }
    let from = (r.x - p.x, r.y - p.y);
    let t = cross(from, d2) / den;
    let u = cross(from, d1) / den;
    let x = Point::new(p.x + t * d1.0, p.y + t * d1.1);
    let near = distance(x, q) <= reach && distance(x, r) <= reach;
    (t > 0.0 && u < 1.0 && near).then_some(x)
}

impl PolyData {
    fn end(&self, tail: bool) -> Point {
        self.points[if tail { self.points.len() - 1 } else { 0 }]
    }

    /// The last (or first) segment as `(inner point, end point)`, if it is
    /// straight.
    fn tip(&self, tail: bool) -> Option<(Point, Point)> {
        let n = self.points.len();
        if tail {
            (bulge::at(&self.bulges, n - 2) == 0.0)
                .then(|| (self.points[n - 2], self.points[n - 1]))
        } else {
            (bulge::at(&self.bulges, 0) == 0.0).then(|| (self.points[1], self.points[0]))
        }
    }

    fn set_end(&mut self, tail: bool, at: Point) {
        let last = self.points.len() - 1;
        self.points[if tail { last } else { 0 }] = at;
    }

    /// `piece` added on after the last point. Ends closer than `epsilon` are
    /// one point; further apart, a straight segment crosses the gap.
    fn append(&mut self, piece: &PolyData, epsilon: f64) {
        let mut bulges = self.full_bulges();
        let mut widths = self.full_widths();
        let (more_b, more_w) = (piece.full_bulges(), piece.full_widths());
        let last = self.points.len() - 1;
        let bridged = distance(self.points[last], piece.points[0]) <= epsilon;
        if bridged {
            bulges[last] = more_b[0];
            widths[last] = more_w[0];
            self.points.extend_from_slice(&piece.points[1..]);
            bulges.extend_from_slice(&more_b[1..]);
            widths.extend_from_slice(&more_w[1..]);
        } else {
            self.points.extend_from_slice(&piece.points);
            bulges.extend_from_slice(&more_b);
            widths.extend_from_slice(&more_w);
        }
        self.bulges = bulges;
        self.widths = widths;
    }

    /// Whether `other`, meeting this at the given ends a distance `d` apart,
    /// can join: ends that touch always can; others as `kind` allows.
    fn can_join(
        &self,
        other: &PolyData,
        ways: (bool, bool),
        d: f64,
        limits: (f64, f64),
        kind: JoinType,
    ) -> bool {
        let (epsilon, reach) = limits;
        if d <= epsilon {
            return true;
        }
        if d > reach {
            return false;
        }
        kind != JoinType::Extend
            || self
                .tip(ways.0)
                .zip(other.tip(ways.1))
                .is_some_and(|(a, b)| meet(a, b, reach).is_some())
    }

    /// This open polyline with whatever of `others` has an end within `fuzz`
    /// of one of its ends, chained on. Ends that do not quite meet are
    /// dealt with as `kind` says. Also the indexes of `others` used. `None`
    /// if this one is closed or nothing joined.
    pub fn joined(
        &self,
        others: &[PolyData],
        fuzz: f64,
        kind: JoinType,
    ) -> Option<(PolyData, Vec<usize>)> {
        if self.closed || others.len() > MAX_JOIN_CANDIDATES {
            return None;
        }
        let mut everything: Vec<&PolyData> = others.iter().collect();
        everything.push(self);
        let epsilon = point_epsilon(&everything);
        let limits = (epsilon, fuzz.max(epsilon));
        let mut chain = self.clone();
        let mut used: Vec<usize> = Vec::new();
        loop {
            // The nearest ends win; ties go to the tail of the chain, then
            // to the earlier candidate.
            let mut best: Option<(f64, usize, usize)> = None;
            for (i, other) in others.iter().enumerate() {
                if other.closed || used.contains(&i) {
                    continue;
                }
                for (way, &(chain_tail, other_tail)) in WAYS.iter().enumerate() {
                    let d = distance(chain.end(chain_tail), other.end(other_tail));
                    let fits = chain.can_join(other, (chain_tail, other_tail), d, limits, kind);
                    if fits && best.is_none_or(|(b, ..)| d < b) {
                        best = Some((d, i, way));
                    }
                }
            }
            let Some((d, i, way)) = best else { break };
            let (chain_tail, other_tail) = WAYS[way];
            let mut piece = others[i].clone();
            if d > epsilon && kind != JoinType::Add {
                let crossing = chain
                    .tip(chain_tail)
                    .zip(piece.tip(other_tail))
                    .and_then(|(a, b)| meet(a, b, limits.1));
                if let Some(x) = crossing {
                    chain.set_end(chain_tail, x);
                    piece.set_end(other_tail, x);
                }
            }
            match way {
                0 => chain.append(&piece, epsilon),
                1 => chain.append(&piece.reversed(), epsilon),
                _ => {
                    let mut front = if way == 2 { piece } else { piece.reversed() };
                    front.append(&chain, epsilon);
                    chain = front;
                }
            }
            used.push(i);
        }
        used.sort_unstable();
        (!used.is_empty()).then_some((chain, used))
    }
}
