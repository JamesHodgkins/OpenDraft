//! Parallel copies of an entity at a distance. The base for OFFSET.
//!
//! The side is chosen by a point: the copy goes to the side of the outline
//! the point is on. Lines move sideways, circles and arcs change radius,
//! rectangles grow or shrink, and polylines move every segment sideways and
//! join the neighbours where their lines meet (a sharp corner, not a round
//! one). Pieces of a polyline that an inward offset turns back on themselves
//! are dropped. Offsets that would self-cross (a tight curve offset inward) are
//! not trimmed.

use std::f64::consts::{PI, TAU};

use crate::intersect::{circle_circle, line_circle, line_line};
use crate::{arc, bulge, Entity, Point};

/// Lengths below this are nothing.
const EPS: f64 = 1e-9;

impl Entity {
    /// A copy `distance` away on the side `toward` is on. `None` when there
    /// is no such copy: a zero or negative distance, a point on the outline
    /// (no side), or an inward offset that does not fit (a circle's radius
    /// used up, a rectangle squeezed flat, a polyline that vanishes).
    pub fn offset(&self, distance: f64, toward: Point) -> Option<Entity> {
        if !(distance.is_finite() && distance > EPS) {
            return None;
        }
        match self {
            Entity::Dimension(_)
            | Entity::MText(_)
            | Entity::Text(_)
            | Entity::MLeader(_)
            | Entity::Leader(_)
            | Entity::Insert(_)
            | Entity::AttributeDefinition(_)
            | Entity::Point { .. }
            | Entity::Hatch(_) => None,
            Entity::Xline { origin, direction } | Entity::Ray { origin, direction } => {
                // Which side `toward` is on; a point on the line has none.
                let side =
                    (toward.x - origin.x) * -direction.y + (toward.y - origin.y) * direction.x;
                if side.abs() <= EPS {
                    return None;
                }
                let shifted =
                    crate::infinite::shifted(*origin, *direction, distance * side.signum());
                Some(self.with_infinite_line(shifted, *direction))
            }
            // The offset of an ellipse is not an ellipse; AutoCAD makes a
            // spline, which the drawing has no kind for yet.
            Entity::Ellipse(_) => None,
            // A spline's offset needs fitting a new spline; not built yet.
            Entity::Spline(_) => None,
            Entity::Line { start, end } => {
                let side = side_of(*start, *end, toward)?;
                let n = left_normal(*start, *end)?;
                let shift = (n.x * side * distance, n.y * side * distance);
                Some(Entity::Line {
                    start: start.translated(shift.0, shift.1),
                    end: end.translated(shift.0, shift.1),
                })
            }
            Entity::Circle { center, radius } => Some(Entity::Circle {
                center: *center,
                radius: new_radius(*center, radius.abs(), distance, toward)?,
            }),
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => Some(Entity::Arc {
                center: *center,
                radius: new_radius(*center, radius.abs(), distance, toward)?,
                start: *start,
                end: *end,
            }),
            Entity::Rectangle { min, max } => offset_rectangle(*min, *max, distance, toward),
            Entity::Polyline {
                points,
                closed,
                bulges,
                ..
            } => offset_polyline(points, *closed, bulges, distance, toward),
        }
    }
}

