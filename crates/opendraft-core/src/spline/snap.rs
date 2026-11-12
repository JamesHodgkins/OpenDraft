//! What the object snaps need of a spline: where it crosses another curve
//! or spline, and where it meets a line from a point at a right angle or
//! just touching. The spline is cut into straight pieces to find them, and
//! each is then refined on the spline itself, so the point is on the curve
//! and not on a chord of it.

use super::flatten::MAX_POINTS;
use super::Spline;
use crate::{Bounds, Curve, Point};

/// Samples of the parameter per control point when hunting for feet and
/// tangent points.
const SAMPLES_PER_POINT: usize = 16;
/// How many times a crossing is zoomed in on, and into how many parts each
/// time.
const ROUNDS: usize = 5;
const SPLIT: usize = 16;

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn boxes_meet(a: &Bounds, b: &Bounds) -> bool {
    a.intersects(b)
}

fn box_of(a: Point, b: Point) -> Bounds {
    Bounds::from_points(a, b)
}

/// A piece of the cut spline: the parameter and point at each end.
type Piece = ((f64, Point), (f64, Point));

/// Drop points that repeat an earlier one.
fn distinct(points: Vec<Point>, scale: f64) -> Vec<Point> {
    let mut kept: Vec<Point> = Vec::new();
    for p in points {
        if kept.iter().all(|q| distance(*q, p) > 1e-7 * (1.0 + scale)) {
            kept.push(p);
        }
    }
    kept
}

impl Spline {
    fn extent(&self) -> f64 {
        let b = self.bounds();
        distance(b.min, b.max)
    }

    /// The straight pieces the spline is cut into, those whose box meets
    /// `window`.
    fn snap_pieces(&self, window: &Bounds) -> Vec<Piece> {
        if !self.well_formed() {
            return Vec::new();
        }
        let tolerance = (self.extent() * 1e-3).max(1e-9);
        let samples = self
            .flatten_params(tolerance, MAX_POINTS)
            .unwrap_or_else(|_| self.coarse());
        samples
            .windows(2)
            .filter(|w| boxes_meet(&box_of(w[0].1, w[1].1), window))
            .map(|w| (w[0], w[1]))
            .collect()
    }

    /// Zoom in on the crossing of the spline between parameters `lo` and
    /// `hi` with `other`, starting from the rough point `near`.
    fn zoom_on(&self, other: &Curve, mut lo: f64, mut hi: f64, near: Point) -> Point {
        let mut best = near;
        for _ in 0..ROUNDS {
            let step = (hi - lo) / SPLIT as f64;
            let points: Vec<Point> = (0..=SPLIT)
                .map(|k| self.point_at(lo + step * k as f64))
                .collect();
            let mut pick: Option<(usize, Point)> = None;
            for k in 0..SPLIT {
                let chord = Curve::Segment {
                    a: points[k],
                    b: points[k + 1],
                };
                for p in chord.intersections(other) {
                    if pick.is_none_or(|(_, q)| distance(p, best) < distance(q, best)) {
                        pick = Some((k, p));
                    }
                }
            }
            let Some((k, p)) = pick else {
                break;
            };
            best = p;
            (lo, hi) = (lo + step * k as f64, lo + step * (k + 1) as f64);
        }
        best
    }

    /// Where the spline crosses `other`, among the pieces of it that meet
    /// `window`.
    pub fn crossings_with(&self, other: &Curve, window: &Bounds) -> Vec<Point> {
        let mut found = Vec::new();
        for ((ua, pa), (ub, pb)) in self.snap_pieces(window) {
            let chord = Curve::Segment { a: pa, b: pb };
            for rough in chord.intersections(other) {
                found.push(self.zoom_on(other, ua, ub, rough));
            }
        }
        distinct(found, self.extent())
    }

