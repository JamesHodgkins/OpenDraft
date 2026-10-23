//! Following a spline with straight pieces. Derived and bounded: a call does
//! at most a fixed amount of work and says so rather than returning
//! something less accurate than asked.
//!
//! Each span is a rational Bézier piece, which lies inside the hull of its
//! control points because the weights are positive. A piece whose control
//! points all lie within the tolerance of its chord is therefore within the
//! tolerance of that chord, and is drawn as it; any other is cut in half and
//! tried again, the first half first.
//!
//! A run of spans whose control points all fit in a box no wider than the
//! tolerance lies in that box, so it is drawn as one chord between its
//! ends without being cut into pieces at all: a long spline seen from afar
//! costs little more than its control points.

use super::eval::{halve, local, H};
use super::{epsilon, Spline, SplineError};
use crate::{Bounds, Point};

/// Most points one flattening for the screen may give.
pub const MAX_POINTS: usize = 8192;
/// Most points one flattening for a plot may give.
pub const MAX_PLOT_POINTS: usize = 65_536;
/// How many times a piece may be halved.
const MAX_DEPTH: u32 = 32;
/// Points of the coarse outline used when the tolerance cannot be met.
const COARSE_POINTS: usize = 2048;

/// Whether every control point of a piece lies within `tolerance` of the
/// chord between its ends.
fn flat(c: &[H], tolerance: f64) -> bool {
    if !c.iter().all(|h| h[2] > 0.0) {
        return false;
    }
    let (Some(first), Some(last)) = (c.first(), c.last()) else {
        return true;
    };
    let (a, b) = (local(*first), local(*last));
    c[1..c.len() - 1]
        .iter()
        .all(|h| to_segment(local(*h), a, b) <= tolerance)
}

fn diagonal(b: &Bounds) -> f64 {
    (b.max.x - b.min.x).hypot(b.max.y - b.min.y)
}

/// Adds a point unless it repeats the last one; `ResourceLimit` past `max`.
fn emit(
    out: &mut Vec<(f64, Point)>,
    at: (f64, Point),
    near: f64,
    max: usize,
) -> Result<(), SplineError> {
    let last = out[out.len() - 1].1;
    if (at.1.x - last.x).hypot(at.1.y - last.y) > near {
        if out.len() >= max {
            return Err(SplineError::ResourceLimit);
        }
        out.push(at);
    }
    Ok(())
}

/// Distance from `p` to the segment from `a` to `b`.
pub(super) fn to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy)
}

impl Spline {
    /// Points along the curve from start to end (a closed spline ends where
    /// it began) such that no straight piece between two of them strays more
    /// than `tolerance` from the curve. A tolerance that is not a positive
    /// number means the smallest the coordinates can tell. `ResourceLimit`
    /// past [`MAX_POINTS`] points.
    pub fn flatten(&self, tolerance: f64) -> Result<Vec<Point>, SplineError> {
        self.flatten_within(tolerance, MAX_POINTS)
    }

    /// [`flatten`](Self::flatten) with another point limit (plots allow
    /// [`MAX_PLOT_POINTS`]).
    pub fn flatten_within(
        &self,
        tolerance: f64,
        max_points: usize,
    ) -> Result<Vec<Point>, SplineError> {
        Ok(self
            .flatten_params(tolerance, max_points)?
            .into_iter()
            .map(|(_, p)| p)
            .collect())
    }

    /// The flattening with the parameter of each point.
    pub(super) fn flatten_params(
        &self,
        tolerance: f64,
        max_points: usize,
    ) -> Result<Vec<(f64, Point)>, SplineError> {
        if !self.well_formed() {
            return Err(SplineError::WrongKnotCount);
        }
        let o = self.origin();
        let near = epsilon(self.scale());
        let tolerance = if tolerance.is_finite() && tolerance > 0.0 {
            tolerance
        } else {
            near
        };
        let (start, _) = self.domain();
        let mut out = vec![(start, self.start_point())];
        let n = usize::from(self.degree) + 1;
        let (mut scratch, mut left, mut right) = (vec![[0.0; 3]; n], Vec::new(), Vec::new());
        let mut arena: Vec<H> = Vec::with_capacity(n * (MAX_DEPTH as usize + 2));
        let mut stack = Vec::with_capacity(MAX_DEPTH as usize + 2);
        let count = self.control_points.len();
        let mut k = usize::from(self.degree);
        while k < count {
            let (u0, u1) = (self.knots[k], self.knots[k + 1]);
            if u1 <= u0 {
                k += 1;
                continue;
            }
            if let Some(last) = self.tiny_run(k, tolerance) {
                let u = self.knots[last + 1];
                emit(&mut out, (u, self.point_at(u)), near, max_points)?;
                k = last + 1;
                continue;
            }
            // Pieces waiting, last first; their points sit one after another
            // in `arena`, so the piece on top is at its end.
            self.bezier_into(k, &mut scratch, &mut arena);
            stack.push((0.0, 1.0, 0u32));
            while let Some((t0, t1, depth)) = stack.pop() {
                let base = arena.len() - n;
                if flat(&arena[base..], tolerance) {
                    let q = local(arena[arena.len() - 1]);
                    let q = Point::new(o.x + q.x, o.y + q.y);
                    arena.truncate(base);
                    emit(&mut out, (u0 + (u1 - u0) * t1, q), near, max_points)?;
                    continue;
                }
                if depth >= MAX_DEPTH {
                    return Err(SplineError::ResourceLimit);
                }
                halve(&mut arena[base..], &mut left, &mut right);
                arena.truncate(base);
                arena.extend_from_slice(&right);
                arena.extend_from_slice(&left);
                let mid = (t0 + t1) / 2.0;
                stack.push((mid, t1, depth + 1));
                stack.push((t0, mid, depth + 1));
            }
            k += 1;
        }
        if out.len() < 2 {
            let (_, end) = self.domain();
            out.push((end, self.end_point()));
        }
        Ok(out)
    }

    /// The last span of the run from span `k` whose control points fit in a
    /// box no wider than `tolerance`, or `None` when span `k`'s alone do not.
    fn tiny_run(&self, k: usize, tolerance: f64) -> Option<usize> {
        let p = usize::from(self.degree);
        let points = &self.control_points;
        let first = points[k - p];
        let mut b = points[k - p..=k]
            .iter()
            .fold(Bounds::from_points(first, first), |b, q| {
                b.union(Bounds::from_points(*q, *q))
            });
        if diagonal(&b) > tolerance {
            return None;
        }
        let mut last = k;
        while let Some(q) = points.get(last + 1) {
            let grown = b.union(Bounds::from_points(*q, *q));
            if diagonal(&grown) > tolerance {
                break;
            }
            b = grown;
            last += 1;
        }
        Some(last)
    }

    /// Like [`flatten`](Self::flatten), but never fails: when the tolerance
    /// cannot be met it gives an outline of points evenly spaced in the
    /// parameter. For the screen, where the next frame tries again; plots
    /// use `flatten` and report the failure.
    pub fn outline(&self, tolerance: f64) -> Vec<Point> {
        self.flatten(tolerance)
            .unwrap_or_else(|_| self.coarse().into_iter().map(|(_, p)| p).collect())
    }

    /// Points evenly spaced in the parameter, with their parameters.
    pub(super) fn coarse(&self) -> Vec<(f64, Point)> {
        let (a, b) = self.domain();
        (0..=COARSE_POINTS)
            .map(|i| {
                let u = a + (b - a) * i as f64 / COARSE_POINTS as f64;
                (u, self.point_at(u))
            })
            .collect()
    }
}
