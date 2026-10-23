//! Where curves meet and which point of one is nearest a given point. Pure
//! geometry over [`Entity`], the base for Trim, Extend, Break, Fillet and
//! Offset.
//!
//! Every entity is a list of [`Curve`]s: a segment, a whole circle, an arc,
//! or an ellipse (whole or an arc of one). Two curves meet where their
//! carriers (the infinite line, full circle or full ellipse they lie on)
//! meet and the point lies on both curves themselves. Curves that
//! overlap along a stretch (collinear segments, equal circles) have no single
//! crossing point and report none.

use crate::{arc, bulge, Ellipse, Entity, Point};

/// Distances this close (in drawing units, scaled by size) count as equal.
const EPS: f64 = 1e-9;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    Segment {
        a: Point,
        b: Point,
    },
    Circle {
        center: Point,
        radius: f64,
    },
    Arc {
        center: Point,
        radius: f64,
        start: f64,
        end: f64,
    },
    /// A whole ellipse or an elliptical arc.
    Ellipse(Ellipse),
}

/// The infinite line, full circle or full ellipse a curve lies on.
#[derive(Clone, Copy)]
enum Carrier {
    Line { from: Point, to: Point },
    Circle { center: Point, radius: f64 },
    Ellipse(Ellipse),
}

/// The segments of a path through `points`, including the closing one when
/// `closed`.
pub(crate) fn path_segments(points: &[Point], closed: bool) -> Vec<(Point, Point)> {
    let mut found: Vec<(Point, Point)> = points.windows(2).map(|w| (w[0], w[1])).collect();
    if closed && points.len() > 2 {
        found.push((points[points.len() - 1], points[0]));
    }
    found
}

/// The corners of a rectangle, counter-clockwise from `min`.
pub(crate) fn corners(min: Point, max: Point) -> [Point; 4] {
    [min, Point::new(max.x, min.y), max, Point::new(min.x, max.y)]
}

impl Entity {
    /// The curves the outline is made of, in order.
    pub fn curves(&self) -> Vec<Curve> {
        let segments = |pairs: Vec<(Point, Point)>| {
            pairs
                .into_iter()
                .map(|(a, b)| Curve::Segment { a, b })
                .collect()
        };
        match self {
            // Nothing trims to or extends to a dimension.
            Entity::Dimension(_)
            | Entity::MText(_)
            | Entity::MLeader(_)
            | Entity::Leader(_)
            | Entity::Insert(_)
            | Entity::Point { .. }
            | Entity::Hatch(_) => Vec::new(),
            Entity::Ellipse(e) => vec![Curve::Ellipse(*e)],
            // Not a curve the crossing code knows yet: Trim, Extend and the
            // corner commands refuse splines by name.
            Entity::Spline(_) => Vec::new(),
            Entity::Line { start, end } => vec![Curve::Segment { a: *start, b: *end }],
            Entity::Circle { center, radius } => vec![Curve::Circle {
                center: *center,
                radius: radius.abs(),
            }],
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => vec![Curve::Arc {
                center: *center,
                radius: radius.abs(),
                start: *start,
                end: *end,
            }],
            Entity::Rectangle { min, max } => segments(path_segments(&corners(*min, *max), true)),
            Entity::Polyline {
                points,
                closed,
                bulges,
                ..
            } => {
                let n = points.len();
                (0..bulge::segment_count(n, *closed))
                    .map(|i| {
                        let (a, b) = (points[i], points[(i + 1) % n]);
                        match bulge::arc_of(a, b, bulge::at(bulges, i)) {
                            Some(s) => Curve::Arc {
                                center: s.center,
                                radius: s.radius,
                                start: s.start,
                                end: s.end,
                            },
                            None => Curve::Segment { a, b },
                        }
                    })
                    .collect()
            }
        }
    }

    /// Every point where this entity's outline crosses `other`'s, without
    /// repeats (a corner shared by two segments counts once).
    pub fn intersections(&self, other: &Entity) -> Vec<Point> {
        let theirs = other.curves();
        let mut found = Vec::new();
        for mine in self.curves() {
            for curve in &theirs {
                found.extend(mine.intersections(curve));
            }
        }
        dedupe(found)
    }

