//! Arc segments of polylines. A segment from `a` to `b` with a *bulge* is an
//! arc: the bulge is the tangent of a quarter of its angle (DXF's group 42),
//! 0 for a straight segment, positive turning counter-clockwise from `a` to
//! `b`, negative clockwise, 1 a half circle. These helpers are pure geometry
//! over `a`, `b` and the bulge, shared by everything that reads a polyline.

use std::f64::consts::{PI, TAU};

use crate::{arc, Bounds, Point};

/// Bulges smaller than this are straight.
pub const STRAIGHT: f64 = 1e-9;

/// The circle a bulged segment lies on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentArc {
    pub center: Point,
    pub radius: f64,
    /// The arc as an [`Entity::Arc`](crate::Entity::Arc) runs: counter-clockwise
    /// from `start` to `end`.
    pub start: f64,
    pub end: f64,
    /// Whether travelling from the segment's first point to its second follows
    /// `start` to `end` (a positive bulge) or goes the other way.
    pub forward: bool,
    /// How far round the arc turns, in (0, 2π).
    pub sweep: f64,
}

/// The bulge of segment `i`; 0 when the polyline has none.
pub fn at(bulges: &[f64], i: usize) -> f64 {
    bulges.get(i).copied().unwrap_or(0.0)
}

/// How many segments `points` points make.
pub fn segment_count(points: usize, closed: bool) -> usize {
    match points {
        0 | 1 => 0,
        2 => 1,
        n if closed => n,
        n => n - 1,
    }
}

/// Whether any segment of a polyline is an arc.
pub fn has_arcs(bulges: &[f64]) -> bool {
    bulges.iter().any(|b| b.abs() > STRAIGHT)
}

/// The arc of the segment `a` to `b` with this bulge, or `None` for a
/// straight segment (or one with no length).
pub fn arc_of(a: Point, b: Point, bulge: f64) -> Option<SegmentArc> {
    if !bulge.is_finite() || bulge.abs() <= STRAIGHT {
        return None;
    }
    let chord = (b.x - a.x).hypot(b.y - a.y);
    if chord <= 1e-12 {
        return None;
    }
    let left = Point::new(-(b.y - a.y) / chord, (b.x - a.x) / chord);
    let mid = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    let offset = chord / 2.0 * (1.0 - bulge * bulge) / (2.0 * bulge);
    let center = Point::new(mid.x + left.x * offset, mid.y + left.y * offset);
    let radius = chord / 2.0 * (1.0 + bulge * bulge) / (2.0 * bulge.abs());
    let (from, to) = (arc::angle_of(center, a), arc::angle_of(center, b));
    let forward = bulge > 0.0;
    let (start, end) = if forward { (from, to) } else { (to, from) };
    Some(SegmentArc {
        center,
        radius,
        start,
        end,
        forward,
        sweep: 4.0 * bulge.abs().atan(),
    })
}

/// The bulge of an arc that turns `sweep` radians, counter-clockwise or not.
pub fn from_sweep(sweep: f64, counter_clockwise: bool) -> f64 {
    let magnitude = (sweep / 4.0).tan();
    if counter_clockwise {
        magnitude
    } else {
        -magnitude
    }
}

/// The bulge of the arc from `a` through `m` to `b`, or `None` when the
/// three are in a line (or the arc would be a whole circle).
pub fn through(a: Point, m: Point, b: Point) -> Option<f64> {
    let (center, ..) = arc::through_points(a, m, b)?;
    let (sa, sm) = (arc::angle_of(center, a), arc::angle_of(center, m));
    let counter_clockwise = arc::contains(sa, arc::angle_of(center, b), sm);
    let sweep = if counter_clockwise {
        arc::sweep(sa, arc::angle_of(center, b))
    } else {
        arc::sweep(arc::angle_of(center, b), sa)
    };
    (sweep < TAU - 1e-6).then(|| from_sweep(sweep, counter_clockwise))
}

/// The point `t` of the way along the segment (0 is `a`, 1 is `b`; for an arc
/// `t` is a fraction of the angle).
pub fn point_at(a: Point, b: Point, bulge: f64, t: f64) -> Point {
    match arc_of(a, b, bulge) {
        None => Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t),
        Some(s) => {
            let angle = if s.forward {
                s.start + s.sweep * t
            } else {
                s.end - s.sweep * t
            };
            arc::point_at(s.center, s.radius, angle)
        }
    }
}

/// The middle of the segment.
pub fn midpoint(a: Point, b: Point, bulge: f64) -> Point {
    point_at(a, b, bulge, 0.5)
}

