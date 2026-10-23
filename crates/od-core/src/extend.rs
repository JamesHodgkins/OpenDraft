//! Lengthening an entity until it meets a boundary. The base for Extend.

use std::f64::consts::TAU;

use crate::intersect::dedupe;
use crate::{arc, bulge, Curve, Ellipse, Entity, Point};

const EPS: f64 = 1e-9;

/// Which end of an entity to move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Start,
    End,
}

impl Entity {
    /// The entity with `end` lengthened, in the direction it already runs,
    /// to the nearest point where that run meets one of `boundaries`. `None`
    /// when nothing is in the way, or the kind cannot be extended (a circle,
    /// a full ellipse, a rectangle, a closed polyline).
    ///
    /// - A line keeps its direction; an arc keeps its circle and grows round
    ///   it, never past a full turn, and an elliptical arc likewise round its
    ///   ellipse; an open polyline extends its first or last segment as a
    ///   line.
    /// - A boundary has to exist where it is met: extending does not
    ///   lengthen the boundary too.
    pub fn extended_to(&self, end: End, boundaries: &[Entity]) -> Option<Entity> {
        let walls: Vec<Curve> = boundaries.iter().flat_map(Entity::curves).collect();
        match self {
            Entity::Line { start, end: stop } => {
                let (base, tip) = match end {
                    End::Start => (*stop, *start),
                    End::End => (*start, *stop),
                };
                let reached = reach_along(base, tip, &walls)?;
                Some(match end {
                    End::Start => Entity::Line {
                        start: reached,
                        end: *stop,
                    },
                    End::End => Entity::Line {
                        start: *start,
                        end: reached,
                    },
                })
            }
            Entity::Arc {
                center,
                radius,
                start,
                end: stop,
            } => {
                let (new_start, new_end) =
                    extend_arc(*center, *radius, *start, *stop, end, &walls)?;
                Some(Entity::Arc {
                    center: *center,
                    radius: *radius,
                    start: new_start,
                    end: new_end,
                })
            }
            Entity::Ellipse(e) if !e.is_full() => {
                extend_ellipse(e, end, &walls).map(Entity::Ellipse)
            }
            Entity::Polyline {
                points,
                closed: false,
                bulges,
                widths,
            } if points.len() >= 2 => {
                let n = points.len();
                // The end segment, and which of its two ends is the tip.
                let (segment, tip_is_end) = match end {
                    End::Start => (0, false),
                    End::End => (n - 2, true),
                };
                let (a, b) = (points[segment], points[segment + 1]);
                let curve = bulge::at(bulges, segment);
                let (new_tip, new_curve) = match bulge::arc_of(a, b, curve) {
                    // A straight end segment runs on in its own direction.
                    None => {
                        let (base, tip) = if tip_is_end { (a, b) } else { (b, a) };
                        (reach_along(base, tip, &walls)?, 0.0)
                    }
                    // An arc segment runs on round its own circle.
                    Some(s) => {
                        let moves = if tip_is_end == s.forward {
                            End::End
                        } else {
                            End::Start
                        };
                        let (start, stop) =
                            extend_arc(s.center, s.radius, s.start, s.end, moves, &walls)?;
                        let angle = if moves == End::End { stop } else { start };
                        (
                            arc::point_at(s.center, s.radius, angle),
                            bulge::from_sweep(arc::sweep(start, stop), s.forward),
                        )
                    }
                };
                let mut points = points.clone();
                let mut curves = bulges.clone();
                points[if tip_is_end { n - 1 } else { 0 }] = new_tip;
                if !curves.is_empty() || new_curve != 0.0 {
                    curves.resize(n, 0.0);
                    curves[segment] = new_curve;
                }
                Some(Entity::Polyline {
                    points,
                    closed: false,
                    bulges: curves,
                    widths: widths.clone(),
                })
            }
            _ => None,
        }
    }
}

