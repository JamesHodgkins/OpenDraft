//! Circles tangent to two objects (CIRCLE, Ttr). A circle of radius `r`
//! touches a line when its centre is `r` from it, and touches a circle of
//! radius `R` when its centre is `R + r` or `|R - r|` from the circle's
//! centre. The centres that touch both objects are where those offset curves
//! cross. Objects count as the whole line or circle they lie on, as in
//! AutoCAD, so the circle may touch beyond the drawn end of a line or arc.

use crate::intersect::{circle_circle, dedupe, line_circle, line_line, Curve};
use crate::{Entity, Point};

const EPS: f64 = 1e-9;

#[derive(Clone, Copy)]
enum Carrier {
    Line(Point, Point),
    Circle(Point, f64),
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn carrier(curve: &Curve) -> Option<Carrier> {
    match *curve {
        Curve::Segment { a, b } if distance(a, b) > EPS => Some(Carrier::Line(a, b)),
        Curve::Circle { center, radius } | Curve::Arc { center, radius, .. } if radius > EPS => {
            Some(Carrier::Circle(center, radius))
        }
        _ => None,
    }
}

/// Where the centre of a circle of `radius` touching `carrier` can be.
fn offsets(carrier: Carrier, radius: f64) -> Vec<Carrier> {
    match carrier {
        Carrier::Line(a, b) => {
            let len = distance(a, b);
            let (nx, ny) = (-(b.y - a.y) / len * radius, (b.x - a.x) / len * radius);
            let shift = |s: f64| {
                Carrier::Line(
                    Point::new(a.x + s * nx, a.y + s * ny),
                    Point::new(b.x + s * nx, b.y + s * ny),
                )
            };
            vec![shift(1.0), shift(-1.0)]
        }
        Carrier::Circle(center, r) => [r + radius, (r - radius).abs()]
            .into_iter()
            .filter(|&r| r > EPS)
            .map(|r| Carrier::Circle(center, r))
            .collect(),
    }
}

fn cross(a: Carrier, b: Carrier) -> Vec<Point> {
    match (a, b) {
        (Carrier::Line(p, q), Carrier::Line(r, s)) => line_line(p, q, r, s),
        (Carrier::Line(p, q), Carrier::Circle(c, r))
        | (Carrier::Circle(c, r), Carrier::Line(p, q)) => line_circle(p, q, c, r),
        (Carrier::Circle(c1, r1), Carrier::Circle(c2, r2)) => circle_circle(c1, r1, c2, r2),
    }
}

/// Where a circle centred at `centre` touches `carrier`.
fn touch(carrier: Carrier, centre: Point) -> Option<Point> {
    match carrier {
        Carrier::Line(a, b) => {
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let t = ((centre.x - a.x) * dx + (centre.y - a.y) * dy) / (dx * dx + dy * dy);
            Some(Point::new(a.x + t * dx, a.y + t * dy))
        }
        Carrier::Circle(c, r) => {
            let d = distance(c, centre);
            (d > EPS).then(|| {
                Point::new(
                    c.x + (centre.x - c.x) / d * r,
                    c.y + (centre.y - c.y) / d * r,
                )
            })
        }
    }
}

/// Whether two lines lie along each other.
fn same_line(a: Carrier, b: Carrier) -> bool {
    let (Carrier::Line(p, q), Carrier::Line(r, s)) = (a, b) else {
        return false;
    };
    let (dx, dy) = (q.x - p.x, q.y - p.y);
    let len = dx.hypot(dy);
    let off = |t: Point| ((t.x - p.x) * dy - (t.y - p.y) * dx).abs() / len;
    off(r) < EPS * (1.0 + len) && off(s) < EPS * (1.0 + len)
}

/// The picked part of `entity` as an object of its own: a polyline's
/// segment as a line or arc.
fn as_entity(entity: &Entity, pick: Point) -> Option<Entity> {
    Some(match picked_curve(entity, pick)? {
        Curve::Segment { a, b } => Entity::Line { start: a, end: b },
        Curve::Circle { center, radius } => Entity::Circle { center, radius },
        Curve::Arc {
            center,
            radius,
            start,
            end,
        } => Entity::Arc {
            center,
            radius,
            start,
            end,
        },
        Curve::Ellipse(e) => Entity::Ellipse(e),
    })
}

/// The curve of `entity` nearest `pick`.
fn picked_curve(entity: &Entity, pick: Point) -> Option<Curve> {
    entity.curves().into_iter().min_by(|a, b| {
        distance(a.nearest_point(pick), pick).total_cmp(&distance(b.nearest_point(pick), pick))
    })
}

/// The centre of the circle of `radius` tangent to the objects `first` and
/// `second`, which were picked at `pick_first` and `pick_second`. Of the
/// possible circles it is the one whose touching points lie nearest the picks.
/// `None` when no such circle exists.
pub fn tangent_circle(
    first: &Entity,
    pick_first: Point,
    second: &Entity,
    pick_second: Point,
    radius: f64,
) -> Option<Point> {
    if radius <= EPS {
        return None;
    }
    if matches!(first, Entity::Ellipse(_)) || matches!(second, Entity::Ellipse(_)) {
        let (a, b) = (
            as_entity(first, pick_first)?,
            as_entity(second, pick_second)?,
        );
        return crate::fillet_ellipse::tangent_circle_ellipse(
            &a,
            pick_first,
            &b,
            pick_second,
            radius,
        );
    }
    let a = carrier(&picked_curve(first, pick_first)?)?;
    let b = carrier(&picked_curve(second, pick_second)?)?;
    let mut centres = Vec::new();
    let middle = Point::new(
        (pick_first.x + pick_second.x) / 2.0,
        (pick_first.y + pick_second.y) / 2.0,
    );
    for oa in offsets(a, radius) {
        for ob in offsets(b, radius) {
            let found = cross(oa, ob);
            // Parallel lines a diameter apart share their middle line: the
            // circle can sit anywhere along it, so take the spot by the picks.
            if found.is_empty() {
                if let (Carrier::Line(..), Carrier::Line(..)) = (oa, ob) {
                    if let Some(at) = same_line(oa, ob).then(|| touch(oa, middle)).flatten() {
                        centres.push(at);
                    }
                }
            } else {
                centres.extend(found);
            }
        }
    }
    dedupe(centres)
        .into_iter()
        .filter_map(|centre| {
            let (ta, tb) = (touch(a, centre)?, touch(b, centre)?);
            Some((distance(ta, pick_first) + distance(tb, pick_second), centre))
        })
        .min_by(|x, y| x.0.total_cmp(&y.0))
        .map(|(_, centre)| centre)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: (f64, f64), b: (f64, f64)) -> Entity {
        Entity::Line {
            start: p(a.0, a.1),
            end: p(b.0, b.1),
        }
    }

