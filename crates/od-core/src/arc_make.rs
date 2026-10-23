//! Building an arc from the parts AutoCAD's ARC command offers: centre, start,
//! end, included angle, chord length, radius and start direction. Every
//! function gives the arc as an [`Entity`], which always runs counter-clockwise
//! (so a clockwise arc comes back with its ends swapped), or `None` when the
//! parts make no arc.

use std::f64::consts::TAU;

use crate::arc::{angle_of, normalize};
use crate::{Entity, Point};

const EPS: f64 = 1e-9;

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn arc(center: Point, radius: f64, start: f64, end: f64) -> Entity {
    Entity::Arc {
        center,
        radius,
        start,
        end,
    }
}

/// The left-hand unit normal of the way from `a` to `b`.
fn left_of(a: Point, b: Point, length: f64) -> (f64, f64) {
    (-(b.y - a.y) / length, (b.x - a.x) / length)
}

/// The arc about `center` that starts at `start` and runs counter-clockwise to
/// the direction of `toward`.
pub fn from_center_start_end(center: Point, start: Point, toward: Point) -> Option<Entity> {
    let radius = distance(center, start);
    (radius > EPS && distance(center, toward) > EPS).then(|| {
        arc(
            center,
            radius,
            angle_of(center, start),
            angle_of(center, toward),
        )
    })
}

/// The arc about `center` from `start` through the included `angle` (radians):
/// counter-clockwise when positive, clockwise when negative.
pub fn from_center_start_angle(center: Point, start: Point, angle: f64) -> Option<Entity> {
    let radius = distance(center, start);
    if radius <= EPS || angle.abs() <= EPS || angle.abs() >= TAU - EPS {
        return None;
    }
    let from = angle_of(center, start);
    Some(if angle > 0.0 {
        arc(center, radius, from, normalize(from + angle))
    } else {
        arc(center, radius, normalize(from + angle), from)
    })
}

/// The arc about `center` from `start` whose chord is `chord` long: the
/// shorter arc counter-clockwise for a positive length, the longer one for a
/// negative length.
pub fn from_center_start_chord(center: Point, start: Point, chord: f64) -> Option<Entity> {
    let radius = distance(center, start);
    if radius <= EPS || chord.abs() <= EPS || chord.abs() > 2.0 * radius + EPS {
        return None;
    }
    let minor = 2.0 * (chord.abs() / (2.0 * radius)).clamp(-1.0, 1.0).asin();
    from_center_start_angle(center, start, if chord > 0.0 { minor } else { TAU - minor })
}

/// The arc from `start` to `end` with the given `radius`, counter-clockwise:
/// the shorter arc for a positive radius, the longer one for a negative one.
pub fn from_start_end_radius(start: Point, end: Point, radius: f64) -> Option<Entity> {
    let chord = distance(start, end);
    if chord <= EPS || radius.abs() * 2.0 < chord - EPS {
        return None;
    }
    let r = radius.abs();
    let height = (r * r - chord * chord / 4.0).max(0.0).sqrt();
    // The centre of a short counter-clockwise arc is left of start -> end.
    let left = left_of(start, end, chord);
    let side = if radius > 0.0 { height } else { -height };
    let center = Point::new(
        (start.x + end.x) / 2.0 + left.0 * side,
        (start.y + end.y) / 2.0 + left.1 * side,
    );
    Some(arc(
        center,
        r,
        angle_of(center, start),
        angle_of(center, end),
    ))
}

/// The arc from `start` to `end` that sweeps the included `angle`:
/// counter-clockwise from `start` when positive, clockwise when negative.
pub fn from_start_end_angle(start: Point, end: Point, angle: f64) -> Option<Entity> {
    if angle < 0.0 {
        return from_start_end_angle(end, start, -angle);
    }
    let chord = distance(start, end);
    if chord <= EPS || angle <= EPS || angle >= TAU - EPS {
        return None;
    }
    let left = left_of(start, end, chord);
    // Past a half circle the centre moves to the other side of the chord
    // (the tangent turns negative), so one formula serves both.
    let offset = chord / (2.0 * (angle / 2.0).tan());
    let center = Point::new(
        (start.x + end.x) / 2.0 + left.0 * offset,
        (start.y + end.y) / 2.0 + left.1 * offset,
    );
    Some(arc(
        center,
        distance(center, start),
        angle_of(center, start),
        angle_of(center, end),
    ))
}