/// The nearest point beyond `tip`, on the line from `base` through it, that
/// lies on a wall.
fn reach_along(base: Point, tip: Point, walls: &[Curve]) -> Option<Point> {
    let (dx, dy) = (tip.x - base.x, tip.y - base.y);
    let len_sq = dx * dx + dy * dy;
    if len_sq == 0.0 {
        return None;
    }
    let ray = Curve::Segment { a: base, b: tip };
    let hits = dedupe(
        walls
            .iter()
            .flat_map(|wall| ray.extended_intersections(wall))
            .collect(),
    );
    hits.into_iter()
        .map(|q| (((q.x - tip.x) * dx + (q.y - tip.y) * dy) / len_sq, q))
        // Strictly ahead of the tip, by more than rounding.
        .filter(|(t, _)| *t > EPS)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, q)| q)
}

/// The new `(start, end)` angles of an arc with one end grown to a wall.
fn extend_arc(
    center: Point,
    radius: f64,
    start: f64,
    stop: f64,
    end: End,
    walls: &[Curve],
) -> Option<(f64, f64)> {
    let sweep = arc::sweep(start, stop);
    let circle = Curve::Circle { center, radius };
    let angles = dedupe(
        walls
            .iter()
            .flat_map(|wall| circle.extended_intersections(wall))
            .collect(),
    )
    .into_iter()
    .map(|q| arc::angle_of(center, q));
    // How far round from the moving end, in the direction it grows. Points
    // that fall on the arc already are not beyond the end.
    let run = |angle: f64| match end {
        End::End => (angle - stop).rem_euclid(TAU),
        End::Start => (start - angle).rem_euclid(TAU),
    };
    let grow = angles
        .map(run)
        .filter(|d| *d > EPS && *d < TAU - sweep - EPS)
        .min_by(f64::total_cmp)?;
    Some(match end {
        End::End => (start, arc::normalize(stop + grow)),
        End::Start => (arc::normalize(start - grow), stop),
    })
}

