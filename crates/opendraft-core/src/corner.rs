//! Joining two lines at a corner with a cut (chamfer), and the types FILLET
//! shares: what went wrong ([`CornerError`]) and what came out ([`Joined`]).
//! The arc version is in `fillet.rs`; whole polylines are in `polycorner.rs`.
//!
//! Both lines are extended or cut back so that they meet the joint. Each
//! line keeps the part on the side of the point the user picked on it, so
//! clicking near the end you want to keep decides which way the corner opens.

use std::fmt;

use crate::intersect::line_line;
use crate::{Entity, Point};

/// Lengths below this are nothing.
const EPS: f64 = 1e-9;

/// Why two objects cannot be joined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CornerError {
    /// A chamfer needs lines (or the segments of a polyline).
    NotLines,
    /// The lines run side by side and never meet.
    Parallel,
    /// A line is too short for the radius or distances asked for.
    TooLong,
    /// Objects of a kind that cannot be joined this way.
    Unsupported,
    /// No arc of that radius touches both objects.
    NoFit,
    /// Two segments of a polyline that do not meet at a vertex.
    NotAdjacent,
}

impl fmt::Display for CornerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CornerError::NotLines => "A chamfer needs lines or polyline segments.",
            CornerError::Parallel => "The lines are parallel.",
            CornerError::TooLong => "The radius or distance is too large for these objects.",
            CornerError::Unsupported => "Those objects cannot be joined at a corner.",
            CornerError::NoFit => "No arc of that radius fits between those objects.",
            CornerError::NotAdjacent => "Select two adjacent segments of the polyline.",
        })
    }
}

/// The two objects as they end up, and what joins them (`None` when they
/// simply meet at their corner).
#[derive(Clone, Debug, PartialEq)]
pub struct Joined {
    pub a: Entity,
    pub b: Entity,
    pub joint: Option<Entity>,
}

/// How a chamfer is measured.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Chamfer {
    /// This far from the corner along the first line, that far along the
    /// second.
    Distances(f64, f64),
    /// This far along the first line, and the cut leaves it at this angle
    /// (radians) to that line.
    Angle { length: f64, angle: f64 },
}

impl Chamfer {
    /// The distances from the corner along each of two lines that meet at
    /// `corner` radians. `None` when the angle leaves no triangle.
    pub fn distances(self, corner: f64) -> Option<(f64, f64)> {
        match self {
            Chamfer::Distances(a, b) => Some((a.max(0.0), b.max(0.0))),
            Chamfer::Angle { length, angle } => {
                // The triangle of corner, first point and second point has the
                // corner angle at the corner and `angle` at the first point.
                let third = std::f64::consts::PI - corner - angle;
                (length.is_finite() && angle > EPS && third > EPS)
                    .then(|| (length.max(0.0), length * angle.sin() / third.sin()))
            }
        }
    }
}

/// One line's part in a corner.
struct Arm {
    /// Unit vector from the corner along the part that is kept.
    along: Point,
    /// How far that part reaches from the corner.
    reach: f64,
    /// The line with its corner-side end moved to a given point.
    keep: Box<dyn Fn(Point) -> Entity>,
}

struct Corner {
    at: Point,
    a: Arm,
    b: Arm,
    /// The angle between the two kept parts, in (0, π).
    angle: f64,
}

fn unit(from: Point, to: Point) -> Option<Point> {
    let len = (to.x - from.x).hypot(to.y - from.y);
    (len > EPS).then(|| Point::new((to.x - from.x) / len, (to.y - from.y) / len))
}

fn dot(a: Point, b: Point) -> f64 {
    a.x * b.x + a.y * b.y
}

fn arm(entity: &Entity, at: Point, picked: Point) -> Result<Arm, CornerError> {
    let Entity::Line { start, end } = entity else {
        return Err(CornerError::NotLines);
    };
    let (start, end) = (*start, *end);
    let direction = unit(start, end).ok_or(CornerError::NotLines)?;
    let offset = |p: Point| Point::new(p.x - at.x, p.y - at.y);
    // Keep the side of the corner the pick is on.
    let side = if dot(offset(picked), direction) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let along = Point::new(direction.x * side, direction.y * side);
    // The end of the line out on that side stays where it is.
    let keeps_end = dot(offset(end), along) >= dot(offset(start), along);
    let far = if keeps_end { end } else { start };
    Ok(Arm {
        along,
        reach: dot(offset(far), along),
        keep: Box::new(move |near| {
            if keeps_end {
                Entity::Line {
                    start: near,
                    end: far,
                }
            } else {
                Entity::Line {
                    start: far,
                    end: near,
                }
            }
        }),
    })
}

