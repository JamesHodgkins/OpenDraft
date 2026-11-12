//! Circular arcs. An [`Entity::Arc`](crate::Entity::Arc) runs counter-clockwise
//! from `start` to `end` (radians, as in DXF). These helpers are pure geometry
//! over those four numbers, shared by drawing, hit-testing, snapping and files.

use std::f64::consts::TAU;

use crate::{Bounds, Point};

/// Below this a sweep counts as empty, which for an arc means a full turn.
const EMPTY_SWEEP: f64 = 1e-12;

/// `angle` brought into `[0, 2π)`.
pub fn normalize(angle: f64) -> f64 {
    angle.rem_euclid(TAU)
}

/// How far the arc turns counter-clockwise from `start` to `end`, in
/// `(0, 2π]`. Equal angles make a full circle.
pub fn sweep(start: f64, end: f64) -> f64 {
    let s = (end - start).rem_euclid(TAU);
    if s < EMPTY_SWEEP {
        TAU
    } else {
        s
    }
}

/// The point at `angle` on the circle.
pub fn point_at(center: Point, radius: f64, angle: f64) -> Point {
    Point::new(
        center.x + radius * angle.cos(),
        center.y + radius * angle.sin(),
    )
}

/// The direction from `center` to `p`, in `[0, 2π)`.
pub fn angle_of(center: Point, p: Point) -> f64 {
    normalize((p.y - center.y).atan2(p.x - center.x))
}

/// Whether `angle` lies on the arc from `start` to `end`.
pub fn contains(start: f64, end: f64, angle: f64) -> bool {
    (angle - start).rem_euclid(TAU) <= sweep(start, end) + EMPTY_SWEEP
}

/// The middle of the arc, halfway round its sweep.
pub fn midpoint(center: Point, radius: f64, start: f64, end: f64) -> Point {
    point_at(center, radius, start + sweep(start, end) / 2.0)
}

/// The box around the arc: its end points, and any quarter points it passes.
pub fn bounds(center: Point, radius: f64, start: f64, end: f64) -> Bounds {
    let r = radius.abs();
    let mut b = Bounds::from_points(point_at(center, r, start), point_at(center, r, end));
    for quarter in 0..4 {
        let angle = f64::from(quarter) * TAU / 4.0;
        if contains(start, end, angle) {
            let p = point_at(center, r, angle);
            b = b.union(Bounds::from_points(p, p));
        }
    }
    b
}

/// Distance from `p` to the arc itself (not the rest of its circle).
pub fn distance_to(center: Point, radius: f64, start: f64, end: f64, p: Point) -> f64 {
    let r = radius.abs();
    if contains(start, end, angle_of(center, p)) {
        return ((p.x - center.x).hypot(p.y - center.y) - r).abs();
    }
    let to = |q: Point| (p.x - q.x).hypot(p.y - q.y);
    to(point_at(center, r, start)).min(to(point_at(center, r, end)))
}

/// Points along the arc, close enough that straight pieces between them stay
/// within `tolerance` of the curve. Always includes both ends.
pub fn flatten(center: Point, radius: f64, start: f64, end: f64, tolerance: f64) -> Vec<Point> {
    let r = radius.abs();
    let total = sweep(start, end);
    // A chord of angle `a` sags by r(1 - cos(a/2)).
    let step = if r <= tolerance {
        total
    } else {
        2.0 * (1.0 - tolerance / r).clamp(-1.0, 1.0).acos()
    };
    let pieces = ((total / step.max(1e-3)).ceil() as usize).clamp(1, 4096);
    (0..=pieces)
        .map(|i| point_at(center, r, start + total * i as f64 / pieces as f64))
        .collect()
}

