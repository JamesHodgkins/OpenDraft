//! Rounding the corner between two curves with an arc: a line, an arc, a
//! circle or an ellipse on either side (ellipses in `fillet_ellipse`). The
//! base for FILLET.
//!
//! The arc is a circle of the given radius that touches both curves. Such
//! circles are found by moving each curve sideways by the radius and meeting
//! the results; of the several that exist, the one whose touching points are
//! nearest the points the user picked is the one wanted. Each curve then
//! keeps the part on the side it was picked, cut back or extended to its
//! touching point. A circle is never trimmed.

use std::f64::consts::{PI, TAU};

use crate::corner::{CornerError, Joined};
use crate::intersect::{circle_circle, line_circle, line_line};
use crate::{arc, Entity, Point};

const EPS: f64 = 1e-9;

/// The whole line or circle a curve lies on.
#[derive(Clone, Copy)]
enum Carrier {
    Line { from: Point, along: Point },
    Circle { center: Point, radius: f64 },
}

fn dist(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn carrier(entity: &Entity) -> Option<Carrier> {
    match entity {
        Entity::Line { start, end } => {
            let len = dist(*start, *end);
            (len > EPS).then(|| Carrier::Line {
                from: *start,
                along: Point::new((end.x - start.x) / len, (end.y - start.y) / len),
            })
        }
        Entity::Arc { center, radius, .. } | Entity::Circle { center, radius } => {
            (radius.abs() > EPS).then(|| Carrier::Circle {
                center: *center,
                radius: radius.abs(),
            })
        }
        _ => None,
    }
}

/// The curves a circle of `radius` could touch from outside or inside: the
/// carrier moved sideways by the radius, each way.
fn parallels(c: Carrier, radius: f64) -> Vec<Carrier> {
    match c {
        Carrier::Line { from, along } => {
            let n = Point::new(-along.y * radius, along.x * radius);
            vec![
                Carrier::Line {
                    from: Point::new(from.x + n.x, from.y + n.y),
                    along,
                },
                Carrier::Line {
                    from: Point::new(from.x - n.x, from.y - n.y),
                    along,
                },
            ]
        }
        Carrier::Circle { center, radius: r } => {
            let mut found = vec![Carrier::Circle {
                center,
                radius: r + radius,
            }];
            if (r - radius).abs() > EPS {
                found.push(Carrier::Circle {
                    center,
                    radius: (r - radius).abs(),
                });
            }
            found
        }
    }
}

fn meet(a: Carrier, b: Carrier) -> Vec<Point> {
    let through = |from: Point, along: Point| Point::new(from.x + along.x, from.y + along.y);
    match (a, b) {
        (
            Carrier::Line { from, along },
            Carrier::Line {
                from: f2,
                along: a2,
            },
        ) => line_line(from, through(from, along), f2, through(f2, a2)),
        (Carrier::Line { from, along }, Carrier::Circle { center, radius })
        | (Carrier::Circle { center, radius }, Carrier::Line { from, along }) => {
            line_circle(from, through(from, along), center, radius)
        }
        (
            Carrier::Circle { center, radius },
            Carrier::Circle {
                center: c2,
                radius: r2,
            },
        ) => circle_circle(center, radius, c2, r2),
    }
}

/// Where a circle centred at `center` touches the carrier.
fn touching(c: Carrier, center: Point) -> Option<Point> {
    match c {
        Carrier::Line { from, along } => {
            let t = (center.x - from.x) * along.x + (center.y - from.y) * along.y;
            Some(Point::new(from.x + along.x * t, from.y + along.y * t))
        }
        Carrier::Circle {
            center: c, radius, ..
        } => {
            let len = dist(c, center);
            (len > EPS).then(|| {
                Point::new(
                    c.x + (center.x - c.x) / len * radius,
                    c.y + (center.y - c.y) / len * radius,
                )
            })
        }
    }
}

/// The part of `entity` that stays when it is cut (or extended) at `at`, on
/// the side of `picked`.
pub(crate) fn keep_part(entity: &Entity, at: Point, picked: Point) -> Result<Entity, CornerError> {
    match entity {
        Entity::Line { start, end } => {
            let len = dist(*start, *end);
            let u = Point::new((end.x - start.x) / len, (end.y - start.y) / len);
            let along = |p: Point| (p.x - start.x) * u.x + (p.y - start.y) * u.y;
            let (t, pick) = (along(at), along(picked));
            if pick >= t {
                if t >= len - EPS {
                    return Err(CornerError::TooLong);
                }
                Ok(Entity::Line {
                    start: at,
                    end: *end,
                })
            } else {
                if t <= EPS {
                    return Err(CornerError::TooLong);
                }
                Ok(Entity::Line {
                    start: *start,
                    end: at,
                })
            }
        }
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => {
            let sweep = arc::sweep(*start, *end);
            let cut = arc::angle_of(*center, at);
            let from_start = (cut - start).rem_euclid(TAU);
            // How far round from the start the cut is; before the start counts
            // as negative so an extension either way is told from a point on
            // the arc.
            let t = if from_start <= sweep || from_start - sweep < TAU - from_start {
                from_start
            } else {
                from_start - TAU
            };
            let pick = (arc::angle_of(*center, picked) - start).rem_euclid(TAU);
            if pick >= t {
                if t >= sweep - EPS {
                    return Err(CornerError::TooLong);
                }
                Ok(Entity::Arc {
                    center: *center,
                    radius: *radius,
                    start: cut,
                    end: *end,
                })
            } else {
                if t <= EPS {
                    return Err(CornerError::TooLong);
                }
                Ok(Entity::Arc {
                    center: *center,
                    radius: *radius,
                    start: *start,
                    end: cut,
                })
            }
        }
        // A circle has no ends to cut back.
        other => Ok(other.clone()),
    }
}

/// Round the corner of `a` and `b` (lines, arcs or circles) with an arc of
/// `radius`; with a radius of zero the two meet at a point instead. `picked_a`
/// and `picked_b` are where each was clicked.
pub fn fillet(
    a: &Entity,
    b: &Entity,
    radius: f64,
    picked_a: Point,
    picked_b: Point,
) -> Result<Joined, CornerError> {
    if matches!(a, Entity::Ellipse(_)) || matches!(b, Entity::Ellipse(_)) {
        return crate::fillet_ellipse::fillet_ellipse(a, b, radius, picked_a, picked_b);
    }
    let (Some(ca), Some(cb)) = (carrier(a), carrier(b)) else {
        return Err(CornerError::Unsupported);
    };
    let round = radius.is_finite() && radius > EPS;
    // Pairs of touching points, one on each carrier, with the centre of the
    // circle that makes them (the same point twice for a plain corner).
    let mut options: Vec<(Point, Point, Point)> = Vec::new();
    if round {
        for pa in parallels(ca, radius) {
            for pb in parallels(cb, radius) {
                for center in meet(pa, pb) {
                    if let (Some(ta), Some(tb)) = (touching(ca, center), touching(cb, center)) {
                        if dist(ta, tb) > EPS {
                            options.push((ta, tb, center));
                        }
                    }
                }
            }
        }
    } else {
        for corner in meet(ca, cb) {
            options.push((corner, corner, corner));
        }
    }
    if options.is_empty() {
        let both_lines = matches!((ca, cb), (Carrier::Line { .. }, Carrier::Line { .. }));
        return Err(if both_lines {
            CornerError::Parallel
        } else {
            CornerError::NoFit
        });
    }
    let score = |o: &(Point, Point, Point)| dist(o.0, picked_a) + dist(o.1, picked_b);
    let (ta, tb, center) = options
        .into_iter()
        .min_by(|x, y| score(x).total_cmp(&score(y)))
        .ok_or(CornerError::NoFit)?;

    let new_a = keep_part(a, ta, picked_a)?;
    let new_b = keep_part(b, tb, picked_b)?;
    Ok(Joined {
        a: new_a,
        b: new_b,
        joint: round.then(|| joint_arc(center, radius, ta, tb)),
    })
}

/// The fillet arc about `center` between the touching points, the short way
/// round.
pub(crate) fn joint_arc(center: Point, radius: f64, ta: Point, tb: Point) -> Entity {
    let (from, to) = (arc::angle_of(center, ta), arc::angle_of(center, tb));
    let turn = (to - from + PI).rem_euclid(TAU) - PI;
    let (start, end) = if turn >= 0.0 { (from, to) } else { (to, from) };
    Entity::Arc {
        center,
        radius,
        start,
        end,
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn near(a: Point, b: Point) -> bool {
        dist(a, b) < 1e-9
    }

    fn arc_entity(c: Point, r: f64, start: f64, end: f64) -> Entity {
        Entity::Arc {
            center: c,
            radius: r,
            start,
            end,
        }
    }

    fn joint(done: &Joined) -> (Point, f64, f64, f64) {
        match &done.joint {
            Some(Entity::Arc {
                center,
                radius,
                start,
                end,
            }) => (*center, *radius, *start, *end),
            other => panic!("expected an arc, got {other:?}"),
        }
    }

    /// The fillet arc touches both curves: the centre is a radius from each
    /// and the arc's ends lie on them.
    fn assert_tangent(a: &Entity, b: &Entity, done: &Joined) {
        let (center, radius, start, end) = joint(done);
        let ends = [
            arc::point_at(center, radius, start),
            arc::point_at(center, radius, end),
        ];
        for curve in [a, b] {
            // A circle or arc is touched from outside or inside: the
            // distance between centres is R + r or |R - r|.
            let touches = ends.iter().any(|e| curve.distance_to(*e) < 1e-6);
            assert!(touches, "an end of the arc is on {curve:?}");
        }
        assert!(ends
            .iter()
            .all(|e| (dist(*e, center) - radius).abs() < 1e-9));
    }

    #[test]
    fn two_lines_at_a_right_angle() {
        let a = line(p(0.0, 0.0), p(10.0, 0.0));
        let b = line(p(0.0, 0.0), p(0.0, 10.0));
        let done = fillet(&a, &b, 1.0, p(5.0, 0.0), p(0.0, 5.0)).unwrap();
        let Entity::Line { start, .. } = done.a else {
            panic!()
        };
        assert!(near(start, p(1.0, 0.0)));
        let (center, radius, start, end) = joint(&done);
        assert!(near(center, p(1.0, 1.0)) && (radius - 1.0).abs() < 1e-9);
        assert!((start - PI).abs() < 1e-9 && (end - 3.0 * FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn the_picked_quadrant_decides_which_corner_is_rounded() {
        let a = line(p(-10.0, 0.0), p(10.0, 0.0));
        let b = line(p(0.0, -10.0), p(0.0, 10.0));
        let done = fillet(&a, &b, 1.0, p(-5.0, 0.0), p(0.0, 5.0)).unwrap();
        assert!(near(joint(&done).0, p(-1.0, 1.0)));
        let done = fillet(&a, &b, 1.0, p(5.0, 0.0), p(0.0, -5.0)).unwrap();
        assert!(near(joint(&done).0, p(1.0, -1.0)));
    }

    #[test]
    fn a_line_and_an_arc_join_with_a_tangent_arc() {
        // A quarter arc of radius 10 about the origin, east to north, and a
        // wall at x = 12. A circle of radius 3 outside the arc and left of
        // the wall has its centre at (9, sqrt(88)).
        let quarter = arc_entity(p(0.0, 0.0), 10.0, 0.0, FRAC_PI_2);
        let wall = line(p(12.0, 0.0), p(12.0, 15.0));
        let done = fillet(&quarter, &wall, 3.0, p(6.0, 8.0), p(12.0, 12.0)).unwrap();
        assert_tangent(&quarter, &wall, &done);
        let rise = 88.0_f64.sqrt();
        let (center, ..) = joint(&done);
        assert!(near(center, p(9.0, rise)));
        // The arc keeps its upper part from the touching point to the top.
        let cut = (rise).atan2(9.0);
        let Entity::Arc { start, end, .. } = done.a else {
            panic!("an arc stays an arc")
        };
        assert!((start - cut).abs() < 1e-9 && (end - FRAC_PI_2).abs() < 1e-9);
        // The wall keeps what is above its touching point.
        assert!(matches!(done.b, Entity::Line { start, end }
            if near(start, p(12.0, rise)) && near(end, p(12.0, 15.0))));
    }

    #[test]
    fn two_circles_are_joined_but_not_cut() {
        let a = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 5.0,
        };
        let b = Entity::Circle {
            center: p(14.0, 0.0),
            radius: 5.0,
        };
        let done = fillet(&a, &b, 3.0, p(5.0, 0.0), p(9.0, 0.0)).unwrap();
        assert_eq!(done.a, a, "a circle is left whole");
        assert_eq!(done.b, b);
        let (center, radius, ..) = joint(&done);
        assert!((radius - 3.0).abs() < 1e-9);
        // Externally tangent to both: centre 8 from each circle's centre.
        assert!((dist(center, p(0.0, 0.0)) - 8.0).abs() < 1e-9);
        assert!((dist(center, p(14.0, 0.0)) - 8.0).abs() < 1e-9);
        assert_tangent(&a, &b, &done);
    }

    #[test]
    fn a_line_and_a_circle_pick_the_nearest_of_several_fillets() {
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 5.0,
        };
        let floor = line(p(-20.0, -8.0), p(20.0, -8.0));
        // Pick the right-hand side of both: the fillet is on the right.
        let right = fillet(&circle, &floor, 2.0, p(5.0, 0.0), p(12.0, -8.0)).unwrap();
        assert!(joint(&right).0.x > 0.0);
        let left = fillet(&circle, &floor, 2.0, p(-5.0, 0.0), p(-12.0, -8.0)).unwrap();
        assert!(joint(&left).0.x < 0.0);
        assert_tangent(&circle, &floor, &right);
        // The floor keeps its right-hand part, from the touching point out.
        let Entity::Line { start, end } = right.b else {
            panic!()
        };
        assert!(start.x > 0.0 && near(end, p(20.0, -8.0)));
    }

    #[test]
    fn radius_zero_makes_curves_meet_where_they_cross() {
        let a = line(p(0.0, 0.0), p(5.0, 0.0));
        let b = line(p(8.0, -6.0), p(8.0, 4.0));
        let done = fillet(&a, &b, 0.0, p(3.0, 0.0), p(8.0, -4.0)).unwrap();
        assert_eq!(done.joint, None);
        assert_eq!(done.a, line(p(0.0, 0.0), p(8.0, 0.0)));
        assert_eq!(done.b, line(p(8.0, -6.0), p(8.0, 0.0)));
        // A line and a circle: the line is cut where it meets the circle.
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 5.0,
        };
        let spoke = line(p(0.0, 0.0), p(20.0, 0.0));
        let done = fillet(&spoke, &circle, 0.0, p(15.0, 0.0), p(5.0, 0.0)).unwrap();
        assert!(matches!(done.a, Entity::Line { start, .. } if near(start, p(5.0, 0.0))));
    }

    #[test]
    fn a_radius_larger_than_the_lines_extends_them_to_the_arc() {
        let a = line(p(0.0, 0.0), p(10.0, 0.0));
        let b = line(p(0.0, 0.0), p(0.0, 10.0));
        let done = fillet(&a, &b, 20.0, p(5.0, 0.0), p(0.0, 5.0)).unwrap();
        assert_eq!(done.a, line(p(0.0, 0.0), p(20.0, 0.0)));
        assert_eq!(done.b, line(p(0.0, 0.0), p(0.0, 20.0)));
        assert!(near(joint(&done).0, p(20.0, 20.0)));
    }

    #[test]
    fn what_cannot_be_joined_is_explained() {
        let a = line(p(0.0, 0.0), p(10.0, 0.0));
        let parallel = line(p(0.0, 3.0), p(10.0, 3.0));
        assert_eq!(
            fillet(&a, &parallel, 1.0, p(5.0, 0.0), p(5.0, 3.0)),
            Err(CornerError::Parallel)
        );
        let rectangle = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(1.0, 1.0),
        };
        assert_eq!(
            fillet(&a, &rectangle, 1.0, p(5.0, 0.0), p(0.0, 0.5)),
            Err(CornerError::Unsupported)
        );
        // Circles too far apart for the radius to bridge them.
        let far = Entity::Circle {
            center: p(100.0, 0.0),
            radius: 1.0,
        };
        let near_circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        };
        assert_eq!(
            fillet(&near_circle, &far, 0.1, p(1.0, 0.0), p(99.0, 0.0)),
            Err(CornerError::NoFit)
        );
    }
}