    /// Refine a crossing of this spline's parameter `u` and `other`'s `v`
    /// by Newton's method, or `None` if it does not settle.
    fn settle(&self, other: &Spline, mut u: f64, mut v: f64) -> Option<Point> {
        let scale = self.extent().max(other.extent());
        let (a, b) = (self.domain(), other.domain());
        for _ in 0..12 {
            let (p, q) = (self.point_at(u), other.point_at(v));
            let f = (p.x - q.x, p.y - q.y);
            if f.0.hypot(f.1) < 1e-12 * (1.0 + scale) {
                return Some(p);
            }
            let (da, db) = (self.derivative_at(u), other.derivative_at(v));
            // Solve [da | -db] (du, dv) = -f.
            let det = -da.x * db.y + db.x * da.y;
            if det.abs() < 1e-14 {
                return None;
            }
            let du = (-f.0 * -db.y - -db.x * -f.1) / det;
            let dv = (da.x * -f.1 - da.y * -f.0) / det;
            u = (u + du).clamp(a.0, a.1);
            v = (v + dv).clamp(b.0, b.1);
        }
        let (p, q) = (self.point_at(u), other.point_at(v));
        (distance(p, q) < 1e-9 * (1.0 + scale)).then_some(p)
    }

    /// Where two splines cross, among the pieces of each that meet `window`.
    pub fn crossings_with_spline(&self, other: &Spline, window: &Bounds) -> Vec<Point> {
        let theirs = other.snap_pieces(window);
        let mut found = Vec::new();
        for ((ua, pa), (ub, pb)) in self.snap_pieces(window) {
            let mine = Curve::Segment { a: pa, b: pb };
            for ((va, qa), (vb, qb)) in &theirs {
                if !boxes_meet(&box_of(pa, pb), &box_of(*qa, *qb)) {
                    continue;
                }
                let theirs = Curve::Segment { a: *qa, b: *qb };
                for rough in mine.intersections(&theirs) {
                    let along = |a: Point, b: Point, (u0, u1): (f64, f64)| {
                        let length = distance(a, b);
                        let t = if length > 0.0 {
                            distance(a, rough) / length
                        } else {
                            0.0
                        };
                        u0 + t * (u1 - u0)
                    };
                    let (u, v) = (along(pa, pb, (ua, ub)), along(*qa, *qb, (*va, *vb)));
                    found.push(self.settle(other, u, v).unwrap_or(rough));
                }
            }
        }
        distinct(found, self.extent().max(other.extent()))
    }