/// How long the segment is.
pub fn length(a: Point, b: Point, bulge: f64) -> f64 {
    match arc_of(a, b, bulge) {
        None => (b.x - a.x).hypot(b.y - a.y),
        Some(s) => s.radius * s.sweep,
    }
}

/// The bulge of the part of a segment between fractions `from` and `to` of
/// the way along it: a piece of an arc turns by the same fraction of its
/// angle.
pub fn part(bulge: f64, from: f64, to: f64) -> f64 {
    if bulge.abs() <= STRAIGHT {
        return 0.0;
    }
    ((to - from) * bulge.atan()).tan()
}

/// The point of the segment nearest `p`, and how far along it that is.
pub fn nearest(a: Point, b: Point, bulge: f64, p: Point) -> (Point, f64) {
    let Some(s) = arc_of(a, b, bulge) else {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len_sq = dx * dx + dy * dy;
        let t = if len_sq == 0.0 {
            0.0
        } else {
            (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0)
        };
        return (point_at(a, b, 0.0, t), t);
    };
    let angle = arc::angle_of(s.center, p);
    if arc::contains(s.start, s.end, angle) {
        let along = (angle - s.start).rem_euclid(TAU).min(s.sweep);
        let t = if s.forward {
            along / s.sweep
        } else {
            1.0 - along / s.sweep
        };
        return (arc::point_at(s.center, s.radius, angle), t);
    }
    let da = (p.x - a.x).hypot(p.y - a.y);
    let db = (p.x - b.x).hypot(p.y - b.y);
    if da <= db {
        (a, 0.0)
    } else {
        (b, 1.0)
    }
}

/// Points along the segment, within `tolerance` of the curve, both ends
/// included, in the order from `a` to `b`.
pub fn flatten(a: Point, b: Point, bulge: f64, tolerance: f64) -> Vec<Point> {
    let Some(s) = arc_of(a, b, bulge) else {
        return vec![a, b];
    };
    let mut points = arc::flatten(s.center, s.radius, s.start, s.end, tolerance);
    if !s.forward {
        points.reverse();
    }
    // The ends are exact, whatever rounding the arc did.
    if let Some(first) = points.first_mut() {
        *first = a;
    }
    if let Some(last) = points.last_mut() {
        *last = b;
    }
    points
}

/// The box around the segment.
pub fn bounds(a: Point, b: Point, bulge: f64) -> Bounds {
    match arc_of(a, b, bulge) {
        None => Bounds::from_points(a, b),
        Some(s) => arc::bounds(s.center, s.radius, s.start, s.end),
    }
}

/// The box around a path of segments (or around its points alone when it has
/// fewer than two), or `None` for no points at all.
pub fn path_bounds(points: &[Point], bulges: &[f64], closed: bool) -> Option<Bounds> {
    let segments = segment_count(points.len(), closed);
    let mut found: Option<Bounds> = None;
    let mut add = |b: Bounds| found = Some(found.map_or(b, |f| f.union(b)));
    for i in 0..segments {
        add(bounds(
            points[i],
            points[(i + 1) % points.len()],
            at(bulges, i),
        ));
    }
    if segments == 0 {
        points.iter().for_each(|p| add(Bounds::from_points(*p, *p)));
    }
    found
}

/// The points of a path with its arc segments approximated by straight
/// pieces within `tolerance` of the curve: one list to draw as a line (and
/// close, when `closed`; the first point is not repeated at the end).
pub fn flatten_path(points: &[Point], bulges: &[f64], closed: bool, tolerance: f64) -> Vec<Point> {
    let n = points.len();
    let segments = segment_count(n, closed);
    if segments == 0 {
        return points.to_vec();
    }
    let mut out: Vec<Point> = Vec::with_capacity(n);
    for i in 0..segments {
        let piece = flatten(points[i], points[(i + 1) % n], at(bulges, i), tolerance);
        // Each piece starts where the last one ended.
        out.extend(piece.iter().take(piece.len() - 1).copied());
    }
    if !closed {
        out.push(points[n - 1]);
    }
    out
}

/// The direction of travel at the end of a segment, as a unit vector.
pub fn direction_at_end(a: Point, b: Point, bulge: f64) -> Point {
    match arc_of(a, b, bulge) {
        None => {
            let len = (b.x - a.x).hypot(b.y - a.y);
            if len <= 1e-12 {
                Point::new(1.0, 0.0)
            } else {
                Point::new((b.x - a.x) / len, (b.y - a.y) / len)
            }
        }
        Some(s) => {
            let angle = arc::angle_of(s.center, b);
            // Counter-clockwise travel is a quarter turn ahead of the radius.
            if s.forward {
                Point::new(-angle.sin(), angle.cos())
            } else {
                Point::new(angle.sin(), -angle.cos())
            }
        }
    }
}