fn corner(a: &Entity, b: &Entity, picked_a: Point, picked_b: Point) -> Result<Corner, CornerError> {
    let (Entity::Line { start: a0, end: a1 }, Entity::Line { start: b0, end: b1 }) = (a, b) else {
        return Err(CornerError::NotLines);
    };
    let at = line_line(*a0, *a1, *b0, *b1)
        .first()
        .copied()
        .ok_or(CornerError::Parallel)?;
    let (arm_a, arm_b) = (arm(a, at, picked_a)?, arm(b, at, picked_b)?);
    let angle = dot(arm_a.along, arm_b.along).clamp(-1.0, 1.0).acos();
    Ok(Corner {
        at,
        a: arm_a,
        b: arm_b,
        angle,
    })
}

fn point_along(from: Point, direction: Point, distance: f64) -> Point {
    Point::new(
        from.x + direction.x * distance,
        from.y + direction.y * distance,
    )
}

/// Cut the corner of lines `a` and `b` with a straight line. Zero distances
/// bring the lines together at the corner.
pub fn chamfer_lines(
    a: &Entity,
    b: &Entity,
    chamfer: Chamfer,
    picked_a: Point,
    picked_b: Point,
) -> Result<Joined, CornerError> {
    let c = corner(a, b, picked_a, picked_b)?;
    let (da, db) = chamfer.distances(c.angle).ok_or(CornerError::TooLong)?;
    if da <= EPS && db <= EPS {
        return Ok(Joined {
            a: (c.a.keep)(c.at),
            b: (c.b.keep)(c.at),
            joint: None,
        });
    }
    if !(da.is_finite() && db.is_finite()) || da > c.a.reach - EPS || db > c.b.reach - EPS {
        return Err(CornerError::TooLong);
    }
    let (ta, tb) = (
        point_along(c.at, c.a.along, da),
        point_along(c.at, c.b.along, db),
    );
    Ok(Joined {
        a: (c.a.keep)(ta),
        b: (c.b.keep)(tb),
        joint: Some(Entity::Line { start: ta, end: tb }),
    })
}

#[cfg(test)]
mod tests {
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

    fn ends(e: &Entity) -> (Point, Point) {
        match e {
            Entity::Line { start, end } => (*start, *end),
            other => panic!("expected a line, got {other:?}"),
        }
    }

    fn has_ends(e: &Entity, a: Point, b: Point) -> bool {
        let (start, end) = ends(e);
        near(start, a) && near(end, b)
    }

    #[test]
    fn a_chamfer_cuts_the_corner_at_the_two_distances() {
        let a = line(p(0.0, 0.0), p(10.0, 0.0));
        let b = line(p(0.0, 0.0), p(0.0, 10.0));
        let d = Chamfer::Distances(2.0, 3.0);
        let done = chamfer_lines(&a, &b, d, p(5.0, 0.0), p(0.0, 5.0)).unwrap();
        assert!(has_ends(&done.a, p(2.0, 0.0), p(10.0, 0.0)));
        assert!(has_ends(&done.b, p(0.0, 3.0), p(0.0, 10.0)));
        assert!(matches!(&done.joint, Some(j) if has_ends(j, p(2.0, 0.0), p(0.0, 3.0))));
    }