/// The arc from `start` to `end` that leaves `start` heading `direction`
/// (radians). It curves towards `end`, so it can turn either way.
pub fn from_start_end_direction(start: Point, end: Point, direction: f64) -> Option<Entity> {
    let v = (start.x - end.x, start.y - end.y);
    let normal = (-direction.sin(), direction.cos());
    let along = normal.0 * v.0 + normal.1 * v.1;
    if distance(start, end) <= EPS || along.abs() <= EPS {
        return None;
    }
    // The centre is on the normal at `start`, as far from `end` as from `start`.
    let signed = -(v.0 * v.0 + v.1 * v.1) / (2.0 * along);
    let center = Point::new(start.x + normal.0 * signed, start.y + normal.1 * signed);
    let radius = signed.abs();
    let (a, b) = (angle_of(center, start), angle_of(center, end));
    // The centre on the left of the heading means a counter-clockwise turn.
    Some(if signed > 0.0 {
        arc(center, radius, a, b)
    } else {
        arc(center, radius, b, a)
    })
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;
    use crate::arc::{point_at, sweep};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn parts(e: Entity) -> (Point, f64, f64, f64) {
        let Entity::Arc {
            center,
            radius,
            start,
            end,
        } = e
        else {
            panic!("an arc")
        };
        (center, radius, start, end)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// Equal as directions: 0 and a full turn are the same.
    fn same_angle(a: f64, b: f64) -> bool {
        close((a - b + PI).rem_euclid(TAU), PI)
    }

    fn near(a: Point, b: Point) -> bool {
        distance(a, b) < 1e-9
    }

    #[test]
    fn centre_start_end_runs_counter_clockwise_to_the_end_direction() {
        let e = from_center_start_end(p(0.0, 0.0), p(5.0, 0.0), p(0.0, 9.0)).unwrap();
        let (c, r, s, en) = parts(e);
        assert_eq!(c, p(0.0, 0.0));
        assert!(close(r, 5.0) && same_angle(s, 0.0) && same_angle(en, FRAC_PI_2));
        assert!(from_center_start_end(p(0.0, 0.0), p(0.0, 0.0), p(1.0, 0.0)).is_none());
    }

    #[test]
    fn included_angle_sweeps_either_way_from_the_start() {
        let up = from_center_start_angle(p(0.0, 0.0), p(5.0, 0.0), FRAC_PI_2).unwrap();
        let (_, _, s, en) = parts(up);
        assert!(same_angle(s, 0.0) && same_angle(en, FRAC_PI_2));
        let down = from_center_start_angle(p(0.0, 0.0), p(5.0, 0.0), -FRAC_PI_2).unwrap();
        let (c, r, s, en) = parts(down);
        assert!(close(sweep(s, en), FRAC_PI_2));
        assert!(
            near(point_at(c, r, en), p(5.0, 0.0)),
            "it ends where it started"
        );
        assert!(from_center_start_angle(p(0.0, 0.0), p(5.0, 0.0), 0.0).is_none());
        assert!(from_center_start_angle(p(0.0, 0.0), p(5.0, 0.0), TAU).is_none());
    }

    #[test]
    fn chord_length_gives_the_short_arc_or_the_long_one() {
        let chord = 5.0 * 2.0_f64.sqrt();
        let short = from_center_start_chord(p(0.0, 0.0), p(5.0, 0.0), chord).unwrap();
        let (_, _, s, en) = parts(short);
        assert!(close(sweep(s, en), FRAC_PI_2));
        let long = from_center_start_chord(p(0.0, 0.0), p(5.0, 0.0), -chord).unwrap();
        let (_, _, s, en) = parts(long);
        assert!(close(sweep(s, en), 3.0 * FRAC_PI_2));
        assert!(from_center_start_chord(p(0.0, 0.0), p(5.0, 0.0), 11.0).is_none());
    }

    #[test]
    fn start_end_radius_is_short_for_positive_and_long_for_negative() {
        let short = from_start_end_radius(p(5.0, 0.0), p(0.0, 5.0), 5.0).unwrap();
        let (c, r, s, en) = parts(short);
        assert!(near(c, p(0.0, 0.0)) && close(r, 5.0));
        assert!(close(sweep(s, en), FRAC_PI_2));
        let long = from_start_end_radius(p(5.0, 0.0), p(0.0, 5.0), -5.0).unwrap();
        let (c, _, s, en) = parts(long);
        assert!(near(c, p(5.0, 5.0)));
        assert!(close(sweep(s, en), 3.0 * FRAC_PI_2));
        assert!(from_start_end_radius(p(0.0, 0.0), p(10.0, 0.0), 4.0).is_none());
    }

    #[test]
    fn start_end_angle_places_the_centre_by_the_included_angle() {
        let e = from_start_end_angle(p(5.0, 0.0), p(0.0, 5.0), FRAC_PI_2).unwrap();
        let (c, r, s, _) = parts(e);
        assert!(near(c, p(0.0, 0.0)) && close(r, 5.0));
        assert!(same_angle(s, 0.0));
        // A reflex angle puts the centre on the other side of the chord.
        let reflex = from_start_end_angle(p(5.0, 0.0), p(0.0, 5.0), 3.0 * FRAC_PI_2).unwrap();
        let (c, _, s, en) = parts(reflex);
        assert!(near(c, p(5.0, 5.0)));
        assert!(close(sweep(s, en), 3.0 * FRAC_PI_2));
        // Negative: clockwise from the start, so the same arc as the reverse.
        let back = from_start_end_angle(p(0.0, 5.0), p(5.0, 0.0), -FRAC_PI_2).unwrap();
        let (c, ..) = parts(back);
        assert!(near(c, p(0.0, 0.0)));
        assert!(from_start_end_angle(p(0.0, 0.0), p(1.0, 0.0), TAU).is_none());
    }

    #[test]
    fn start_end_direction_is_tangent_at_the_start() {
        // Heading north from (5,0) towards (0,5) curves left about the origin.
        let e = from_start_end_direction(p(5.0, 0.0), p(0.0, 5.0), FRAC_PI_2).unwrap();
        let (c, r, s, en) = parts(e);
        assert!(near(c, p(0.0, 0.0)) && close(r, 5.0));
        assert!(same_angle(s, 0.0) && same_angle(en, FRAC_PI_2));
        // Heading south curves right: the same circle, drawn clockwise, so the
        // counter-clockwise arc starts at the end.
        let e = from_start_end_direction(p(5.0, 0.0), p(0.0, -5.0), -FRAC_PI_2).unwrap();
        let (c, _, s, en) = parts(e);
        assert!(near(c, p(0.0, 0.0)));
        assert!(same_angle(s, -FRAC_PI_2) && same_angle(en, 0.0));
        // Heading straight at the end makes a line, not an arc.
        assert!(from_start_end_direction(p(0.0, 0.0), p(5.0, 0.0), 0.0).is_none());
    }
}
