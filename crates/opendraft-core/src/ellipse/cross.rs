//! Where a whole ellipse meets a line, a circle or another whole ellipse.
//! The base for trimming and extending with ellipses.
//!
//! A line is met exactly: in the ellipse's own frame, stretched so the
//! ellipse is the unit circle, the line is still a line. A circle or an
//! ellipse is a level set `|w|² = k` of a point `w` that depends linearly on
//! position, so along this ellipse `f(t) = |w(t)|² - k` with
//! `w(t) = w0 + w1·cos t + w2·sin t`: a trigonometric polynomial of degree
//! two, with at most four roots. They are found with fixed work: sample `f`
//! and its slope round the turn, split at the slope's zeros so every piece
//! is monotone, and bisect each piece where `f` changes sign. A turning point
//! where the curves come within a length tolerance of each other (judged by
//! distance, `|f| / |∇f|`, not by `f`, which a thin ellipse scales up) is a
//! tangent point. Curves that lie on each other give no single crossing.

use std::f64::consts::TAU;

use super::{dot, left, vector, Ellipse};
use crate::intersect::{dedupe, line_circle};
use crate::Point;

/// Samples round the turn.
const SAMPLES: usize = 256;
/// Bisection steps, far past what a double can tell apart.
const STEPS: usize = 64;

/// `w(t) = w0 + w1·cos t + w2·sin t` and the level `k` it is compared with.
/// `w` is `M·p` plus a constant for the point `p`; `rows` are the rows of
/// `M`. `near` is how close (a length) counts as touching.
struct Level {
    w0: Point,
    w1: Point,
    w2: Point,
    k: f64,
    rows: [Point; 2],
    near: f64,
}

impl Level {
    fn w(&self, t: f64) -> Point {
        let (s, c) = t.sin_cos();
        vector(
            self.w0.x + self.w1.x * c + self.w2.x * s,
            self.w0.y + self.w1.y * c + self.w2.y * s,
        )
    }

    fn f(&self, t: f64) -> f64 {
        let w = self.w(t);
        dot(w, w) - self.k
    }

    /// Roughly how far the point at `t` is from the level set: `|f|` over
    /// the length of its gradient in the plane, `2·Mᵀw`.
    fn gap(&self, t: f64) -> f64 {
        let w = self.w(t);
        let r = self.rows;
        let grad = vector(r[0].x * w.x + r[1].x * w.y, r[0].y * w.x + r[1].y * w.y);
        let length = 2.0 * grad.x.hypot(grad.y);
        if length == 0.0 {
            return f64::INFINITY;
        }
        self.f(t).abs() / length
    }

    /// Half the slope of `f`.
    fn slope(&self, t: f64) -> f64 {
        let (s, c) = t.sin_cos();
        let dw = vector(
            -self.w1.x * s + self.w2.x * c,
            -self.w1.y * s + self.w2.y * c,
        );
        dot(self.w(t), dw)
    }

