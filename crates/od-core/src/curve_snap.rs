//! Where a curve offers a Perpendicular or Tangent object snap to a point
//! elsewhere (the command's last point), and the box around a curve. Pure
//! geometry over [`Curve`].

use std::f64::consts::TAU;

use crate::{arc, Bounds, Curve, Point};

/// Distances this close count as equal.
const EPS: f64 = 1e-9;
/// Samples of an ellipse's parameter when hunting for sign changes; four
/// solutions at most, so this only has to resolve them apart.
const ELLIPSE_SAMPLES: usize = 128;

impl Curve {
    /// The box around the curve.
    pub fn bounds(&self) -> Bounds {
        match *self {
            Curve::Segment { a, b } => Bounds::from_points(a, b),
            Curve::Circle { center, radius } => Bounds::from_points(
                Point::new(center.x - radius, center.y - radius),
                Point::new(center.x + radius, center.y + radius),
            ),
            Curve::Arc {
                center,
                radius,
                start,
                end,
            } => arc::bounds(center, radius, start, end),
            Curve::Ellipse(e) => e.bounds(),
        }
    }

    /// Every point of the curve where the line to `from` meets it at a right
    /// angle: the feet of the perpendiculars from `from`. A segment offers its
    /// foot only inside its length, an arc only inside its sweep. Nothing for
    /// a circle or arc whose centre is `from`, where every direction is one.
    pub fn perpendicular_feet(&self, from: Point) -> Vec<Point> {
        match *self {
            Curve::Segment { a, b } => {
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let len_sq = dx * dx + dy * dy;
                if len_sq < EPS * EPS {
                    return Vec::new();
                }
                let t = ((from.x - a.x) * dx + (from.y - a.y) * dy) / len_sq;
                let slack = EPS / len_sq.sqrt();
                if (-slack..=1.0 + slack).contains(&t) {
                    vec![Point::new(a.x + t * dx, a.y + t * dy)]
                } else {
                    Vec::new()
                }
            }
            Curve::Circle { center, radius } => radial_pair(center, radius, from),
            Curve::Arc {
                center,
                radius,
                start,
                end,
            } => radial_pair(center, radius, from)
                .into_iter()
                .filter(|p| arc::contains(start, end, arc::angle_of(center, *p)))
                .collect(),
            Curve::Ellipse(e) => roots_on_ellipse(&e, |t| {
                let p = e.point_at(t);
                let d = e.tangent_at(t);
                (p.x - from.x) * d.x + (p.y - from.y) * d.y
            }),
        }
    }

    /// Every point of the curve where the line from `from` just touches it.
    /// Lines have none, and neither has a curve that encloses `from`. A point
    /// on the curve is its own tangent point.
    pub fn tangent_points(&self, from: Point) -> Vec<Point> {
        match *self {
            Curve::Segment { .. } => Vec::new(),
            Curve::Circle { center, radius } => circle_tangents(center, radius, from),
            Curve::Arc {
                center,
                radius,
                start,
                end,
            } => circle_tangents(center, radius, from)
                .into_iter()
                .filter(|p| arc::contains(start, end, arc::angle_of(center, *p)))
                .collect(),
            Curve::Ellipse(e) => roots_on_ellipse(&e, |t| {
                let p = e.point_at(t);
                let d = e.tangent_at(t);
                (p.x - from.x) * d.y - (p.y - from.y) * d.x
            }),
        }
    }
}

/// The two points of a circle on the line through its centre and `from`.
fn radial_pair(center: Point, radius: f64, from: Point) -> Vec<Point> {
    let (dx, dy) = (from.x - center.x, from.y - center.y);
    let len = dx.hypot(dy);
    if len < EPS || radius < EPS {
        return Vec::new();
    }
    let (ux, uy) = (dx / len * radius, dy / len * radius);
    vec![
        Point::new(center.x + ux, center.y + uy),
        Point::new(center.x - ux, center.y - uy),
    ]
}

/// The points of a circle where the line from `from` touches it: two when
/// `from` is outside, one when it is on the circle, none inside.
fn circle_tangents(center: Point, radius: f64, from: Point) -> Vec<Point> {
    let d = (from.x - center.x).hypot(from.y - center.y);
    if radius < EPS || d < radius - EPS * (1.0 + radius) {
        return Vec::new();
    }
    if d <= radius + EPS * (1.0 + radius) {
        return vec![from];
    }
    let toward = arc::angle_of(center, from);
    let spread = (radius / d).acos();
    vec![
        arc::point_at(center, radius, toward + spread),
        arc::point_at(center, radius, toward - spread),
    ]
}

