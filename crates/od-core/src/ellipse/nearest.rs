//! The point of an ellipse nearest a given point.
//!
//! There is no closed form, so it is found with fixed work: the derivative
//! of the squared distance is sampled along the arc, at even steps and at
//! the ends of the axes (where a flat ellipse turns round, so no step
//! crosses from one long side to the other), and each step where it goes
//! from falling to rising holds a local minimum, found by bisection. The
//! arc's ends are candidates too.

use std::f64::consts::{FRAC_PI_2, TAU};

use super::{dot, left, vector, Ellipse};
use crate::Point;

/// Samples per full turn; an arc gets its share, but never fewer than
/// `MIN_SAMPLES`.
const SAMPLES: usize = 64;
const MIN_SAMPLES: usize = 8;
/// Bisection steps for one bracket, far past what a double can tell apart.
const STEPS: usize = 64;
/// At most this many local minima are refined (an ellipse has at most two).
const MAX_MINIMA: usize = 8;

/// The ellipse in its own axes: half-lengths and the query point there.
struct Local {
    a: f64,
    b: f64,
    x: f64,
    y: f64,
}

impl Local {
    fn squared(&self, t: f64) -> f64 {
        let (sin, cos) = t.sin_cos();
        let (dx, dy) = (self.a * cos - self.x, self.b * sin - self.y);
        dx * dx + dy * dy
    }

    /// Half the derivative of [`squared`](Self::squared).
    fn slope(&self, t: f64) -> f64 {
        let (sin, cos) = t.sin_cos();
        (self.b * self.b - self.a * self.a) * sin * cos + self.a * self.x * sin
            - self.b * self.y * cos
    }

    /// Where the slope crosses zero in `[lo, hi]`, given it is not positive
    /// at `lo` and not negative at `hi`.
    fn refine(&self, mut lo: f64, mut hi: f64) -> f64 {
        for _ in 0..STEPS {
            let mid = lo + (hi - lo) / 2.0;
            if mid <= lo || mid >= hi {
                break;
            }
            if self.slope(mid) < 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo + (hi - lo) / 2.0
    }
}

impl Ellipse {
    /// Where the distance is sampled: evenly along the arc, plus the ends of
    /// the axes it passes, in order. A full ellipse's last sample is a turn
    /// after its first.
    fn samples(&self) -> Vec<f64> {
        let sweep = self.sweep();
        let count = ((SAMPLES as f64 * sweep / TAU).ceil() as usize).max(MIN_SAMPLES);
        let mut ts: Vec<f64> = (0..=count)
            .map(|i| self.start + sweep * i as f64 / count as f64)
            .collect();
        let mut k = (self.start / FRAC_PI_2).floor() + 1.0;
        while k * FRAC_PI_2 < self.end {
            ts.push(k * FRAC_PI_2);
            k += 1.0;
        }
        ts.sort_by(f64::total_cmp);
        ts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        ts
    }

    /// The point of the curve nearest `p` and its parameter. Ties go to the
    /// lower parameter along the arc.
    pub fn nearest(&self, p: Point) -> (Point, f64) {
        let a = self.major_radius();
        let u = vector(self.major_axis.x / a, self.major_axis.y / a);
        let q = vector(p.x - self.center.x, p.y - self.center.y);
        let local = Local {
            a,
            b: a * self.radius_ratio,
            x: dot(q, u),
            y: dot(q, left(u)),
        };
        let ts = self.samples();
        let n = ts.len() - 1;
        let slopes: Vec<f64> = ts.iter().map(|t| local.slope(*t)).collect();

        // Candidates: the ends (the start, for a full ellipse), then each
        // local minimum.
        let mut best = (ts[0], local.squared(ts[0]));
        if !self.is_full() {
            let end = local.squared(ts[n]);
            if end < best.1 {
                best = (ts[n], end);
            }
        }
        let mut refined = 0;
        for i in 0..n {
            if !(slopes[i] <= 0.0 && slopes[i + 1] >= 0.0) || refined == MAX_MINIMA {
                continue;
            }
            refined += 1;
            let t = local.refine(ts[i], ts[i + 1]);
            let d = local.squared(t);
            if d < best.1 {
                best = (t, d);
            }
        }
        let t = best.0.rem_euclid(TAU);
        (self.point_at(t), t)
    }

    /// Distance from `p` to the curve.
    pub fn distance_to(&self, p: Point) -> f64 {
        let (q, _) = self.nearest(p);
        (q.x - p.x).hypot(q.y - p.y)
    }
}