    /// The points of the spline where `f` of the parameter changes sign.
    fn roots(&self, f: impl Fn(f64) -> f64) -> Vec<Point> {
        if !self.well_formed() {
            return Vec::new();
        }
        let (a, b) = self.domain();
        let n = (self.control_points.len() * SAMPLES_PER_POINT).clamp(64, 4096);
        let at = |i: usize| a + (b - a) * i as f64 / n as f64;
        let mut found = Vec::new();
        for i in 0..n {
            let (mut lo, mut hi) = (at(i), at(i + 1));
            let (flo, fhi) = (f(lo), f(hi));
            if flo == 0.0 {
                found.push(self.point_at(lo));
            } else if flo * fhi < 0.0 {
                for _ in 0..60 {
                    let mid = 0.5 * (lo + hi);
                    if f(lo) * f(mid) <= 0.0 {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                found.push(self.point_at(0.5 * (lo + hi)));
            }
        }
        distinct(found, self.extent())
    }

    /// The points of the spline where the line to `from` meets it at a
    /// right angle.
    pub fn perpendicular_feet(&self, from: Point) -> Vec<Point> {
        self.roots(|u| {
            let (p, d) = (self.point_at(u), self.derivative_at(u));
            (p.x - from.x) * d.x + (p.y - from.y) * d.y
        })
    }

    /// The points of the spline where the line from `from` touches it.
    pub fn tangent_points(&self, from: Point) -> Vec<Point> {
        self.roots(|u| {
            let (p, d) = (self.point_at(u), self.derivative_at(u));
            (p.x - from.x) * d.y - (p.y - from.y) * d.x
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn near(a: Point, b: Point) -> bool {
        distance(a, b) < 1e-6
    }

    /// y = 2x - x²/5: a cap from (0, 0) over (5, 5) to (10, 0).
    fn cap() -> Spline {
        Spline::from_control_points(2, vec![p(0.0, 0.0), p(5.0, 10.0), p(10.0, 0.0)]).unwrap()
    }

    fn everywhere() -> Bounds {
        Bounds::from_points(p(-100.0, -100.0), p(100.0, 100.0))
    }

    #[test]
    fn a_line_crosses_a_spline_on_both_sides_of_its_top() {
        let line = Curve::Segment {
            a: p(-1.0, 3.0),
            b: p(11.0, 3.0),
        };
        let found = cap().crossings_with(&line, &everywhere());
        assert_eq!(found.len(), 2, "{found:?}");
        let r = 10.0_f64.sqrt();
        assert!(found.iter().any(|f| near(*f, p(5.0 - r, 3.0))), "{found:?}");
        assert!(found.iter().any(|f| near(*f, p(5.0 + r, 3.0))), "{found:?}");
    }

    #[test]
    fn a_crossing_is_on_the_curve_not_on_a_chord_of_it() {
        let line = Curve::Segment {
            a: p(-1.0, 3.0),
            b: p(11.0, 3.0),
        };
        for q in cap().crossings_with(&line, &everywhere()) {
            let want = 2.0 * q.x - q.x * q.x / 5.0;
            assert!((q.y - want).abs() < 1e-9, "{q:?}");
        }
    }

    #[test]
    fn only_the_part_in_the_window_is_searched() {
        let line = Curve::Segment {
            a: p(-1.0, 3.0),
            b: p(11.0, 3.0),
        };
        let left = Bounds::from_points(p(0.0, 0.0), p(3.0, 6.0));
        let found = cap().crossings_with(&line, &left);
        assert_eq!(found.len(), 1);
        assert!(found[0].x < 3.0);
        let nowhere = Bounds::from_points(p(20.0, 20.0), p(30.0, 30.0));
        assert!(cap().crossings_with(&line, &nowhere).is_empty());
    }

    #[test]
    fn a_curve_that_misses_has_no_crossing() {
        let high = Curve::Segment {
            a: p(-1.0, 8.0),
            b: p(11.0, 8.0),
        };
        assert!(cap().crossings_with(&high, &everywhere()).is_empty());
        let circle = Curve::Circle {
            center: p(5.0, 5.0),
            radius: 2.0,
        };
        // The circle sits across the top of the cap: two crossings.
        assert_eq!(cap().crossings_with(&circle, &everywhere()).len(), 2);
    }

    #[test]
    fn two_splines_cross_where_both_curves_are() {
        let level = Spline::from_control_points(1, vec![p(0.0, 2.0), p(10.0, 2.0)]).unwrap();
        let found = cap().crossings_with_spline(&level, &everywhere());
        assert_eq!(found.len(), 2, "{found:?}");
        let r = 15.0_f64.sqrt();
        assert!(found.iter().any(|f| near(*f, p(5.0 - r, 2.0))), "{found:?}");
        assert!(found.iter().any(|f| near(*f, p(5.0 + r, 2.0))), "{found:?}");
    }

    #[test]
    fn the_foot_from_above_the_top_is_the_top() {
        let feet = cap().perpendicular_feet(p(5.0, 20.0));
        assert!(feet.iter().any(|f| near(*f, p(5.0, 5.0))), "{feet:?}");
        for f in &feet {
            // The line to it is square to the curve there.
            let u = cap().nearest(*f).1;
            let d = cap().derivative_at(u);
            let dot = (f.x - 5.0) * d.x + (f.y - 20.0) * d.y;
            assert!(dot.abs() < 1e-6 * (1.0 + d.x.hypot(d.y)), "{f:?}");
        }
    }

    #[test]
    fn a_tangent_must_be_on_the_spline_itself_and_none_reach_it_from_beneath() {
        // From (10, 10) the tangent at x = 10 - sqrt(50) is on the cap; the
        // other would be past its end.
        let from = p(10.0, 10.0);
        let touch = cap().tangent_points(from);
        assert_eq!(touch.len(), 1, "{touch:?}");
        let x = 10.0 - 50.0_f64.sqrt();
        assert!(near(touch[0], p(x, 2.0 * x - x * x / 5.0)), "{touch:?}");
        let u = cap().nearest(touch[0]).1;
        let d = cap().derivative_at(u);
        let cross = (touch[0].x - from.x) * d.y - (touch[0].y - from.y) * d.x;
        assert!(cross.abs() < 1e-6 * (1.0 + d.x.hypot(d.y)));
        assert!(
            cap().tangent_points(p(5.0, 2.0)).is_empty(),
            "inside the cap"
        );
    }
}