/// The points of the ellipse (inside its arc, if it is one) where `f` of the
/// parameter is zero. Found by sampling for a change of sign, then halving
/// the interval.
fn roots_on_ellipse(e: &crate::Ellipse, f: impl Fn(f64) -> f64) -> Vec<Point> {
    let step = TAU / ELLIPSE_SAMPLES as f64;
    let mut found: Vec<Point> = Vec::new();
    let mut add = |t: f64| {
        let p = e.point_at(t);
        if e.contains(t) && found.iter().all(|q| (q.x - p.x).hypot(q.y - p.y) > 1e-7) {
            found.push(p);
        }
    };
    for i in 0..ELLIPSE_SAMPLES {
        let (mut lo, mut hi) = (step * i as f64, step * (i + 1) as f64);
        let (flo, fhi) = (f(lo), f(hi));
        if flo == 0.0 {
            add(lo);
        } else if flo * fhi < 0.0 {
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                if f(lo) * f(mid) <= 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            add(0.5 * (lo + hi));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;
    use crate::Ellipse;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).hypot(a.y - b.y) < 1e-6
    }

    #[test]
    fn a_segment_offers_its_foot_only_inside_its_length() {
        let s = Curve::Segment {
            a: p(0.0, 0.0),
            b: p(10.0, 0.0),
        };
        assert_eq!(s.perpendicular_feet(p(4.0, 7.0)), [p(4.0, 0.0)]);
        assert!(s.perpendicular_feet(p(12.0, 7.0)).is_empty());
        assert_eq!(s.perpendicular_feet(p(10.0, 3.0)), [p(10.0, 0.0)]);
        assert!(s.tangent_points(p(4.0, 7.0)).is_empty());
    }

    #[test]
    fn a_circle_has_two_feet_and_none_from_its_centre() {
        let c = Curve::Circle {
            center: p(0.0, 0.0),
            radius: 5.0,
        };
        let feet = c.perpendicular_feet(p(10.0, 0.0));
        assert_eq!(feet.len(), 2);
        assert!(feet.iter().any(|f| near(*f, p(5.0, 0.0))));
        assert!(feet.iter().any(|f| near(*f, p(-5.0, 0.0))));
        assert!(c.perpendicular_feet(p(0.0, 0.0)).is_empty());
    }

    #[test]
    fn an_arc_keeps_only_the_feet_inside_its_sweep() {
        let a = Curve::Arc {
            center: p(0.0, 0.0),
            radius: 5.0,
            start: -0.5,
            end: 0.5,
        };
        // Seen from the east the near foot is on the arc and the far one is not.
        assert_eq!(a.perpendicular_feet(p(10.0, 0.0)).len(), 1);
        // Seen from the west the far foot is the one on it.
        let west = a.perpendicular_feet(p(-10.0, 0.0));
        assert_eq!(west.len(), 1);
        assert!(near(west[0], p(5.0, 0.0)));
    }

    #[test]
    fn tangents_from_outside_on_and_inside_a_circle() {
        let c = Curve::Circle {
            center: p(0.0, 0.0),
            radius: 5.0,
        };
        let touch = c.tangent_points(p(13.0, 0.0));
        assert_eq!(touch.len(), 2, "zero, one and two solutions: two");
        for t in &touch {
            // The radius to a tangent point is square to the line from `from`.
            let dot = t.x * (13.0 - t.x) + t.y * (0.0 - t.y);
            assert!(dot.abs() < 1e-9, "{t:?}");
            assert!((t.x.hypot(t.y) - 5.0).abs() < 1e-9);
        }
        assert_eq!(c.tangent_points(p(5.0, 0.0)), [p(5.0, 0.0)]);
        assert!(c.tangent_points(p(1.0, 1.0)).is_empty());
    }

    #[test]
    fn a_tangent_to_an_arc_must_be_on_the_arc() {
        let a = Curve::Arc {
            center: p(0.0, 0.0),
            radius: 5.0,
            start: 0.0,
            end: 1.5,
        };
        let touch = a.tangent_points(p(13.0, 0.0));
        assert_eq!(touch.len(), 1, "the lower tangent point is off the arc");
        assert!(touch[0].y > 0.0);
    }

    #[test]
    fn ellipse_feet_and_tangents_are_found_numerically() {
        let e = Ellipse::full(p(0.0, 0.0), p(10.0, 0.0), 0.5).unwrap();
        let c = Curve::Ellipse(e);
        // From a point on the major axis the feet are the two vertices (and,
        // for this far a point, no others).
        let feet = c.perpendicular_feet(p(30.0, 0.0));
        assert!(feet.iter().any(|f| near(*f, p(10.0, 0.0))), "{feet:?}");
        assert!(feet.iter().any(|f| near(*f, p(-10.0, 0.0))), "{feet:?}");
        let touch = c.tangent_points(p(20.0, 0.0));
        assert_eq!(touch.len(), 2, "{touch:?}");
        for t in &touch {
            // On the ellipse x²/100 + y²/25 = 1, tangent from (20, 0): x = 5.
            assert!((t.x - 5.0).abs() < 1e-6, "{t:?}");
        }
        assert!(c.tangent_points(p(1.0, 1.0)).is_empty());
    }

    #[test]
    fn an_elliptical_arc_keeps_only_solutions_inside_it() {
        let e = Ellipse::new(p(0.0, 0.0), p(10.0, 0.0), 0.5, 0.0, PI).unwrap();
        let touch = Curve::Ellipse(e).tangent_points(p(20.0, 0.0));
        assert_eq!(touch.len(), 1, "{touch:?}");
        assert!(touch[0].y > 0.0);
    }

    #[test]
    fn curve_bounds_hold_the_curve() {
        let c = Curve::Circle {
            center: p(1.0, 2.0),
            radius: 3.0,
        };
        let b = c.bounds();
        assert!(b.intersects(&Bounds::from_points(p(3.5, 2.0), p(4.5, 2.0))));
        assert!(!b.intersects(&Bounds::from_points(p(4.5, 2.0), p(5.0, 2.0))));
    }
}