    fn near(a: Point, b: Point) -> bool {
        distance(a, b) < 1e-9
    }

    #[test]
    fn two_crossing_lines_give_the_circle_in_the_picked_quadrant() {
        let x = line((0.0, 0.0), (10.0, 0.0));
        let y = line((0.0, -10.0), (0.0, 10.0));
        let at = |pa, pb| tangent_circle(&x, pa, &y, pb, 1.0).unwrap();
        assert!(near(at(p(5.0, 0.0), p(0.0, 5.0)), p(1.0, 1.0)));
        assert!(near(at(p(5.0, 0.0), p(0.0, -5.0)), p(1.0, -1.0)));
    }

    #[test]
    fn a_line_and_a_circle_touch_the_nearer_side() {
        let ground = line((-10.0, 0.0), (10.0, 0.0));
        let ring = Entity::Circle {
            center: p(0.0, 3.0),
            radius: 2.0,
        };
        // Resting on the ground to the left of the ring: centre 1 above the
        // ground and 3 from the ring's centre.
        let centre = tangent_circle(&ground, p(-6.0, 0.0), &ring, p(-2.0, 3.0), 1.0).unwrap();
        assert!((centre.y - 1.0).abs() < 1e-9);
        assert!((distance(centre, p(0.0, 3.0)) - 3.0).abs() < 1e-9);
        assert!(centre.x < 0.0);
    }

    #[test]
    fn two_circles_share_a_tangent_circle_on_the_picked_side() {
        let a = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 2.0,
        };
        let b = Entity::Circle {
            center: p(10.0, 0.0),
            radius: 2.0,
        };
        let upper = tangent_circle(&a, p(0.0, 2.0), &b, p(10.0, 2.0), 4.0).unwrap();
        assert!((distance(upper, p(0.0, 0.0)) - 6.0).abs() < 1e-9);
        assert!((distance(upper, p(10.0, 0.0)) - 6.0).abs() < 1e-9);
        assert!(upper.y > 0.0);
        let lower = tangent_circle(&a, p(0.0, -2.0), &b, p(10.0, -2.0), 4.0).unwrap();
        assert!(lower.y < 0.0);
    }

    #[test]
    fn no_circle_when_the_objects_are_too_far_apart_for_the_radius() {
        let a = line((0.0, 0.0), (10.0, 0.0));
        let b = line((0.0, 10.0), (10.0, 10.0));
        assert!(tangent_circle(&a, p(5.0, 0.0), &b, p(5.0, 10.0), 1.0).is_none());
        let fits = tangent_circle(&a, p(5.0, 0.0), &b, p(5.0, 10.0), 5.0).unwrap();
        assert!((fits.y - 5.0).abs() < 1e-9);
    }

    #[test]
    fn a_zero_radius_is_refused() {
        let a = line((0.0, 0.0), (10.0, 0.0));
        let b = line((0.0, 0.0), (0.0, 10.0));
        assert!(tangent_circle(&a, p(5.0, 0.0), &b, p(0.0, 5.0), 0.0).is_none());
    }
}