/// The arc that goes from `a` through `b` to `c`: its centre, radius and
/// start and end angles. `None` when the points are in a line (or repeat).
pub fn through_points(a: Point, b: Point, c: Point) -> Option<(Point, f64, f64, f64)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    let scale = (a.x - b.x)
        .abs()
        .max((b.x - c.x).abs())
        .max((a.y - b.y).abs());
    if d.abs() <= 1e-12 * (1.0 + scale * scale) {
        return None;
    }
    let (a2, b2, c2) = (
        a.x * a.x + a.y * a.y,
        b.x * b.x + b.y * b.y,
        c.x * c.x + c.y * c.y,
    );
    let center = Point::new(
        (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d,
        (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d,
    );
    let radius = (a.x - center.x).hypot(a.y - center.y);
    let (sa, sb, sc) = (
        angle_of(center, a),
        angle_of(center, b),
        angle_of(center, c),
    );
    // Entities run counter-clockwise: if `b` is not on the way from `a` to
    // `c` that way, the user drew it clockwise, so swap the ends.
    if contains(sa, sc, sb) {
        Some((center, radius, sa, sc))
    } else {
        Some((center, radius, sc, sa))
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn sweep_runs_counter_clockwise_and_equal_ends_are_a_full_circle() {
        assert!(near(sweep(0.0, FRAC_PI_2), FRAC_PI_2));
        assert!(near(sweep(3.0 * FRAC_PI_2, FRAC_PI_2), PI), "wraps past 0");
        assert!(near(sweep(1.0, 1.0), TAU));
    }

    #[test]
    fn contains_follows_the_sweep_across_zero() {
        let (start, end) = (3.0 * FRAC_PI_2, FRAC_PI_2);
        assert!(contains(start, end, 0.0));
        assert!(!contains(start, end, PI));
        assert!(contains(start, end, start) && contains(start, end, end));
    }

    #[test]
    fn bounds_include_only_the_quarter_points_the_arc_passes() {
        let o = Point::new(0.0, 0.0);
        // The upper half: reaches y = 1 but not y = -1.
        let b = bounds(o, 1.0, 0.0, PI);
        assert!(near(b.max.y, 1.0) && near(b.min.y, 0.0));
        assert!(near(b.min.x, -1.0) && near(b.max.x, 1.0));
        // A small arc near angle 0 reaches only past x = cos.
        let small = bounds(o, 1.0, -0.1, 0.1);
        assert!(near(small.max.x, 1.0) && small.min.x > 0.9);
    }

    #[test]
    fn distance_is_to_the_arc_not_its_circle() {
        let o = Point::new(0.0, 0.0);
        // Upper half; a point straight below is nearest an end, not the circle.
        assert!(near(
            distance_to(o, 1.0, 0.0, PI, Point::new(0.0, 2.0)),
            1.0
        ));
        assert!(near(
            distance_to(o, 1.0, 0.0, PI, Point::new(0.0, -1.0)),
            2.0_f64.sqrt()
        ));
    }

    #[test]
    fn three_points_give_the_arc_through_them_in_either_direction() {
        let (a, b, c) = (
            Point::new(1.0, 0.0),
            Point::new(0.0, 1.0),
            Point::new(-1.0, 0.0),
        );
        let (center, r, s, e) = through_points(a, b, c).unwrap();
        assert!(near(center.x, 0.0) && near(center.y, 0.0) && near(r, 1.0));
        assert!(near(s, 0.0) && near(e, PI), "a to c, counter-clockwise");
        // Drawn the other way round the same circle: the ends swap.
        let (_, _, s, e) = through_points(c, Point::new(0.0, -1.0), a).unwrap();
        assert!(near(s, PI) && near(e, 0.0), "c to a through the bottom");
        let (_, _, s, e) = through_points(a, Point::new(0.0, -1.0), c).unwrap();
        assert!(near(s, PI) && near(e, 0.0), "clockwise a to c is c to a");
    }

    #[test]
    fn points_in_a_line_make_no_arc() {
        assert!(through_points(
            Point::new(0.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(2.0, 2.0)
        )
        .is_none());
        assert!(through_points(
            Point::new(1.0, 1.0),
            Point::new(1.0, 1.0),
            Point::new(3.0, 0.0)
        )
        .is_none());
    }

    #[test]
    fn flattening_stays_within_tolerance_and_keeps_both_ends() {
        let o = Point::new(5.0, 5.0);
        let points = flatten(o, 10.0, 0.0, PI, 0.01);
        assert!(points.len() > 8);
        let first = points[0];
        let last = points[points.len() - 1];
        assert!(near(first.x, 15.0) && near(last.x, -5.0));
        for w in points.windows(2) {
            let m = Point::new((w[0].x + w[1].x) / 2.0, (w[0].y + w[1].y) / 2.0);
            let sag = 10.0 - (m.x - o.x).hypot(m.y - o.y);
            assert!(sag <= 0.0101, "sag {sag}");
        }
    }
}