/// The bulge of the arc that leaves `from` along `direction` (a unit vector)
/// and ends at `to`, or `None` when `to` is straight ahead or straight behind.
pub fn tangent(from: Point, direction: Point, to: Point) -> Option<f64> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    if dx.hypot(dy) <= 1e-12 {
        return None;
    }
    // The angle from the direction to the chord, counter-clockwise positive.
    let turn = (direction.x * dy - direction.y * dx).atan2(direction.x * dx + direction.y * dy);
    // Straight ahead is no arc, straight back would be a whole circle.
    (turn.abs() > 1e-9 && turn.abs() < PI - 1e-6).then(|| (turn / 2.0).tan())
}

/// How long a path is, arcs included.
pub fn path_length(points: &[Point], bulges: &[f64], closed: bool) -> f64 {
    let n = points.len();
    (0..segment_count(n, closed))
        .map(|i| length(points[i], points[(i + 1) % n], at(bulges, i)))
        .sum()
}

/// The area enclosed by a path taken as a closed shape (an open path is
/// closed by a straight line back to its start, as AutoCAD reports it), arcs
/// included.
pub fn path_area(points: &[Point], bulges: &[f64]) -> f64 {
    let n = points.len();
    if n < 2 {
        return 0.0;
    }
    let mut twice = 0.0;
    let mut bows = 0.0;
    for i in 0..n {
        let (a, b) = (points[i], points[(i + 1) % n]);
        twice += a.x * b.y - b.x * a.y;
        // The last point's bulge only matters when the path closes on it; an
        // open path's closing line is straight.
        if let Some(s) = arc_of(a, b, at(bulges, i)) {
            // The sliver between the chord and the arc, counted with the sign
            // of the turn so it adds to a counter-clockwise shape.
            let sliver = 0.5 * s.radius * s.radius * (s.sweep - s.sweep.sin());
            bows += if s.forward { sliver } else { -sliver };
        }
    }
    (twice / 2.0 + bows).abs()
}