/// An elliptical arc with one end grown round its ellipse to a wall.
fn extend_ellipse(e: &Ellipse, end: End, walls: &[Curve]) -> Option<Ellipse> {
    let whole = Curve::Ellipse(Ellipse {
        start: 0.0,
        end: TAU,
        ..*e
    });
    let sweep = e.sweep();
    let run = |t: f64| match end {
        End::End => (t - e.end).rem_euclid(TAU),
        End::Start => (e.start - t).rem_euclid(TAU),
    };
    let grow = dedupe(
        walls
            .iter()
            .flat_map(|wall| whole.extended_intersections(wall))
            .collect(),
    )
    .into_iter()
    .map(|q| run(e.parameter_of(q)))
    .filter(|d| *d > EPS && *d < TAU - sweep - EPS)
    .min_by(f64::total_cmp)?;
    let (start, stop) = match end {
        End::End => (e.start, e.end + grow),
        End::Start => (e.start - grow, e.end),
    };
    Ellipse::new(e.center, e.major_axis, e.radius_ratio, start, stop).ok()
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

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).hypot(a.y - b.y) < 1e-9
    }

    fn wall(x: f64) -> Entity {
        line(p(x, -10.0), p(x, 10.0))
    }

    #[test]
    fn a_line_grows_from_either_end_to_the_nearest_wall() {
        let l = line(p(0.0, 0.0), p(2.0, 0.0));
        let walls = [wall(10.0), wall(5.0), wall(-4.0)];
        assert_eq!(
            l.extended_to(End::End, &walls),
            Some(line(p(0.0, 0.0), p(5.0, 0.0))),
            "the nearer wall ahead, not the far one"
        );
        assert_eq!(
            l.extended_to(End::Start, &walls),
            Some(line(p(-4.0, 0.0), p(2.0, 0.0))),
        );
    }

    #[test]
    fn nothing_ahead_or_a_wall_that_is_too_short_means_no_extension() {
        let l = line(p(0.0, 0.0), p(2.0, 0.0));
        assert_eq!(l.extended_to(End::End, &[]), None);
        assert_eq!(l.extended_to(End::End, &[wall(-3.0)]), None, "behind it");
        // The wall stops above where the line would meet it.
        let high = line(p(8.0, 1.0), p(8.0, 9.0));
        assert_eq!(l.extended_to(End::End, &[high]), None);
        // A wall the line already touches or crosses is not ahead of the tip.
        assert_eq!(l.extended_to(End::End, &[wall(1.0)]), None);
        assert_eq!(l.extended_to(End::End, &[wall(2.0)]), None, "already there");
    }

    #[test]
    fn a_line_can_grow_to_a_circle_and_an_arc() {
        let l = line(p(0.0, 0.0), p(1.0, 0.0));
        let circle = Entity::Circle {
            center: p(10.0, 0.0),
            radius: 3.0,
        };
        assert_eq!(
            l.extended_to(End::End, &[circle]),
            Some(line(p(0.0, 0.0), p(7.0, 0.0)))
        );
        let upper_half = Entity::Arc {
            center: p(10.0, 0.0),
            radius: 3.0,
            start: 0.0,
            end: PI,
        };
        // The line runs along the axis, through both ends of the half arc:
        // the near end is at (7, 0).
        assert_eq!(
            l.extended_to(End::End, &[upper_half]),
            Some(line(p(0.0, 0.0), p(7.0, 0.0)))
        );
    }

    #[test]
    fn an_arc_grows_round_its_own_circle_in_the_direction_of_the_end() {
        // A quarter from east to north, radius 5.
        let a = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 5.0,
            start: 0.0,
            end: FRAC_PI_2,
        };
        // A wall on the left: x = -3 meets the circle at (-3, 4), 143 degrees on.
        let grown = a.extended_to(End::End, &[wall(-3.0)]).unwrap();
        let Entity::Arc { start, end, .. } = grown else {
            panic!()
        };
        assert!(start.abs() < 1e-9);
        let want = (4.0_f64).atan2(-3.0);
        assert!((end - want).abs() < 1e-9, "{end} vs {want}");
        // The start grows the other way, clockwise, to the low side of the wall.
        let grown = a.extended_to(End::Start, &[wall(-3.0)]).unwrap();
        let Entity::Arc { start, end, .. } = grown else {
            panic!()
        };
        assert!((end - FRAC_PI_2).abs() < 1e-9);
        assert!((start - (-4.0_f64).atan2(-3.0).rem_euclid(TAU)).abs() < 1e-9);
    }

    #[test]
    fn an_arc_never_grows_back_over_itself() {
        // Three quarters of a circle; the only wall point is inside the arc.
        let a = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 5.0,
            start: 0.0,
            end: 3.0 * FRAC_PI_2,
        };
        // x = -3 meets the circle at (-3, 4) and (-3, -4): both on the arc
        // already, so neither is beyond its end.
        assert_eq!(a.extended_to(End::End, &[wall(-3.0)]), None);
        assert_eq!(a.extended_to(End::Start, &[wall(-3.0)]), None);
        // x = 3 meets it at (3, -4), in the quarter the arc leaves open.
        let Some(Entity::Arc { end, .. }) = a.extended_to(End::End, &[wall(3.0)]) else {
            panic!("the open quarter can be closed up to the wall");
        };
        assert!((end - (-4.0_f64).atan2(3.0).rem_euclid(TAU)).abs() < 1e-9);
    }

    #[test]
    fn an_open_polyline_extends_its_end_segment_only() {
        let l = Entity::Polyline {
            points: vec![p(0.0, 5.0), p(0.0, 0.0), p(2.0, 0.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        assert_eq!(
            l.extended_to(End::End, &[wall(6.0)]),
            Some(Entity::Polyline {
                points: vec![p(0.0, 5.0), p(0.0, 0.0), p(6.0, 0.0)],
                closed: false,
                bulges: Vec::new(),
                widths: Vec::new()
            })
        );
        // The start segment runs up the page; a wall above it is the target.
        let ceiling = line(p(-5.0, 9.0), p(5.0, 9.0));
        assert_eq!(
            l.extended_to(End::Start, &[ceiling]),
            Some(Entity::Polyline {
                points: vec![p(0.0, 9.0), p(0.0, 0.0), p(2.0, 0.0)],
                closed: false,
                bulges: Vec::new(),
                widths: Vec::new()
            })
        );
    }

    #[test]
    fn a_polyline_extends_an_arc_end_segment_round_its_circle() {
        // A quarter circle of radius 5 about the origin from (5, 0) up to
        // (0, 5): bulge tan(90 degrees / 4), then a straight leg back down.
        let bulge = std::f64::consts::FRAC_PI_8.tan();
        let l = Entity::Polyline {
            points: vec![p(5.0, 0.0), p(0.0, 5.0)],
            closed: false,
            bulges: vec![bulge, 0.0],
            widths: Vec::new(),
        };
        // A wall at x = -3 meets the circle at (-3, 4): the end grows there.
        let Some(Entity::Polyline { points, bulges, .. }) = l.extended_to(End::End, &[wall(-3.0)])
        else {
            panic!("the arc end reaches the wall")
        };
        assert!(near(points[0], p(5.0, 0.0)), "the other end stays");
        assert!(near(points[1], p(-3.0, 4.0)));
        // The bulge now describes the longer arc: 90 degrees plus the extra
        // angle to (-3, 4).
        let total = (4.0_f64).atan2(-3.0);
        assert!(
            (bulges[0] - (total / 4.0).tan()).abs() < 1e-9,
            "{}",
            bulges[0]
        );
        // The start grows the other way round, clockwise.
        let Some(Entity::Polyline { points, .. }) = l.extended_to(End::Start, &[wall(2.0)]) else {
            panic!("the start reaches the wall")
        };
        assert!(near(points[0], p(2.0, -(21.0_f64).sqrt())));
    }

    #[test]
    fn closed_shapes_and_circles_do_not_extend() {
        let w = [wall(100.0)];
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        };
        let rect = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(1.0, 1.0),
        };
        let closed = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)],
            closed: true,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        for e in [circle, rect, closed] {
            assert_eq!(e.extended_to(End::End, &w), None);
        }
        assert!(near(p(0.0, 0.0), p(0.0, 0.0)));
    }

    #[test]
    fn an_elliptical_arc_grows_round_its_ellipse_to_the_nearest_wall() {
        let quarter = Ellipse::new(p(0.0, 0.0), p(4.0, 0.0), 0.5, 0.0, FRAC_PI_2).unwrap();
        // The end (at the top) runs on anticlockwise to the wall at x = -2.
        let walls = [wall(-2.0), wall(-3.0)];
        let Some(Entity::Ellipse(e)) = Entity::Ellipse(quarter).extended_to(End::End, &walls)
        else {
            panic!("an elliptical arc")
        };
        assert!(
            near(e.start_point(), quarter.start_point()),
            "the start stays"
        );
        assert!((e.end_point().x + 2.0).abs() < 1e-9 && e.end_point().y > 0.0);
        // Nothing ahead within a turn, and a full ellipse, do not extend.
        let far = [line(p(20.0, -1.0), p(20.0, 1.0))];
        assert_eq!(Entity::Ellipse(quarter).extended_to(End::End, &far), None);
        let whole = Ellipse::full(p(0.0, 0.0), p(4.0, 0.0), 0.5).unwrap();
        assert_eq!(Entity::Ellipse(whole).extended_to(End::End, &walls), None);
        // A line reaches an ellipse that is in its way.
        let l = line(p(-10.0, 1.0), p(-9.0, 1.0));
        let Some(Entity::Line { end, .. }) = l.extended_to(End::End, &[Entity::Ellipse(whole)])
        else {
            panic!("a line")
        };
        assert!(near(end, p(-4.0 * 0.75f64.sqrt(), 1.0)));
    }
}