    #[test]
    fn zero_distances_meet_at_the_corner_and_too_long_is_refused() {
        let a = line(p(0.0, 0.0), p(10.0, 0.0));
        let b = line(p(0.0, 0.0), p(0.0, 10.0));
        let zero = Chamfer::Distances(0.0, 0.0);
        let corner = chamfer_lines(&a, &b, zero, p(5.0, 0.0), p(0.0, 5.0)).unwrap();
        assert_eq!(corner.joint, None);
        let long = Chamfer::Distances(11.0, 1.0);
        assert_eq!(
            chamfer_lines(&a, &b, long, p(5.0, 0.0), p(0.0, 5.0)),
            Err(CornerError::TooLong)
        );
        let parallel = line(p(0.0, 3.0), p(10.0, 3.0));
        let one = Chamfer::Distances(1.0, 1.0);
        assert_eq!(
            chamfer_lines(&a, &parallel, one, p(5.0, 0.0), p(5.0, 3.0)),
            Err(CornerError::Parallel)
        );
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        };
        assert_eq!(
            chamfer_lines(&a, &circle, one, p(5.0, 0.0), p(1.0, 0.0)),
            Err(CornerError::NotLines)
        );
    }

    #[test]
    fn a_length_and_an_angle_make_the_other_distance() {
        // A right angle, 3 along the first line, the cut leaving it at 45
        // degrees: an isosceles triangle, so 3 along the other too.
        let a = line(p(0.0, 0.0), p(10.0, 0.0));
        let b = line(p(0.0, 0.0), p(0.0, 10.0));
        let by_angle = Chamfer::Angle {
            length: 3.0,
            angle: std::f64::consts::FRAC_PI_4,
        };
        let done = chamfer_lines(&a, &b, by_angle, p(5.0, 0.0), p(0.0, 5.0)).unwrap();
        assert!(matches!(&done.joint, Some(j) if has_ends(j, p(3.0, 0.0), p(0.0, 3.0))));
        // 60 degrees leaves a longer second distance: 3 * sin 60 / sin 30.
        let steeper = Chamfer::Angle {
            length: 3.0,
            angle: std::f64::consts::FRAC_PI_3,
        };
        let (da, db) = steeper.distances(std::f64::consts::FRAC_PI_2).unwrap();
        assert!((da - 3.0).abs() < 1e-9 && (db - 3.0 * 3.0_f64.sqrt()).abs() < 1e-9);
        // An angle that leaves no triangle is refused.
        let flat = Chamfer::Angle {
            length: 3.0,
            angle: 2.0,
        };
        assert_eq!(flat.distances(std::f64::consts::FRAC_PI_2), None);
        assert_eq!(
            chamfer_lines(&a, &b, flat, p(5.0, 0.0), p(0.0, 5.0)),
            Err(CornerError::TooLong)
        );
    }

    #[test]
    fn the_picked_side_and_the_drawing_direction_do_not_change_the_corner() {
        let a = line(p(10.0, 0.0), p(0.0, 0.0));
        let b = line(p(0.0, 0.0), p(0.0, 10.0));
        let one = Chamfer::Distances(1.0, 1.0);
        let done = chamfer_lines(&a, &b, one, p(5.0, 0.0), p(0.0, 5.0)).unwrap();
        assert!(
            has_ends(&done.a, p(10.0, 0.0), p(1.0, 0.0)),
            "keeps its direction"
        );
        // The same cross, joined in another quadrant.
        let a = line(p(-10.0, 0.0), p(10.0, 0.0));
        let b = line(p(0.0, -10.0), p(0.0, 10.0));
        let two = Chamfer::Distances(2.0, 2.0);
        let done = chamfer_lines(&a, &b, two, p(-5.0, 0.0), p(0.0, 5.0)).unwrap();
        assert!(has_ends(&done.a, p(-10.0, 0.0), p(-2.0, 0.0)));
        assert!(has_ends(&done.b, p(0.0, 2.0), p(0.0, 10.0)));
    }

    #[test]
    fn lines_that_do_not_reach_are_extended() {
        let a = line(p(0.0, 0.0), p(5.0, 0.0));
        let b = line(p(8.0, -6.0), p(8.0, 4.0));
        let two = Chamfer::Distances(2.0, 2.0);
        let done = chamfer_lines(&a, &b, two, p(3.0, 0.0), p(8.0, -4.0)).unwrap();
        assert!(
            has_ends(&done.a, p(0.0, 0.0), p(6.0, 0.0)),
            "extended to the cut"
        );
        assert!(has_ends(&done.b, p(8.0, -6.0), p(8.0, -2.0)));
    }
}
