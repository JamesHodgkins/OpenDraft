//! An arc that leaves the end of a line or arc without a corner: the bend of
//! a pipe, the end of a slot, a smooth curve between two straights.
//!
//! The arc starts at the chosen end and heads on in the direction the shape
//! was going. `sweep` is how far it turns, in radians: positive turns left
//! (counter-clockwise), negative turns right.

use std::f64::consts::TAU;

use crate::{arc, Entity, Point};

/// Lengths below this are nothing.
const EPS: f64 = 1e-9;

impl Entity {
    /// The arc of `radius` that continues this line or arc from its end (or
    /// its start, when `at_end` is false), turning through `sweep`. `None`
    /// for any other kind, a radius that is not above zero, or a sweep of
    /// nothing or more than a full turn.
    pub fn tangent_arc(&self, at_end: bool, radius: f64, sweep: f64) -> Option<Entity> {
        if !(radius.is_finite() && radius > EPS && sweep.is_finite()) {
            return None;
        }
        if sweep.abs() <= EPS || sweep.abs() > TAU + EPS {
            return None;
        }
        let (from, heading) = leaving(self, at_end)?;
        // The centre is a radius to the side the arc turns toward.
        let turn = sweep.signum();
        let centre = Point::new(
            from.x - turn * heading.sin() * radius,
            from.y + turn * heading.cos() * radius,
        );
        let at = arc::angle_of(centre, from);
        let (start, end) = if sweep > 0.0 {
            (at, at + sweep)
        } else {
            (at + sweep, at)
        };
        Some(Entity::Arc {
            center: centre,
            radius,
            start: arc::normalize(start),
            end: arc::normalize(end),
        })
    }
}

/// The point at an end, and the direction (radians) a path leaving it by
/// that end goes in.
fn leaving(entity: &Entity, at_end: bool) -> Option<(Point, f64)> {
    match entity {
        Entity::Line { start, end } => {
            let (from, back) = if at_end {
                (*end, *start)
            } else {
                (*start, *end)
            };
            let (dx, dy) = (from.x - back.x, from.y - back.y);
            (dx.hypot(dy) > EPS).then(|| (from, dy.atan2(dx)))
        }
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => {
            // Going counter-clockwise the way is a quarter turn ahead of the
            // radius; leaving by the start means going the other way.
            let (angle, ahead) = if at_end {
                (*end, std::f64::consts::FRAC_PI_2)
            } else {
                (*start, -std::f64::consts::FRAC_PI_2)
            };
            Some((arc::point_at(*center, *radius, angle), angle + ahead))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn line(x0: f64, y0: f64, x1: f64, y1: f64) -> Entity {
        Entity::Line {
            start: Point::new(x0, y0),
            end: Point::new(x1, y1),
        }
    }

    fn parts(entity: Entity) -> (Point, f64, f64, f64) {
        match entity {
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => (center, radius, start, end),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_left_bend_off_the_end_of_a_line_turns_up() {
        // Heading right along y = 0, bending left through a quarter turn: the
        // centre is above the end, the arc runs from straight below it round
        // to the right of it.
        let (c, r, s, e) = parts(
            line(0.0, 0.0, 2000.0, 0.0)
                .tangent_arc(true, 229.0, FRAC_PI_2)
                .unwrap(),
        );
        assert!(close(c.x, 2000.0) && close(c.y, 229.0), "{c:?}");
        assert!(close(r, 229.0));
        assert!(close(s, 3.0 * FRAC_PI_2) && close(e, 0.0), "{s} {e}");
    }

    #[test]
    fn a_right_bend_turns_down() {
        let (c, _, s, e) = parts(
            line(0.0, 0.0, 10.0, 0.0)
                .tangent_arc(true, 5.0, -FRAC_PI_2)
                .unwrap(),
        );
        assert!(close(c.x, 10.0) && close(c.y, -5.0), "{c:?}");
        // From straight above the centre round to the right of it, clockwise.
        assert!(close(s, 0.0) && close(e, FRAC_PI_2), "{s} {e}");
    }

    #[test]
    fn the_arc_starts_at_the_end_and_is_tangent_to_the_line() {
        let l = line(0.0, 0.0, 3.0, 4.0);
        let (c, r, s, _) = parts(l.tangent_arc(true, 2.0, 1.0).unwrap());
        let start = arc::point_at(c, r, s);
        assert!(close(start.x, 3.0) && close(start.y, 4.0), "{start:?}");
        // The radius to the start is square to the line.
        let dot = (start.x - c.x) * 3.0 + (start.y - c.y) * 4.0;
        assert!(dot.abs() < 1e-9, "{dot}");
    }

    #[test]
    fn leaving_by_the_start_goes_backwards_along_the_line() {
        let (c, _, s, e) = parts(
            line(0.0, 0.0, 10.0, 0.0)
                .tangent_arc(false, 5.0, -FRAC_PI_2)
                .unwrap(),
        );
        // Heading left from the origin and turning right: up and over.
        assert!(close(c.x, 0.0) && close(c.y, 5.0), "{c:?}");
        let begin = arc::point_at(c, 5.0, e);
        assert!(close(begin.x, 0.0) && close(begin.y, 0.0), "{begin:?} {s}");
    }

    #[test]
    fn it_continues_an_arc_without_a_corner() {
        // A quarter circle about the origin ending at (0, 10), going left.
        let a = Entity::Arc {
            center: Point::new(0.0, 0.0),
            radius: 10.0,
            start: 0.0,
            end: FRAC_PI_2,
        };
        // Bending the other way (right) with the same radius makes an S.
        let (c, _, _, _) = parts(a.tangent_arc(true, 10.0, -PI).unwrap());
        assert!(close(c.x, 0.0) && close(c.y, 20.0), "{c:?}");
    }

    #[test]
    fn leaving_an_arc_by_its_start_goes_back_the_way_it_came() {
        // A quarter circle about the origin from (10, 0) round to (0, 10). Its
        // start is at (10, 0) and leaving there heads straight down. Turning
        // left from heading down puts the centre to the right of the start.
        let a = Entity::Arc {
            center: Point::new(0.0, 0.0),
            radius: 10.0,
            start: 0.0,
            end: FRAC_PI_2,
        };
        let (c, _, _, _) = parts(a.tangent_arc(false, 5.0, FRAC_PI_2).unwrap());
        assert!(close(c.x, 15.0) && close(c.y, 0.0), "{c:?}");
    }

    #[test]
    fn nonsense_is_refused() {
        let l = line(0.0, 0.0, 1.0, 0.0);
        assert!(l.tangent_arc(true, 0.0, 1.0).is_none());
        assert!(l.tangent_arc(true, -1.0, 1.0).is_none());
        assert!(l.tangent_arc(true, 1.0, 0.0).is_none());
        assert!(l.tangent_arc(true, 1.0, 7.0).is_none());
        assert!(l.tangent_arc(true, f64::NAN, 1.0).is_none());
        assert!(line(1.0, 1.0, 1.0, 1.0)
            .tangent_arc(true, 1.0, 1.0)
            .is_none());
        let circle = Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 1.0,
        };
        assert!(circle.tangent_arc(true, 1.0, 1.0).is_none());
    }
}