/// Mirroring a polyline turns every arc the other way.
pub fn mirrored(bulges: &[f64]) -> Vec<f64> {
    bulges.iter().map(|b| -b).collect()
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).hypot(a.y - b.y) < 1e-9
    }

    #[test]
    fn a_bulge_of_one_is_a_half_circle_on_the_chord() {
        // From (0,0) to (2,0), counter-clockwise: passes below, through (1,-1).
        let s = arc_of(p(0.0, 0.0), p(2.0, 0.0), 1.0).unwrap();
        assert!(near(s.center, p(1.0, 0.0)) && (s.radius - 1.0).abs() < 1e-9);
        assert!((s.sweep - PI).abs() < 1e-9);
        assert!(near(midpoint(p(0.0, 0.0), p(2.0, 0.0), 1.0), p(1.0, -1.0)));
        // Negative runs the other way, through (1, 1).
        assert!(near(midpoint(p(0.0, 0.0), p(2.0, 0.0), -1.0), p(1.0, 1.0)));
        assert!((length(p(0.0, 0.0), p(2.0, 0.0), 1.0) - PI).abs() < 1e-9);
    }

    #[test]
    fn a_zero_bulge_is_straight() {
        assert_eq!(arc_of(p(0.0, 0.0), p(2.0, 0.0), 0.0), None);
        assert!(near(midpoint(p(0.0, 0.0), p(4.0, 2.0), 0.0), p(2.0, 1.0)));
        assert!((length(p(0.0, 0.0), p(3.0, 4.0), 0.0) - 5.0).abs() < 1e-9);
        assert_eq!(arc_of(p(1.0, 1.0), p(1.0, 1.0), 0.5), None, "no chord");
    }

    #[test]
    fn a_quarter_turn_has_a_bulge_of_tan_of_an_eighth() {
        let b = from_sweep(FRAC_PI_2, true);
        assert!((b - (FRAC_PI_2 / 4.0).tan()).abs() < 1e-12);
        let s = arc_of(p(1.0, 0.0), p(0.0, 1.0), b).unwrap();
        assert!(near(s.center, p(0.0, 0.0)) && (s.radius - 1.0).abs() < 1e-9);
        assert!(s.start.abs() < 1e-9 && (s.end - FRAC_PI_2).abs() < 1e-9);
        assert!(s.forward);
        // The same quarter drawn clockwise from (0, 1) to (1, 0).
        let s = arc_of(p(0.0, 1.0), p(1.0, 0.0), -b).unwrap();
        assert!(near(s.center, p(0.0, 0.0)) && !s.forward);
        assert!(s.start.abs() < 1e-9 && (s.end - FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn points_run_along_the_arc_from_a_to_b_whichever_way_it_turns() {
        for bulge in [0.7, -0.7] {
            let (a, b) = (p(0.0, 0.0), p(4.0, 1.0));
            assert!(near(point_at(a, b, bulge, 0.0), a));
            assert!(near(point_at(a, b, bulge, 1.0), b));
            let s = arc_of(a, b, bulge).unwrap();
            for t in [0.2, 0.5, 0.9] {
                let q = point_at(a, b, bulge, t);
                assert!(((q.x - s.center.x).hypot(q.y - s.center.y) - s.radius).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn the_bulge_through_three_points_round_trips() {
        let (a, b) = (p(0.0, 0.0), p(4.0, 0.0));
        for bulge in [0.4, -0.4, 1.5, -2.0, 0.05] {
            let m = midpoint(a, b, bulge);
            let found = through(a, m, b).unwrap();
            assert!((found - bulge).abs() < 1e-9, "{found} vs {bulge}");
        }
        assert_eq!(
            through(p(0.0, 0.0), p(1.0, 1.0), p(2.0, 2.0)),
            None,
            "a line"
        );
    }

    #[test]
    fn a_piece_of_an_arc_has_its_own_bulge() {
        let bulge = 0.8;
        let (a, b) = (p(0.0, 0.0), p(5.0, 2.0));
        // Cut at the third and two thirds: the middle part spans a third of
        // the angle and joins the two cut points.
        let (t0, t1) = (1.0 / 3.0, 2.0 / 3.0);
        let (c0, c1) = (point_at(a, b, bulge, t0), point_at(a, b, bulge, t1));
        let piece = part(bulge, t0, t1);
        let s = arc_of(c0, c1, piece).unwrap();
        let whole = arc_of(a, b, bulge).unwrap();
        assert!(near(s.center, whole.center) && (s.radius - whole.radius).abs() < 1e-9);
        // The halves put back together.
        assert!((part(bulge, 0.0, 1.0) - bulge).abs() < 1e-12);
        assert_eq!(part(0.0, 0.2, 0.7), 0.0);
    }

    #[test]
    fn the_nearest_point_is_on_the_arc_and_t_says_how_far_along() {
        let (a, b, bulge) = (p(0.0, 0.0), p(2.0, 0.0), 1.0);
        // Below the middle of the half circle.
        let (q, t) = nearest(a, b, bulge, p(1.0, -5.0));
        assert!(near(q, p(1.0, -1.0)) && (t - 0.5).abs() < 1e-9);
        // Above the chord, off the arc: the nearer end.
        let (q, t) = nearest(a, b, bulge, p(1.9, 3.0));
        assert!(near(q, b) && t == 1.0);
        // Clockwise: the same middle point at t = 0.5, the ends swapped round.
        let (q, t) = nearest(a, b, -bulge, p(1.0, 5.0));
        assert!(near(q, p(1.0, 1.0)) && (t - 0.5).abs() < 1e-9);
        let (q, t) = nearest(p(0.0, 0.0), p(4.0, 0.0), 0.0, p(1.0, 3.0));
        assert!(near(q, p(1.0, 0.0)) && (t - 0.25).abs() < 1e-9);
    }

    #[test]
    fn flattening_keeps_both_ends_exact_and_stays_close_to_the_curve() {
        let (a, b) = (p(0.0, 0.0), p(10.0, 0.0));
        for bulge in [0.5, -0.5] {
            let points = flatten(a, b, bulge, 0.01);
            assert!(points.len() > 4);
            assert_eq!(points[0], a);
            assert_eq!(points[points.len() - 1], b);
            let s = arc_of(a, b, bulge).unwrap();
            for q in &points {
                let d = (q.x - s.center.x).hypot(q.y - s.center.y);
                assert!((d - s.radius).abs() < 1e-6);
            }
        }
        assert_eq!(flatten(a, b, 0.0, 0.01), vec![a, b]);
    }

    #[test]
    fn a_flattened_path_has_every_vertex_once_and_follows_the_arcs() {
        let points = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)];
        let straight = flatten_path(&points, &[], false, 0.01);
        assert_eq!(straight, points.to_vec());
        let curved = flatten_path(&points, &[1.0, 0.0, 0.0], false, 0.05);
        assert!(curved.len() > 6);
        assert_eq!(curved[0], points[0]);
        assert_eq!(curved[curved.len() - 1], points[2]);
        assert!(
            curved.contains(&points[1]),
            "the vertex between is there once"
        );
        assert_eq!(curved.iter().filter(|q| **q == points[1]).count(), 1);
        // Closed: the first point is not repeated at the end.
        let closed = flatten_path(&points, &[], true, 0.01);
        assert_eq!(closed, points.to_vec());
    }

    #[test]
    fn a_tangent_arc_leaves_along_the_direction_and_meets_the_next_one_smoothly() {
        // Leaving (0,0) heading east, ending at (2, 2): a quarter circle about (0, 2).
        let b = tangent(p(0.0, 0.0), p(1.0, 0.0), p(2.0, 2.0)).unwrap();
        let s = arc_of(p(0.0, 0.0), p(2.0, 2.0), b).unwrap();
        assert!(near(s.center, p(0.0, 2.0)) && (s.radius - 2.0).abs() < 1e-9);
        assert!((s.sweep - FRAC_PI_2).abs() < 1e-9);
        // It leaves heading east (the start of a counter-clockwise arc about a
        // centre above it) and arrives heading north.
        let end = direction_at_end(p(0.0, 0.0), p(2.0, 2.0), b);
        assert!(end.x.abs() < 1e-9 && (end.y - 1.0).abs() < 1e-9, "{end:?}");
        // A right turn is a clockwise arc: negative bulge.
        let down = tangent(p(0.0, 0.0), p(1.0, 0.0), p(2.0, -2.0)).unwrap();
        assert!((down + b).abs() < 1e-9);
        // Straight ahead or straight back is no tangent arc.
        assert_eq!(tangent(p(0.0, 0.0), p(1.0, 0.0), p(5.0, 0.0)), None);
        assert_eq!(tangent(p(0.0, 0.0), p(1.0, 0.0), p(-5.0, 0.0)), None);
        // A straight segment's end direction is along it.
        let d = direction_at_end(p(0.0, 0.0), p(0.0, 3.0), 0.0);
        assert!(near(d, p(0.0, 1.0)));
        // A clockwise arc arrives heading the right way round too.
        let d = direction_at_end(p(0.0, 0.0), p(2.0, -2.0), down);
        assert!(d.x.abs() < 1e-9 && (d.y + 1.0).abs() < 1e-9, "{d:?}");
    }

    #[test]
    fn a_paths_length_and_area_include_its_arcs() {
        // A half circle on the chord from (0,0) to (2,0): the closed shape is a
        // half disc of radius 1.
        let points = [p(0.0, 0.0), p(2.0, 0.0)];
        assert!((path_length(&points, &[1.0, 0.0], false) - PI).abs() < 1e-9);
        assert!((path_area(&points, &[1.0, 0.0]) - PI / 2.0).abs() < 1e-9);
        // Bulging the other way encloses the same area.
        assert!((path_area(&points, &[-1.0, 0.0]) - PI / 2.0).abs() < 1e-9);
        // A square with one side bowed out gains the sliver; bowed in loses it.
        let square = [p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(0.0, 2.0)];
        assert!((path_area(&square, &[]) - 4.0).abs() < 1e-9);
        let sliver = 0.5 * (PI - 0.0); // half a unit disc: the bulge-1 side
        assert!((path_area(&square, &[1.0, 0.0, 0.0, 0.0]) - (4.0 + sliver)).abs() < 1e-9);
        assert!((path_area(&square, &[-1.0, 0.0, 0.0, 0.0]) - (4.0 - sliver)).abs() < 1e-9);
        // The perimeter swaps a side of 2 for a half circle of pi.
        assert!((path_length(&square, &[1.0, 0.0, 0.0, 0.0], true) - (6.0 + PI)).abs() < 1e-9);
    }

    #[test]
    fn the_box_of_a_bulged_segment_includes_the_bulge() {
        let b = bounds(p(0.0, 0.0), p(2.0, 0.0), 1.0);
        assert!(near(b.min, p(0.0, -1.0)) && near(b.max, p(2.0, 0.0)));
        let b = bounds(p(0.0, 0.0), p(2.0, 0.0), -1.0);
        assert!(near(b.min, p(0.0, 0.0)) && near(b.max, p(2.0, 1.0)));
        assert_eq!(mirrored(&[0.5, 0.0, -1.0]), vec![-0.5, -0.0, 1.0]);
        assert_eq!(segment_count(4, false), 3);
        assert_eq!(segment_count(4, true), 4);
        assert_eq!(segment_count(2, true), 1);
        assert!(has_arcs(&[0.0, 0.2]) && !has_arcs(&[0.0, 0.0]) && !has_arcs(&[]));
    }
}