fn dist(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// +1 when `p` is left of the direction a to b, -1 when right, `None` when
/// on the line.
fn side_of(a: Point, b: Point, p: Point) -> Option<f64> {
    let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
    let scale = dist(a, b) * (1.0 + dist(a, p));
    (cross.abs() > EPS * scale).then(|| cross.signum())
}

/// The unit vector to the left of the direction a to b.
fn left_normal(a: Point, b: Point) -> Option<Point> {
    let len = dist(a, b);
    (len > EPS).then(|| Point::new(-(b.y - a.y) / len, (b.x - a.x) / len))
}

fn new_radius(center: Point, radius: f64, distance: f64, toward: Point) -> Option<f64> {
    let away = dist(center, toward);
    if (away - radius).abs() <= EPS * (1.0 + radius) {
        return None;
    }
    let made = if away > radius {
        radius + distance
    } else {
        radius - distance
    };
    (made > EPS).then_some(made)
}

fn offset_rectangle(min: Point, max: Point, distance: f64, toward: Point) -> Option<Entity> {
    let inside = toward.x > min.x && toward.x < max.x && toward.y > min.y && toward.y < max.y;
    let on_edge =
        !inside && toward.x >= min.x && toward.x <= max.x && toward.y >= min.y && toward.y <= max.y;
    if on_edge {
        return None;
    }
    let d = if inside { -distance } else { distance };
    let (lo, hi) = (
        Point::new(min.x - d, min.y - d),
        Point::new(max.x + d, max.y + d),
    );
    (hi.x - lo.x > EPS && hi.y - lo.y > EPS).then_some(Entity::Rectangle { min: lo, max: hi })
}

/// One segment moved sideways: its ends and bulge. A straight segment keeps
/// its direction and an arc keeps its centre and angle, which is how a segment
/// turned back on itself is spotted.
#[derive(Clone, Copy)]
struct Shifted {
    from: Point,
    to: Point,
    bulge: f64,
}

/// An original segment: its ends and bulge.
type Seg = (Point, Point, f64);

/// +1 when `p` is left of the direction of travel along the segment, -1 when
/// right, `None` when on it. An arc's left is toward its centre when it turns
/// counter-clockwise and away from it when it turns clockwise.
fn side_of_segment(seg: Seg, p: Point) -> Option<f64> {
    let (a, b, bulge) = seg;
    match bulge::arc_of(a, b, bulge) {
        None => side_of(a, b, p),
        Some(s) => {
            let away = dist(s.center, p);
            if (away - s.radius).abs() <= EPS * (1.0 + s.radius) {
                return None;
            }
            let inside = away < s.radius;
            Some(if inside == s.forward { 1.0 } else { -1.0 })
        }
    }
}

/// The segment moved `distance` to the `side` of its travel (+1 left), or
/// `None` when an arc has no radius left.
fn shift(seg: Seg, side: f64, distance: f64) -> Option<Shifted> {
    let (a, b, bulge) = seg;
    match bulge::arc_of(a, b, bulge) {
        None => {
            let n = left_normal(a, b)?;
            let by = (n.x * side * distance, n.y * side * distance);
            Some(Shifted {
                from: a.translated(by.0, by.1),
                to: b.translated(by.0, by.1),
                bulge: 0.0,
            })
        }
        Some(s) => {
            let toward_centre = (side > 0.0) == s.forward;
            let radius = if toward_centre {
                s.radius - distance
            } else {
                s.radius + distance
            };
            if radius <= EPS {
                return None;
            }
            let scale = radius / s.radius;
            let on = |p: Point| {
                Point::new(
                    s.center.x + (p.x - s.center.x) * scale,
                    s.center.y + (p.y - s.center.y) * scale,
                )
            };
            Some(Shifted {
                from: on(a),
                to: on(b),
                bulge,
            })
        }
    }
}

fn offset_polyline(
    points: &[Point],
    closed: bool,
    bulges: &[f64],
    distance: f64,
    toward: Point,
) -> Option<Entity> {
    // Repeated points make zero-length segments with no direction.
    let mut clean: Vec<(Point, f64)> = Vec::new();
    for (i, p) in points.iter().enumerate() {
        if clean.last().is_none_or(|last| dist(last.0, *p) > EPS) {
            clean.push((*p, bulge::at(bulges, i)));
        }
    }
    if closed && clean.len() > 1 && dist(clean[0].0, clean[clean.len() - 1].0) <= EPS {
        clean.pop();
    }
    let closed = closed && clean.len() > 2;
    let n = clean.len();
    let count = bulge::segment_count(n, closed);
    if count == 0 {
        return None;
    }
    let originals: Vec<Seg> = (0..count)
        .map(|i| (clean[i].0, clean[(i + 1) % n].0, clean[i].1))
        .collect();
    // The side is taken from the segment nearest the point, then kept the
    // same (to the left or right of travel) for every segment.
    let nearest = originals.iter().min_by(|x, y| {
        let reach = |s: &Seg| {
            let (q, _) = bulge::nearest(s.0, s.1, s.2, toward);
            dist(q, toward)
        };
        reach(x).total_cmp(&reach(y))
    })?;
    let side = side_of_segment(*nearest, toward)?;
    let mut shifted: Vec<Shifted> = originals
        .iter()
        .filter_map(|seg| shift(*seg, side, distance))
        .collect();

    // Join neighbours; a segment that comes out reversed has been squeezed
    // out by the offset, so drop it and join again.
    loop {
        let needed = if closed { 3 } else { 1 };
        if shifted.len() < needed {
            return None;
        }
        let vertices = join(&shifted, closed);
        let curves: Vec<Option<f64>> = shifted
            .iter()
            .enumerate()
            .map(|(i, s)| new_bulge(s, vertices[i], vertices[(i + 1) % vertices.len()]))
            .collect();
        match curves.iter().position(Option::is_none) {
            Some(i) => {
                shifted.remove(i);
            }
            None => {
                let mut curves: Vec<f64> = curves.into_iter().flatten().collect();
                if !closed {
                    curves.push(0.0);
                }
                return Some(Entity::Polyline {
                    points: vertices,
                    closed,
                    bulges: if bulge::has_arcs(&curves) {
                        curves
                    } else {
                        Vec::new()
                    },
                    widths: Vec::new(),
                });
            }
        }
    }
}

/// The bulge of a shifted segment once joined to its neighbours, which moved
/// its ends along its line or circle; `None` when it has been turned back on
/// itself.
fn new_bulge(shifted: &Shifted, from: Point, to: Point) -> Option<f64> {
    match bulge::arc_of(shifted.from, shifted.to, shifted.bulge) {
        None => {
            let along = (to.x - from.x) * (shifted.to.x - shifted.from.x)
                + (to.y - from.y) * (shifted.to.y - shifted.from.y);
            (along > 0.0).then_some(0.0)
        }
        Some(s) => {
            let (a, b) = (arc::angle_of(s.center, from), arc::angle_of(s.center, to));
            let sweep = if s.forward {
                (b - a).rem_euclid(TAU)
            } else {
                (a - b).rem_euclid(TAU)
            };
            // Going the other way round, or round again, is a collapsed arc.
            let sensible = sweep > 1e-9 && (sweep - s.sweep).abs() < PI && sweep < TAU - 1e-9;
            sensible.then(|| bulge::from_sweep(sweep, s.forward))
        }
    }
}

/// The vertices of the path made by the shifted segments: neighbours meet
/// where their lines or circles cross, the crossing nearest where they were
/// joined before (or, when they run on together, where one ends).
fn join(segments: &[Shifted], closed: bool) -> Vec<Point> {
    let meet = |a: &Shifted, b: &Shifted| {
        let reference = Point::new((a.to.x + b.from.x) / 2.0, (a.to.y + b.from.y) / 2.0);
        let carrier =
            |s: &Shifted| bulge::arc_of(s.from, s.to, s.bulge).map(|c| (c.center, c.radius));
        let found = match (carrier(a), carrier(b)) {
            (None, None) => line_line(a.from, a.to, b.from, b.to),
            (None, Some((c, r))) => line_circle(a.from, a.to, c, r),
            (Some((c, r)), None) => line_circle(b.from, b.to, c, r),
            (Some((c1, r1)), Some((c2, r2))) => circle_circle(c1, r1, c2, r2),
        };
        found
            .into_iter()
            .min_by(|x, y| dist(*x, reference).total_cmp(&dist(*y, reference)))
            .unwrap_or(a.to)
    };
    let n = segments.len();
    let mut vertices = Vec::with_capacity(n + 1);
    if closed {
        // Vertex i sits at the start of segment i, where the one before meets it.
        for i in 0..n {
            vertices.push(meet(&segments[(i + n - 1) % n], &segments[i]));
        }
    } else {
        vertices.push(segments[0].from);
        for i in 1..n {
            vertices.push(meet(&segments[i - 1], &segments[i]));
        }
        vertices.push(segments[n - 1].to);
    }
    vertices
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_construction_line_offsets_to_the_side_asked() {
        use crate::{Entity, Point};
        let line = Entity::Xline {
            origin: Point::new(0.0, 0.0),
            direction: Point::new(1.0, 0.0),
        };
        let up = line.offset(2.0, Point::new(50.0, 9.0)).unwrap();
        assert_eq!(up.infinite_line().unwrap().0, Point::new(0.0, 2.0));
        let down = line.offset(2.0, Point::new(50.0, -9.0)).unwrap();
        assert_eq!(down.infinite_line().unwrap().0, Point::new(0.0, -2.0));
        assert_eq!(line.offset(2.0, Point::new(50.0, 0.0)), None);
    }

    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;
    use crate::arc;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn near(a: Point, b: Point) -> bool {
        dist(a, b) < 1e-9
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn polyline(points: &[Point], closed: bool) -> Entity {
        Entity::Polyline {
            points: points.to_vec(),
            closed,
            bulges: Vec::new(),
            widths: Vec::new(),
        }
    }

    fn points_of(e: &Entity) -> Vec<Point> {
        match e {
            Entity::Polyline { points, .. } => points.clone(),
            other => panic!("expected a polyline, got {other:?}"),
        }
    }

    #[test]
    fn a_line_moves_sideways_to_the_side_of_the_point() {
        let l = line(p(0.0, 0.0), p(10.0, 0.0));
        assert_eq!(
            l.offset(2.0, p(5.0, 9.0)),
            Some(line(p(0.0, 2.0), p(10.0, 2.0)))
        );
        assert_eq!(
            l.offset(2.0, p(5.0, -1.0)),
            Some(line(p(0.0, -2.0), p(10.0, -2.0)))
        );
        assert_eq!(l.offset(2.0, p(5.0, 0.0)), None, "on the line: no side");
        assert_eq!(l.offset(0.0, p(5.0, 1.0)), None);
        assert_eq!(l.offset(-1.0, p(5.0, 1.0)), None);
        // The side does not depend on which way the line was drawn.
        let reversed = line(p(10.0, 0.0), p(0.0, 0.0));
        assert_eq!(
            reversed.offset(2.0, p(5.0, 9.0)),
            Some(line(p(10.0, 2.0), p(0.0, 2.0)))
        );
    }

    #[test]
    fn circles_and_arcs_change_radius_and_keep_their_centre_and_sweep() {
        let c = Entity::Circle {
            center: p(1.0, 1.0),
            radius: 5.0,
        };
        let radius = |e: Option<Entity>| match e {
            Some(Entity::Circle { radius, .. }) => Some(radius),
            None => None,
            other => panic!("{other:?}"),
        };
        assert_eq!(radius(c.offset(2.0, p(20.0, 1.0))), Some(7.0));
        assert_eq!(radius(c.offset(2.0, p(2.0, 1.0))), Some(3.0));
        assert_eq!(radius(c.offset(5.0, p(2.0, 1.0))), None, "nothing left");
        assert_eq!(radius(c.offset(9.0, p(2.0, 1.0))), None);
        assert_eq!(radius(c.offset(1.0, p(6.0, 1.0))), None, "on the circle");
        let a = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 4.0,
            start: 0.5,
            end: 2.0,
        };
        assert_eq!(
            a.offset(1.0, p(0.0, 10.0)),
            Some(Entity::Arc {
                center: p(0.0, 0.0),
                radius: 5.0,
                start: 0.5,
                end: 2.0
            })
        );
    }

    #[test]
    fn a_rectangle_grows_or_shrinks_and_stays_a_rectangle() {
        let r = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(10.0, 6.0),
        };
        assert_eq!(
            r.offset(1.0, p(20.0, 20.0)),
            Some(Entity::Rectangle {
                min: p(-1.0, -1.0),
                max: p(11.0, 7.0)
            })
        );
        assert_eq!(
            r.offset(1.0, p(5.0, 3.0)),
            Some(Entity::Rectangle {
                min: p(1.0, 1.0),
                max: p(9.0, 5.0)
            })
        );
        assert_eq!(r.offset(3.0, p(5.0, 3.0)), None, "squeezed to nothing");
        assert_eq!(r.offset(4.0, p(5.0, 3.0)), None);
        assert_eq!(r.offset(1.0, p(5.0, 0.0)), None, "on an edge: no side");
    }

    #[test]
    fn a_closed_polyline_offsets_with_sharp_corners_either_way() {
        // A square, drawn counter-clockwise: the answer must not depend on
        // the drawing direction.
        let ccw = polyline(
            &[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)],
            true,
        );
        let cw = polyline(
            &[p(0.0, 0.0), p(0.0, 10.0), p(10.0, 10.0), p(10.0, 0.0)],
            true,
        );
        for square in [&ccw, &cw] {
            let out = points_of(&square.offset(1.0, p(-5.0, -5.0)).unwrap());
            assert_eq!(out.len(), 4);
            let b = square.offset(1.0, p(-5.0, -5.0)).unwrap().bounds();
            assert!(near(b.min, p(-1.0, -1.0)) && near(b.max, p(11.0, 11.0)));
            let inward = square.offset(1.0, p(5.0, 5.0)).unwrap().bounds();
            assert!(near(inward.min, p(1.0, 1.0)) && near(inward.max, p(9.0, 9.0)));
            assert!(
                square.offset(5.0, p(5.0, 5.0)).is_none(),
                "nothing left inside"
            );
        }
    }

    #[test]
    fn an_open_polyline_is_offset_to_one_side_and_joined_at_its_bends() {
        // An L: along the x axis, then up.
        let l = polyline(&[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)], false);
        // Left of travel is up at the first leg and left at the second.
        let out = points_of(&l.offset(1.0, p(5.0, 3.0)).unwrap());
        assert_eq!(out.len(), 3);
        assert!(near(out[0], p(0.0, 1.0)));
        assert!(
            near(out[1], p(9.0, 1.0)),
            "the corner of the two offset lines"
        );
        assert!(near(out[2], p(9.0, 10.0)));
        // The other side gives the outside of the bend.
        let out = points_of(&l.offset(1.0, p(5.0, -3.0)).unwrap());
        assert!(
            near(out[0], p(0.0, -1.0))
                && near(out[1], p(11.0, -1.0))
                && near(out[2], p(11.0, 10.0))
        );
    }

    #[test]
    fn collinear_segments_offset_without_a_bend() {
        let l = polyline(&[p(0.0, 0.0), p(5.0, 0.0), p(10.0, 0.0)], false);
        let out = points_of(&l.offset(2.0, p(5.0, 1.0)).unwrap());
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|q| (q.y - 2.0).abs() < 1e-9));
    }

    #[test]
    fn a_tight_inward_offset_leaves_no_zero_length_segments() {
        // A bump in the middle of a long run, offset toward its inside by
        // more than half its width: the bump's short top turns back on itself.
        let l = polyline(
            &[
                p(0.0, 0.0),
                p(4.0, 0.0),
                p(4.0, 1.0),
                p(5.0, 1.0),
                p(5.0, 0.0),
                p(9.0, 0.0),
            ],
            false,
        );
        let out = points_of(&l.offset(0.8, p(2.0, -3.0)).unwrap());
        // Every kept segment still points the way it did.
        for w in out.windows(2) {
            assert!(dist(w[0], w[1]) > 1e-9);
        }
        assert!(out.len() >= 2);
        // A shape that is gone entirely gives nothing.
        let tiny = polyline(&[p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)], true);
        assert!(tiny.offset(2.0, p(0.5, 0.5)).is_none());
    }

    #[test]
    fn the_offset_is_the_asked_distance_away() {
        // Lines, circles, arcs and rectangles: sample the copy and measure.
        let cases = [
            (line(p(0.0, 0.0), p(10.0, 3.0)), p(2.0, 8.0)),
            (
                Entity::Circle {
                    center: p(2.0, 2.0),
                    radius: 6.0,
                },
                p(0.0, 0.0),
            ),
            (
                Entity::Arc {
                    center: p(0.0, 0.0),
                    radius: 6.0,
                    start: 0.2,
                    end: PI,
                },
                p(30.0, 30.0),
            ),
        ];
        for (original, toward) in cases {
            let copy = original.offset(1.5, toward).unwrap();
            for curve in copy.curves() {
                let samples: Vec<Point> = match curve {
                    crate::Curve::Segment { a, b } => vec![a, b],
                    crate::Curve::Circle { center, radius } => (0..8)
                        .map(|i| arc::point_at(center, radius, f64::from(i) * FRAC_PI_2 / 2.0))
                        .collect(),
                    crate::Curve::Arc {
                        center,
                        radius,
                        start,
                        end,
                    } => (0..=4)
                        .map(|i| {
                            arc::point_at(
                                center,
                                radius,
                                start + arc::sweep(start, end) * f64::from(i) / 4.0,
                            )
                        })
                        .collect(),
                    crate::Curve::Ellipse(_) => panic!("an offset is never an ellipse"),
                };
                for s in samples {
                    assert!((original.distance_to(s) - 1.5).abs() < 1e-9, "{original:?}");
                }
            }
        }
    }

    /// A 10 by 6 rectangle with each corner rounded to radius 2: four lines and
    /// four quarter arcs, as one counter-clockwise polyline.
    fn rounded_rectangle() -> Entity {
        let q = std::f64::consts::FRAC_PI_8.tan();
        Entity::Polyline {
            points: vec![
                p(2.0, 0.0),
                p(8.0, 0.0),
                p(10.0, 2.0),
                p(10.0, 4.0),
                p(8.0, 6.0),
                p(2.0, 6.0),
                p(0.0, 4.0),
                p(0.0, 2.0),
            ],
            closed: true,
            bulges: vec![0.0, q, 0.0, q, 0.0, q, 0.0, q],
            widths: Vec::new(),
        }
    }

    #[test]
    fn a_polyline_with_arcs_offsets_outward_with_concentric_arcs() {
        let outward = rounded_rectangle().offset(1.0, p(-5.0, -5.0)).unwrap();
        let Entity::Polyline {
            points,
            bulges,
            closed,
            ..
        } = &outward
        else {
            panic!()
        };
        assert!(*closed && points.len() == 8);
        // The lines moved out by one and the corner radii grew to three.
        let b = outward.bounds();
        assert!(
            near(b.min, p(-1.0, -1.0)) && near(b.max, p(11.0, 7.0)),
            "{b:?}"
        );
        let q = std::f64::consts::FRAC_PI_8.tan();
        assert!(bulges
            .iter()
            .skip(1)
            .step_by(2)
            .all(|k| (k - q).abs() < 1e-9));
        let mid = bulge::midpoint(points[1], points[2], bulges[1]);
        assert!(
            (dist(mid, p(8.0, 2.0)) - 3.0).abs() < 1e-9,
            "radius 3 about (8, 2)"
        );
        // Every point of the copy is one away from the original.
        for t in [0.0, 0.3, 0.7] {
            let q = bulge::point_at(points[0], points[1], 0.0, t);
            assert!((rounded_rectangle().distance_to(q) - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn a_polyline_with_arcs_offsets_inward_with_smaller_arcs_and_an_arc_can_vanish() {
        let inward = rounded_rectangle().offset(1.0, p(5.0, 3.0)).unwrap();
        let b = inward.bounds();
        assert!(
            near(b.min, p(1.0, 1.0)) && near(b.max, p(9.0, 5.0)),
            "{b:?}"
        );
        let Entity::Polyline { points, bulges, .. } = &inward else {
            panic!()
        };
        let mid = bulge::midpoint(points[1], points[2], bulges[1]);
        assert!(
            (dist(mid, p(8.0, 2.0)) - 1.0).abs() < 1e-9,
            "radius 1 about (8, 2)"
        );
        // Inward by exactly the radius leaves the arcs with no radius: they go,
        // and the lines are joined where they meet.
        let tight = rounded_rectangle().offset(2.0, p(5.0, 3.0)).unwrap();
        let Entity::Polyline { points, .. } = &tight else {
            panic!()
        };
        let b = tight.bounds();
        assert!(near(b.min, p(2.0, 2.0)) && near(b.max, p(8.0, 4.0)));
        assert_eq!(points.len(), 4, "a plain rectangle: {points:?}");
        // Further in leaves nothing.
        assert!(rounded_rectangle().offset(3.5, p(5.0, 3.0)).is_none());
    }

    #[test]
    fn an_open_arc_polyline_offsets_to_either_side() {
        // A half circle from (0, 0) to (4, 0) bulging below (bulge 1).
        let arc = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0)],
            closed: false,
            bulges: vec![1.0, 0.0],
            widths: Vec::new(),
        };
        // Toward the centre (above the bulge): a smaller half circle.
        let inner = arc.offset(0.5, p(2.0, 0.5)).unwrap();
        let Entity::Polyline { points, bulges, .. } = &inner else {
            panic!()
        };
        assert!(near(points[0], p(0.5, 0.0)) && near(points[1], p(3.5, 0.0)));
        assert!((bulges[0] - 1.0).abs() < 1e-9);
        // Away: a larger one.
        let outer = arc.offset(0.5, p(2.0, -5.0)).unwrap();
        assert!(near(outer.bounds().min, p(-0.5, -2.5)));
    }
}