    /// The point of the outline nearest `p`, or `None` for an entity with no
    /// outline (a polyline without points).
    pub fn nearest_point(&self, p: Point) -> Option<Point> {
        if let Entity::Spline(s) = self {
            return Some(s.nearest(p).0);
        }
        self.curves()
            .iter()
            .map(|c| c.nearest_point(p))
            .min_by(|a, b| distance(*a, p).total_cmp(&distance(*b, p)))
    }

    /// Distance from `p` to the outline (not the inside); infinite when there
    /// is none.
    pub fn distance_to(&self, p: Point) -> f64 {
        self.nearest_point(p)
            .map_or(f64::INFINITY, |q| distance(p, q))
    }
}

impl Curve {
    fn carrier(&self) -> Option<Carrier> {
        match *self {
            Curve::Segment { a, b } if distance(a, b) > EPS => {
                Some(Carrier::Line { from: a, to: b })
            }
            Curve::Segment { .. } => None,
            Curve::Circle { center, radius } | Curve::Arc { center, radius, .. }
                if radius > EPS =>
            {
                Some(Carrier::Circle { center, radius })
            }
            Curve::Ellipse(e) => Some(Carrier::Ellipse(Ellipse {
                start: 0.0,
                end: std::f64::consts::TAU,
                ..e
            })),
            _ => None,
        }
    }

    /// Whether `p`, a point on this curve's carrier, is on the curve itself.
    fn holds(&self, p: Point) -> bool {
        match *self {
            Curve::Segment { a, b } => {
                let d = (b.x - a.x, b.y - a.y);
                let t = ((p.x - a.x) * d.0 + (p.y - a.y) * d.1) / (d.0 * d.0 + d.1 * d.1);
                (-EPS..=1.0 + EPS).contains(&t)
            }
            Curve::Circle { .. } => true,
            Curve::Arc {
                center,
                radius,
                start,
                end,
            } => {
                arc::contains(start, end, arc::angle_of(center, p))
                    // A crossing computed at an end can land a hair outside.
                    || distance(p, arc::point_at(center, radius, start)) <= EPS * (1.0 + radius)
                    || distance(p, arc::point_at(center, radius, end)) <= EPS * (1.0 + radius)
            }
            Curve::Ellipse(e) => {
                let near = EPS * (1.0 + e.major_radius());
                e.is_full()
                    || e.contains(e.parameter_of(p))
                    || distance(p, e.start_point()) <= near
                    || distance(p, e.end_point()) <= near
            }
        }
    }

    /// Where this curve meets `other`.
    pub fn intersections(&self, other: &Curve) -> Vec<Point> {
        self.extended_intersections(other)
            .into_iter()
            .filter(|p| self.holds(*p))
            .collect()
    }

    /// Where this curve's carrier (its whole line or circle, not just the
    /// part drawn) meets `other` itself. For Extend: how far could it go.
    pub fn extended_intersections(&self, other: &Curve) -> Vec<Point> {
        let (Some(a), Some(b)) = (self.carrier(), other.carrier()) else {
            return Vec::new();
        };
        let points = match (a, b) {
            (Carrier::Line { from, to }, Carrier::Line { from: f2, to: t2 }) => {
                line_line(from, to, f2, t2)
            }
            (Carrier::Line { from, to }, Carrier::Circle { center, radius })
            | (Carrier::Circle { center, radius }, Carrier::Line { from, to }) => {
                line_circle(from, to, center, radius)
            }
            (
                Carrier::Circle { center, radius },
                Carrier::Circle {
                    center: c2,
                    radius: r2,
                },
            ) => circle_circle(center, radius, c2, r2),
            (Carrier::Line { from, to }, Carrier::Ellipse(e))
            | (Carrier::Ellipse(e), Carrier::Line { from, to }) => e.line_crossings(from, to),
            (Carrier::Circle { center, radius }, Carrier::Ellipse(e))
            | (Carrier::Ellipse(e), Carrier::Circle { center, radius }) => {
                e.circle_crossings(center, radius)
            }
            (Carrier::Ellipse(e), Carrier::Ellipse(other)) => e.ellipse_crossings(&other),
        };
        points.into_iter().filter(|p| other.holds(*p)).collect()
    }