    /// Where `g` changes sign in `[lo, hi]`, given it does.
    fn bisect(g: impl Fn(f64) -> f64, mut lo: f64, mut hi: f64) -> f64 {
        let rising = g(lo) < g(hi);
        for _ in 0..STEPS {
            let mid = lo + (hi - lo) / 2.0;
            if mid <= lo || mid >= hi {
                break;
            }
            if (g(mid) < 0.0) == rising {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo + (hi - lo) / 2.0
    }

    /// The parameters in `[0, 2π)` where `f` is zero.
    fn roots(&self) -> Vec<f64> {
        let ts: Vec<f64> = (0..=SAMPLES)
            .map(|i| TAU * i as f64 / SAMPLES as f64)
            .collect();
        if ts.iter().all(|t| self.gap(*t) <= self.near) {
            return Vec::new(); // the curves lie on each other
        }
        let slopes: Vec<f64> = ts.iter().map(|t| self.slope(*t)).collect();
        // Every sample, plus every turning point (marked), in order: between
        // two of these `f` is monotone.
        let mut marks: Vec<(f64, bool)> = Vec::with_capacity(ts.len() + 8);
        for i in 0..SAMPLES {
            marks.push((ts[i], false));
            if (slopes[i] < 0.0) != (slopes[i + 1] < 0.0) {
                marks.push((Self::bisect(|t| self.slope(t), ts[i], ts[i + 1]), true));
            }
        }
        marks.push((TAU, false));
        let values: Vec<f64> = marks.iter().map(|(t, _)| self.f(*t)).collect();
        let mut found = Vec::new();
        for i in 0..marks.len() - 1 {
            let (a, b) = (values[i], values[i + 1]);
            let (t, turning) = marks[i];
            if a == 0.0 || (turning && self.gap(t) <= self.near) {
                // On the level exactly, or a turning point that touches it.
                found.push(t);
            } else if (a < 0.0) != (b < 0.0) && b != 0.0 {
                found.push(Self::bisect(|t| self.f(t), t, marks[i + 1].0));
            }
        }
        found.iter().map(|t| t.rem_euclid(TAU)).collect()
    }
}

impl Ellipse {
    /// `p` in the frame where this ellipse is the unit circle about the
    /// origin.
    fn unit_coords(&self, p: Point) -> Point {
        let a = self.major_radius();
        let u = vector(self.major_axis.x / a, self.major_axis.y / a);
        let q = vector(p.x - self.center.x, p.y - self.center.y);
        vector(dot(q, u) / a, dot(q, left(u)) / (a * self.radius_ratio))
    }

    /// The drawing point for unit coordinates: back from [`unit_coords`](Self::unit_coords).
    fn world_of(&self, w: Point) -> Point {
        let b = self.minor_axis();
        Point::new(
            self.center.x + self.major_axis.x * w.x + b.x * w.y,
            self.center.y + self.major_axis.y * w.x + b.y * w.y,
        )
    }

    /// Where the infinite line through `a` and `b` meets the whole ellipse:
    /// two points, one where it touches, or none.
    pub fn line_crossings(&self, a: Point, b: Point) -> Vec<Point> {
        let (ua, ub) = (self.unit_coords(a), self.unit_coords(b));
        if (ub.x - ua.x).hypot(ub.y - ua.y) == 0.0 {
            return Vec::new();
        }
        line_circle(ua, ub, Point::default(), 1.0)
            .into_iter()
            .map(|w| self.world_of(w))
            .collect()
    }

    /// The points of the whole ellipse where the level `|w|² = k` is met,
    /// `w` being `M·point + offset` for the matrix with `rows`.
    fn level_crossings(&self, rows: [Point; 2], offset: Point, k: f64) -> Vec<Point> {
        let map = |p: Point| vector(dot(rows[0], p), dot(rows[1], p));
        let c = map(self.center);
        let scale = self
            .center
            .x
            .abs()
            .max(self.center.y.abs())
            .max(self.major_radius());
        let level = Level {
            w0: vector(c.x + offset.x, c.y + offset.y),
            w1: map(self.major_axis),
            w2: map(self.minor_axis()),
            k,
            rows,
            near: 1e-9 * (1.0 + scale),
        };
        // A tangent point can be found both at a sample and at the turning
        // point beside it.
        dedupe(
            level
                .roots()
                .into_iter()
                .map(|t| self.point_at(t))
                .collect(),
        )
    }

    /// Where the whole circle meets the whole ellipse.
    pub fn circle_crossings(&self, center: Point, radius: f64) -> Vec<Point> {
        if !self.bounds().intersects(&circle_box(center, radius)) {
            return Vec::new();
        }
        let identity = [vector(1.0, 0.0), vector(0.0, 1.0)];
        self.level_crossings(identity, vector(-center.x, -center.y), radius * radius)
    }

    /// Where two whole ellipses meet.
    pub fn ellipse_crossings(&self, other: &Ellipse) -> Vec<Point> {
        if !self.bounds().intersects(&other.bounds()) {
            return Vec::new();
        }
        let a = other.major_radius();
        let u = vector(other.major_axis.x / a, other.major_axis.y / a);
        let (v, b) = (left(u), a * other.radius_ratio);
        // `other`'s unit coordinates are linear in the point.
        let rows = [vector(u.x / a, u.y / a), vector(v.x / b, v.y / b)];
        let shift = vector(dot(rows[0], other.center), dot(rows[1], other.center));
        self.level_crossings(rows, vector(-shift.x, -shift.y), 1.0)
    }
}

fn circle_box(center: Point, radius: f64) -> crate::Bounds {
    crate::Bounds::from_points(
        Point::new(center.x - radius, center.y - radius),
        Point::new(center.x + radius, center.y + radius),
    )
}

#[cfg(test)]
mod tests {
    use std::f64::consts::TAU;

    use super::super::tests::{near, p, Rng};
    use super::*;

    fn has(found: &[Point], q: Point) -> bool {
        found.iter().any(|f| near(*f, q, 1e-9))
    }

    #[test]
    fn a_line_crosses_touches_or_misses_exactly() {
        let e = Ellipse::full(p(0.0, 0.0), p(4.0, 0.0), 0.5).unwrap();
        let x = 4.0 * (0.75f64).sqrt();
        let across = e.line_crossings(p(-10.0, 1.0), p(10.0, 1.0));
        assert_eq!(across.len(), 2);
        assert!(has(&across, p(x, 1.0)) && has(&across, p(-x, 1.0)));
        let touching = e.line_crossings(p(-10.0, 2.0), p(10.0, 2.0));
        assert_eq!(touching.len(), 1);
        assert!(near(touching[0], p(0.0, 2.0), 1e-9));
        assert!(e.line_crossings(p(-10.0, 3.0), p(10.0, 3.0)).is_empty());
        // Turned and moved, the same line in the ellipse's own frame.
        let turned = e.rotated(p(0.0, 0.0), 0.7).translated(5.0, -3.0);
        let line = [p(-10.0, 1.0), p(10.0, 1.0)]
            .map(|q| q.rotated(p(0.0, 0.0), 0.7).translated(5.0, -3.0));
        let found = turned.line_crossings(line[0], line[1]);
        let want = p(x, 1.0).rotated(p(0.0, 0.0), 0.7).translated(5.0, -3.0);
        assert_eq!(found.len(), 2);
        assert!(has(&found, want));
    }

    #[test]
    fn a_circle_crosses_in_up_to_four_points_and_tangents_count_once() {
        let e = Ellipse::full(p(0.0, 0.0), p(4.0, 0.0), 0.5).unwrap();
        // x²/16 + y²/4 = 1 and x² + y² = 9: x² = 20/3, y² = 7/3.
        let (x, y) = ((20.0f64 / 3.0).sqrt(), (7.0f64 / 3.0).sqrt());
        let four = e.circle_crossings(p(0.0, 0.0), 3.0);
        assert_eq!(four.len(), 4, "{four:?}");
        for q in [p(x, y), p(-x, y), p(x, -y), p(-x, -y)] {
            assert!(has(&four, q), "{q:?} in {four:?}");
        }
        let inside = e.circle_crossings(p(0.0, 0.0), 2.0);
        assert_eq!(inside.len(), 2, "touches top and bottom: {inside:?}");
        assert!(has(&inside, p(0.0, 2.0)) && has(&inside, p(0.0, -2.0)));
        assert_eq!(e.circle_crossings(p(0.0, 0.0), 4.0).len(), 2);
        assert!(e.circle_crossings(p(0.0, 0.0), 1.0).is_empty());
        assert!(e.circle_crossings(p(20.0, 0.0), 1.0).is_empty());
        let round = Ellipse::circular(p(1.0, 1.0), 2.0, 0.0, 0.0).unwrap();
        assert!(
            round.circle_crossings(p(1.0, 1.0), 2.0).is_empty(),
            "the same curve"
        );
    }

    #[test]
    fn two_ellipses_cross_and_identical_ones_do_not() {
        let wide = Ellipse::full(p(0.0, 0.0), p(4.0, 0.0), 0.5).unwrap();
        let tall = Ellipse::full(p(0.0, 0.0), p(0.0, 4.0), 0.5).unwrap();
        // x²/16 + y²/4 = 1 and x²/4 + y²/16 = 1: x² = y² = 16/5.
        let c = (16.0f64 / 5.0).sqrt();
        let found = wide.ellipse_crossings(&tall);
        assert_eq!(found.len(), 4, "{found:?}");
        for q in [p(c, c), p(-c, c), p(c, -c), p(-c, -c)] {
            assert!(has(&found, q), "{q:?}");
        }
        assert!(wide.ellipse_crossings(&wide).is_empty(), "the same curve");
        let small = Ellipse::full(p(0.0, 0.0), p(1.0, 0.0), 0.5).unwrap();
        assert!(
            wide.ellipse_crossings(&small).is_empty(),
            "one inside the other"
        );
        // Touching end to end along the major axis.
        let beside = Ellipse::full(p(8.0, 0.0), p(4.0, 0.0), 0.5).unwrap();
        let touch = wide.ellipse_crossings(&beside);
        assert_eq!(touch.len(), 1, "{touch:?}");
        assert!(near(touch[0], p(4.0, 0.0), 1e-6));
    }

    #[test]
    fn every_crossing_lies_on_both_curves_and_is_found_either_way_round() {
        let mut rng = Rng(21);
        let mut seen = 0;
        for _ in 0..400 {
            let mut a = rng.ellipse();
            let mut b = rng.ellipse();
            a.start = 0.0;
            a.end = TAU;
            b.start = 0.0;
            b.end = TAU;
            b.center = p(
                a.center.x + rng.range(-15.0, 15.0),
                a.center.y + rng.range(-15.0, 15.0),
            );
            let size = a.major_radius().max(b.major_radius());
            let ab = a.ellipse_crossings(&b);
            let ba = b.ellipse_crossings(&a);
            for q in &ab {
                assert!(a.distance_to(*q) < 1e-7 * size && b.distance_to(*q) < 1e-7 * size);
            }
            assert_eq!(ab.len(), ba.len(), "{a:?} {b:?}");
            for q in &ba {
                assert!(ab.iter().any(|f| near(*f, *q, 1e-6 * size)));
            }
            seen += ab.len();
            let (c, r) = (b.center, rng.range(0.5, 20.0));
            for q in a.circle_crossings(c, r) {
                assert!(a.distance_to(q) < 1e-7 * size);
                assert!(((q.x - c.x).hypot(q.y - c.y) - r).abs() < 1e-7 * size.max(r));
            }
            let (l0, l1) = (b.center, b.point_at(1.0));
            for q in a.line_crossings(l0, l1) {
                assert!(a.distance_to(q) < 1e-7 * size);
            }
        }
        assert!(seen > 200, "the random pairs do cross: {seen}");
    }
}