    /// The point of the curve nearest `p`.
    pub fn nearest_point(&self, p: Point) -> Point {
        match *self {
            Curve::Segment { a, b } => {
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let len_sq = dx * dx + dy * dy;
                if len_sq == 0.0 {
                    return a;
                }
                let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0);
                Point::new(a.x + t * dx, a.y + t * dy)
            }
            Curve::Circle { center, radius } => on_circle(center, radius, p),
            Curve::Arc {
                center,
                radius,
                start,
                end,
            } => {
                if arc::contains(start, end, arc::angle_of(center, p)) {
                    return on_circle(center, radius, p);
                }
                let (s, e) = (
                    arc::point_at(center, radius, start),
                    arc::point_at(center, radius, end),
                );
                if distance(p, s) <= distance(p, e) {
                    s
                } else {
                    e
                }
            }
            Curve::Ellipse(e) => e.nearest(p).0,
        }
    }
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// The point of the circle in the direction of `p` (east when `p` is the
/// center and every direction is as near as another).
fn on_circle(center: Point, radius: f64, p: Point) -> Point {
    let (dx, dy) = (p.x - center.x, p.y - center.y);
    let len = dx.hypot(dy);
    if len < EPS {
        return Point::new(center.x + radius, center.y);
    }
    Point::new(center.x + dx / len * radius, center.y + dy / len * radius)
}

/// Drop points that repeat an earlier one.
pub(crate) fn dedupe(points: Vec<Point>) -> Vec<Point> {
    let mut kept: Vec<Point> = Vec::new();
    for p in points {
        let tolerance = EPS * (1.0 + p.x.abs().max(p.y.abs()));
        if kept.iter().all(|k| distance(*k, p) > tolerance) {
            kept.push(p);
        }
    }
    kept
}

pub(crate) fn line_line(a: Point, b: Point, c: Point, d: Point) -> Vec<Point> {
    let (r, s) = ((b.x - a.x, b.y - a.y), (d.x - c.x, d.y - c.y));
    let denom = r.0 * s.1 - r.1 * s.0;
    // Parallel (or the same line): no single crossing.
    if denom.abs() <= 1e-12 * r.0.hypot(r.1) * s.0.hypot(s.1) {
        return Vec::new();
    }
    let t = ((c.x - a.x) * s.1 - (c.y - a.y) * s.0) / denom;
    vec![Point::new(a.x + t * r.0, a.y + t * r.1)]
}

pub(crate) fn line_circle(a: Point, b: Point, center: Point, radius: f64) -> Vec<Point> {
    let d = (b.x - a.x, b.y - a.y);
    let f = (a.x - center.x, a.y - center.y);
    let qa = d.0 * d.0 + d.1 * d.1;
    let qb = 2.0 * (f.0 * d.0 + f.1 * d.1);
    let qc = f.0 * f.0 + f.1 * f.1 - radius * radius;
    let mut disc = qb * qb - 4.0 * qa * qc;
    // A line that just touches the circle can come out a hair short.
    if disc < 0.0 {
        if disc < -1e-12 * (qb * qb + (4.0 * qa * qc).abs()) {
            return Vec::new();
        }
        disc = 0.0;
    }
    let root = disc.sqrt();
    let at = |t: f64| Point::new(a.x + t * d.0, a.y + t * d.1);
    if root == 0.0 {
        return vec![at(-qb / (2.0 * qa))];
    }
    vec![at((-qb - root) / (2.0 * qa)), at((-qb + root) / (2.0 * qa))]
}

pub(crate) fn circle_circle(c1: Point, r1: f64, c2: Point, r2: f64) -> Vec<Point> {
    let d = distance(c1, c2);
    // Concentric circles never cross (equal ones overlap everywhere).
    if d < EPS || d > r1 + r2 + EPS * (1.0 + r1 + r2) || d < (r1 - r2).abs() - EPS * (1.0 + r1 + r2)
    {
        return Vec::new();
    }
    let along = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    let h_sq = r1 * r1 - along * along;
    let h = if h_sq > 0.0 { h_sq.sqrt() } else { 0.0 };
    let (ux, uy) = ((c2.x - c1.x) / d, (c2.y - c1.y) / d);
    let base = Point::new(c1.x + along * ux, c1.y + along * uy);
    if h < EPS * (1.0 + r1) {
        return vec![base];
    }
    vec![
        Point::new(base.x - h * uy, base.y + h * ux),
        Point::new(base.x + h * uy, base.y - h * ux),
    ]
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn circle(x: f64, y: f64, r: f64) -> Entity {
        Entity::Circle {
            center: p(x, y),
            radius: r,
        }
    }

    fn arc(x: f64, y: f64, r: f64, start: f64, end: f64) -> Entity {
        Entity::Arc {
            center: p(x, y),
            radius: r,
            start,
            end,
        }
    }

    fn near(a: Point, b: Point) -> bool {
        distance(a, b) < 1e-9
    }

    fn has(found: &[Point], q: Point) -> bool {
        found.iter().any(|f| near(*f, q))
    }

    #[test]
    fn two_segments_cross_only_where_both_reach() {
        let a = line(p(0.0, 0.0), p(10.0, 10.0));
        let found = a.intersections(&line(p(0.0, 10.0), p(10.0, 0.0)));
        assert_eq!(found.len(), 1);
        assert!(near(found[0], p(5.0, 5.0)));
        // The lines would cross at (5, 5) but this segment stops short.
        assert!(a.intersections(&line(p(0.0, 10.0), p(3.0, 7.0))).is_empty());
        // Parallel and collinear segments have no single crossing.
        assert!(a
            .intersections(&line(p(0.0, 1.0), p(10.0, 11.0)))
            .is_empty());
        assert!(a
            .intersections(&line(p(2.0, 2.0), p(20.0, 20.0)))
            .is_empty());
    }

    #[test]
    fn segments_meeting_at_an_end_count_and_touching_is_found_once() {
        let a = line(p(0.0, 0.0), p(5.0, 0.0));
        let found = a.intersections(&line(p(5.0, 0.0), p(5.0, 5.0)));
        assert_eq!(found.len(), 1);
        assert!(near(found[0], p(5.0, 0.0)));
        // A polyline's shared vertex is one crossing, not two.
        let corner = Entity::Polyline {
            points: vec![p(2.0, -3.0), p(2.0, 0.0), p(4.0, 3.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        assert_eq!(a.intersections(&corner).len(), 1);
    }

    #[test]
    fn a_polyline_arc_segment_is_met_like_an_arc() {
        // A half circle from (0, 0) to (4, 0) bulging below (bulge 1), then a
        // straight segment up.
        let poly = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 5.0)],
            closed: false,
            bulges: vec![1.0, 0.0, 0.0],
            widths: Vec::new(),
        };
        let down = line(p(2.0, -10.0), p(2.0, 10.0));
        let found = poly.intersections(&down);
        assert_eq!(found.len(), 1, "the arc bottom only: {found:?}");
        assert!(near(found[0], p(2.0, -2.0)));
        // A horizontal line through the middle of the arc meets it twice.
        let found = poly.intersections(&line(p(-5.0, -1.0), p(9.0, -1.0)));
        assert_eq!(found.len(), 2);
        // The outline reaches below the chord: the nearest point to (2, -5) is
        // the arc's lowest point.
        assert!(near(
            poly.nearest_point(p(2.0, -5.0)).unwrap(),
            p(2.0, -2.0)
        ));
    }

    #[test]
    fn a_segment_meets_a_circle_in_two_one_or_no_points() {
        let c = circle(0.0, 0.0, 5.0);
        let through = line(p(-10.0, 0.0), p(10.0, 0.0));
        let found = through.intersections(&c);
        assert!(found.len() == 2 && has(&found, p(5.0, 0.0)) && has(&found, p(-5.0, 0.0)));
        let touching = line(p(-10.0, 5.0), p(10.0, 5.0));
        let found = touching.intersections(&c);
        assert_eq!(found.len(), 1);
        assert!(near(found[0], p(0.0, 5.0)));
        assert!(line(p(-10.0, 6.0), p(10.0, 6.0))
            .intersections(&c)
            .is_empty());
        // Only the part of the segment that exists counts.
        let short = line(p(0.0, 0.0), p(10.0, 0.0));
        assert_eq!(short.intersections(&c).len(), 1);
        assert!(line(p(0.0, 0.0), p(4.0, 0.0)).intersections(&c).is_empty());
    }

    #[test]
    fn circles_cross_touch_or_miss() {
        let a = circle(0.0, 0.0, 5.0);
        let found = a.intersections(&circle(8.0, 0.0, 5.0));
        assert!(found.len() == 2 && has(&found, p(4.0, 3.0)) && has(&found, p(4.0, -3.0)));
        let found = a.intersections(&circle(10.0, 0.0, 5.0));
        assert!(
            found.len() == 1 && near(found[0], p(5.0, 0.0)),
            "tangent outside"
        );
        let found = a.intersections(&circle(2.0, 0.0, 3.0));
        assert!(
            found.len() == 1 && near(found[0], p(5.0, 0.0)),
            "tangent inside"
        );
        assert!(a.intersections(&circle(20.0, 0.0, 5.0)).is_empty());
        assert!(
            a.intersections(&circle(1.0, 0.0, 1.0)).is_empty(),
            "one inside the other"
        );
        assert!(
            a.intersections(&circle(0.0, 0.0, 3.0)).is_empty(),
            "concentric"
        );
    }

    #[test]
    fn an_arc_is_met_only_along_its_sweep() {
        // The upper half of a circle of radius 5.
        let upper = arc(0.0, 0.0, 5.0, 0.0, PI);
        let vertical = line(p(3.0, -10.0), p(3.0, 10.0));
        let found = vertical.intersections(&upper);
        assert_eq!(found.len(), 1);
        assert!(
            near(found[0], p(3.0, 4.0)),
            "not the (3, -4) on the missing half"
        );
        // At the very end of the arc.
        let at_end = line(p(5.0, -1.0), p(5.0, 1.0)).intersections(&upper);
        assert_eq!(at_end.len(), 1);
        assert!(near(at_end[0], p(5.0, 0.0)));
        // Arc against circle: the circle crosses the upper half once.
        let found = upper.intersections(&circle(8.0, 0.0, 5.0));
        assert_eq!(found.len(), 1);
        assert!(near(found[0], p(4.0, 3.0)));
        // Two arcs of the same circle never cross.
        assert!(upper
            .intersections(&arc(0.0, 0.0, 5.0, PI, 2.0 * PI))
            .is_empty());
    }

    #[test]
    fn rectangles_and_polylines_meet_a_line_at_every_edge_they_cross() {
        let r = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(10.0, 10.0),
        };
        let found = r.intersections(&line(p(-5.0, 5.0), p(15.0, 5.0)));
        assert!(found.len() == 2 && has(&found, p(0.0, 5.0)) && has(&found, p(10.0, 5.0)));
        // Through a corner: one point, not two.
        let found = r.intersections(&line(p(-5.0, -5.0), p(5.0, 5.0)));
        assert_eq!(found.len(), 1, "the corner is shared by two edges");
        assert!(near(found[0], p(0.0, 0.0)));
    }

    #[test]
    fn the_nearest_point_is_on_the_curve() {
        let l = line(p(0.0, 0.0), p(10.0, 0.0));
        assert!(near(l.nearest_point(p(4.0, 3.0)).unwrap(), p(4.0, 0.0)));
        assert!(
            near(l.nearest_point(p(-4.0, 3.0)).unwrap(), p(0.0, 0.0)),
            "clamped to the end"
        );
        let c = circle(0.0, 0.0, 5.0);
        assert!(near(c.nearest_point(p(0.0, 20.0)).unwrap(), p(0.0, 5.0)));
        assert!(near(c.nearest_point(p(1.0, 0.0)).unwrap(), p(5.0, 0.0)));
        // From the center every point is as near: east, by convention.
        assert!(near(c.nearest_point(p(0.0, 0.0)).unwrap(), p(5.0, 0.0)));
        let upper = arc(0.0, 0.0, 5.0, 0.0, PI);
        assert!(near(upper.nearest_point(p(0.0, 9.0)).unwrap(), p(0.0, 5.0)));
        // Below the arc, the nearer end is the answer.
        assert!(near(
            upper.nearest_point(p(4.0, -3.0)).unwrap(),
            p(5.0, 0.0)
        ));
        assert!((upper.distance_to(p(0.0, 8.0)) - 3.0).abs() < 1e-9);
        let empty = Entity::Polyline {
            points: vec![],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        assert!(empty.nearest_point(p(0.0, 0.0)).is_none());
        assert!(empty.distance_to(p(0.0, 0.0)).is_infinite());
    }

    #[test]
    fn an_extended_curve_reaches_what_the_drawn_part_does_not() {
        // A short segment pointing at a wall it does not reach.
        let short = Curve::Segment {
            a: p(0.0, 0.0),
            b: p(2.0, 0.0),
        };
        let wall = Curve::Segment {
            a: p(10.0, -5.0),
            b: p(10.0, 5.0),
        };
        assert!(short.intersections(&wall).is_empty());
        assert!(has(&short.extended_intersections(&wall), p(10.0, 0.0)));
        // The wall itself still has to be there: past its end is no crossing.
        let far_wall = Curve::Segment {
            a: p(10.0, 5.0),
            b: p(10.0, 9.0),
        };
        assert!(short.extended_intersections(&far_wall).is_empty());
        // A quarter arc extended along its whole circle.
        let quarter = Curve::Arc {
            center: p(0.0, 0.0),
            radius: 5.0,
            start: 0.0,
            end: FRAC_PI_2,
        };
        let cutter = Curve::Segment {
            a: p(-10.0, -3.0),
            b: p(10.0, -3.0),
        };
        assert!(quarter.intersections(&cutter).is_empty());
        assert_eq!(quarter.extended_intersections(&cutter).len(), 2);
    }

    /// A small repeatable pseudo-random source, so the checks need no crate.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }

        fn coord(&mut self) -> f64 {
            self.next() * 200.0 - 100.0
        }

        fn point(&mut self) -> Point {
            p(self.coord(), self.coord())
        }

        fn entity(&mut self) -> Entity {
            match (self.next() * 4.0) as u32 {
                0 => line(self.point(), self.point()),
                1 => circle(self.coord(), self.coord(), self.next() * 60.0 + 1.0),
                2 => arc(
                    self.coord(),
                    self.coord(),
                    self.next() * 60.0 + 1.0,
                    self.next() * std::f64::consts::TAU,
                    self.next() * std::f64::consts::TAU,
                ),
                _ => {
                    let points: Vec<Point> = (0..4).map(|_| self.point()).collect();
                    // Half of them have arc segments.
                    let curved = self.next() < 0.5;
                    let bulges = if curved {
                        (0..4).map(|_| self.next() * 3.0 - 1.5).collect()
                    } else {
                        Vec::new()
                    };
                    Entity::Polyline {
                        points,
                        closed: self.next() < 0.5,
                        bulges,
                        widths: Vec::new(),
                    }
                }
            }
        }
    }

    #[test]
    fn every_crossing_found_lies_on_both_outlines_and_is_found_either_way_round() {
        let mut rng = Rng(7);
        let mut crossings = 0;
        for _ in 0..3000 {
            let (a, b) = (rng.entity(), rng.entity());
            let forward = a.intersections(&b);
            for q in &forward {
                assert!(a.distance_to(*q) < 1e-6, "{q:?} not on {a:?}");
                assert!(b.distance_to(*q) < 1e-6, "{q:?} not on {b:?}");
            }
            let backward = b.intersections(&a);
            assert_eq!(forward.len(), backward.len(), "{a:?} and {b:?}");
            for q in &forward {
                assert!(has(&backward, *q));
            }
            crossings += forward.len();
        }
        assert!(
            crossings > 500,
            "the sample should cross often: {crossings}"
        );
    }

    #[test]
    fn the_nearest_point_is_on_the_outline_and_no_other_point_is_nearer() {
        let mut rng = Rng(11);
        for _ in 0..2000 {
            let (e, at) = (rng.entity(), rng.point());
            let q = e.nearest_point(at).unwrap();
            assert!(e.distance_to(q) < 1e-6, "{q:?} not on {e:?}");
            // Sample the outline another way: no sample beats the answer.
            for curve in e.curves() {
                for step in 0..=40 {
                    let t = f64::from(step) / 40.0;
                    let probe = match curve {
                        Curve::Segment { a, b } => p(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t),
                        Curve::Circle { center, radius } => {
                            arc::point_at(center, radius, t * std::f64::consts::TAU)
                        }
                        Curve::Arc {
                            center,
                            radius,
                            start,
                            end,
                        } => arc::point_at(center, radius, start + t * arc::sweep(start, end)),
                        Curve::Ellipse(e) => e.point_at(e.start + t * e.sweep()),
                    };
                    assert!(distance(at, q) <= distance(at, probe) + 1e-6, "{e:?}");
                }
            }
        }
    }
}
