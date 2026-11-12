//! Fillets and tangent circles where one of the objects is an ellipse.
//!
//! A circle of radius `r` touches the ellipse at the point at parameter `t`
//! when its centre is `p(t) ± r·n(t)`, `n` being the outward normal. It
//! touches the other object (a line, a circle or arc, or another ellipse)
//! when that centre is `r` from it on one side: from a line's own line, `R ±
//! r` from a circle's centre, `r` from another ellipse inside or out. Along
//! the ellipse that second condition is a continuous function of `t`, so its
//! zeros are found with fixed work: sample round the turn, bisect where the
//! sign changes. Each zero is a possible circle, with its two touching points.

use std::f64::consts::TAU;

use crate::corner::{CornerError, Joined};
use crate::fillet::{joint_arc, keep_part};
use crate::{Ellipse, Entity, Point};

const EPS: f64 = 1e-9;
/// Samples of the parameter round the turn.
const SAMPLES: usize = 360;
const STEPS: usize = 64;

/// A touching point on the ellipse, one on the other object, and the centre.
type Option3 = (Point, Point, Point);

fn dist(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// The whole ellipse an ellipse or elliptical arc lies on.
fn whole(e: &Ellipse) -> Ellipse {
    Ellipse {
        start: 0.0,
        end: TAU,
        ..*e
    }
}

/// The ellipse as the unit circle: whether `p` is inside it.
fn inside(e: &Ellipse, p: Point) -> bool {
    e.radius_ratio > 0.0 && {
        let a = e.major_radius();
        let (ux, uy) = (e.major_axis.x / a, e.major_axis.y / a);
        let (dx, dy) = (p.x - e.center.x, p.y - e.center.y);
        let (x, y) = (
            (dx * ux + dy * uy) / a,
            (-dx * uy + dy * ux) / (a * e.radius_ratio),
        );
        x * x + y * y < 1.0
    }
}

/// What the circle's centre must keep a radius from, and how.
enum Target {
    /// A line: its signed distance (left positive) is `side`.
    Line {
        from: Point,
        along: Point,
        side: f64,
    },
    /// A circle: the centre is `distance` from its centre.
    Circle { center: Point, distance: f64 },
    /// An ellipse: its signed distance (outside positive) is `side`.
    Ellipse { e: Ellipse, side: f64 },
}

impl Target {
    /// Zero where a circle centred at `x` touches as wanted.
    fn miss(&self, x: Point) -> f64 {
        match *self {
            Target::Line { from, along, side } => {
                along.x * (x.y - from.y) - along.y * (x.x - from.x) - side
            }
            Target::Circle { center, distance } => dist(x, center) - distance,
            Target::Ellipse { e, side } => {
                let d = e.distance_to(x);
                (if inside(&e, x) { -d } else { d }) - side
            }
        }
    }
}

/// The ways a circle of `r` can touch `entity`, or `None` for a kind it
/// cannot (only lines, circles, arcs and ellipses).
fn targets(entity: &Entity, r: f64) -> Option<Vec<Target>> {
    Some(match entity {
        Entity::Line { start, end } => {
            let len = dist(*start, *end);
            if len <= EPS {
                return None;
            }
            let along = Point::new((end.x - start.x) / len, (end.y - start.y) / len);
            [r, -r]
                .map(|side| Target::Line {
                    from: *start,
                    along,
                    side,
                })
                .into()
        }
        Entity::Circle { center, radius } | Entity::Arc { center, radius, .. } => {
            let big = radius.abs();
            let mut found = vec![Target::Circle {
                center: *center,
                distance: big + r,
            }];
            if (big - r).abs() > EPS {
                found.push(Target::Circle {
                    center: *center,
                    distance: (big - r).abs(),
                });
            }
            found
        }
        Entity::Ellipse(e) => [r, -r]
            .map(|side| Target::Ellipse { e: whole(e), side })
            .into(),
        _ => return None,
    })
}

/// Where a circle centred at `x` touches the whole carrier of `entity`.
fn foot(entity: &Entity, x: Point) -> Option<Point> {
    match entity {
        Entity::Line { start, end } => {
            let len = dist(*start, *end);
            let (ux, uy) = ((end.x - start.x) / len, (end.y - start.y) / len);
            let t = (x.x - start.x) * ux + (x.y - start.y) * uy;
            Some(Point::new(start.x + ux * t, start.y + uy * t))
        }
        Entity::Circle { center, radius } | Entity::Arc { center, radius, .. } => {
            let len = dist(*center, x);
            (len > EPS).then(|| {
                let r = radius.abs();
                Point::new(
                    center.x + (x.x - center.x) / len * r,
                    center.y + (x.y - center.y) / len * r,
                )
            })
        }
        Entity::Ellipse(e) => Some(e.nearest_on_whole(x).0),
        _ => None,
    }
}

/// Where `g` changes sign in `[lo, hi]`, given it does.
fn bisect(g: &impl Fn(f64) -> f64, mut lo: f64, mut hi: f64) -> f64 {
    let low_negative = g(lo) < 0.0;
    for _ in 0..STEPS {
        let mid = lo + (hi - lo) / 2.0;
        if mid <= lo || mid >= hi {
            break;
        }
        if (g(mid) < 0.0) == low_negative {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo + (hi - lo) / 2.0
}

/// Every circle of radius `r` touching the ellipse `e` and `other`: its
/// touching points on each and its centre. `None` when `other` is not a
/// kind a circle can be fitted to.
fn touching_circles(e: &Ellipse, other: &Entity, r: f64) -> Option<Vec<Option3>> {
    let targets = targets(other, r)?;
    let e = whole(e);
    let centre = |t: f64, side: f64| {
        let p = e.point_at(t);
        let d = e.tangent_at(t);
        let len = d.x.hypot(d.y);
        // The outward normal of a counter-clockwise curve is to its right.
        Point::new(p.x + side * r * d.y / len, p.y - side * r * d.x / len)
    };
    let mut found = Vec::new();
    for side in [1.0, -1.0] {
        for target in &targets {
            let miss = |t: f64| target.miss(centre(t, side));
            let mut before = miss(0.0);
            for i in 1..=SAMPLES {
                let lo = TAU * (i - 1) as f64 / SAMPLES as f64;
                let hi = TAU * i as f64 / SAMPLES as f64;
                let now = miss(hi);
                let t = if now == 0.0 {
                    Some(hi)
                } else if (before < 0.0) != (now < 0.0) && before != 0.0 {
                    Some(bisect(&miss, lo, hi))
                } else {
                    None
                };
                before = now;
                let Some(t) = t else { continue };
                let c = centre(t, side);
                // A real fit: the centre is a radius from the other object
                // (not a sign change jumped past) and from the whole ellipse
                // (a circle inside a tight bend would cut through it).
                let near = 1e-6 * (1.0 + r);
                let on_other = foot(other, c).filter(|q| (dist(*q, c) - r).abs() <= near);
                if let Some(q) = on_other {
                    if (e.distance_to(c) - r).abs() <= near {
                        found.push((e.point_at(t), q, c));
                    }
                }
            }
        }
    }
    Some(found)
}

/// Where the whole carriers of the ellipse and `other` cross, as touching
/// points for a fillet of no radius.
fn corners(e: &Ellipse, other: &Entity) -> Option<Vec<Option3>> {
    let e = whole(e);
    let points = match other {
        Entity::Line { start, end } => e.line_crossings(*start, *end),
        Entity::Circle { center, radius } | Entity::Arc { center, radius, .. } => {
            e.circle_crossings(*center, radius.abs())
        }
        Entity::Ellipse(o) => e.ellipse_crossings(&whole(o)),
        _ => return None,
    };
    Some(points.into_iter().map(|p| (p, p, p)).collect())
}

/// The part of an ellipse that stays when it is cut (or extended round its
/// ellipse) at `at`, on the side of `picked`. A full ellipse, like a circle,
/// is not trimmed.
fn keep_ellipse(e: &Ellipse, at: Point, picked: Point) -> Result<Entity, CornerError> {
    if e.is_full() {
        return Ok(Entity::Ellipse(*e));
    }
    let sweep = e.sweep();
    let from_start = (e.parameter_of(at) - e.start).rem_euclid(TAU);
    // Before the start counts as negative, so an extension either way is
    // told from a point on the arc.
    let t = if from_start <= sweep || from_start - sweep < TAU - from_start {
        from_start
    } else {
        from_start - TAU
    };
    let pick = (e.nearest_on_whole(picked).1 - e.start).rem_euclid(TAU);
    let (start, end) = if pick >= t {
        if t >= sweep - EPS {
            return Err(CornerError::TooLong);
        }
        (e.start + t, e.end)
    } else {
        if t <= EPS {
            return Err(CornerError::TooLong);
        }
        (e.start, e.start + t)
    };
    Ellipse::new(e.center, e.major_axis, e.radius_ratio, start, end)
        .map(Entity::Ellipse)
        .map_err(|_| CornerError::TooLong)
}

fn keep(entity: &Entity, at: Point, picked: Point) -> Result<Entity, CornerError> {
    match entity {
        Entity::Ellipse(e) => keep_ellipse(e, at, picked),
        other => keep_part(other, at, picked),
    }
}

/// The possible touching circles (or corners, for no radius) between `a`
/// and `b`, one of which is an ellipse, as (on `a`, on `b`, centre).
fn options(a: &Entity, b: &Entity, radius: f64) -> Option<Vec<Option3>> {
    let round = radius.is_finite() && radius > EPS;
    let find = |e: &Ellipse, other: &Entity| {
        if round {
            touching_circles(e, other, radius)
        } else {
            corners(e, other)
        }
    };
    match (a, b) {
        (Entity::Ellipse(e), other) => find(e, other),
        (other, Entity::Ellipse(e)) => Some(
            find(e, other)?
                .into_iter()
                .map(|(on_e, on_o, c)| (on_o, on_e, c))
                .collect(),
        ),
        _ => None,
    }
}

/// FILLET where `a` or `b` is an ellipse: as [`crate::fillet`], each object
/// keeping the side it was picked on, an elliptical arc cut back or
/// extended round its ellipse.
pub(crate) fn fillet_ellipse(
    a: &Entity,
    b: &Entity,
    radius: f64,
    picked_a: Point,
    picked_b: Point,
) -> Result<Joined, CornerError> {
    let found = options(a, b, radius).ok_or(CornerError::Unsupported)?;
    let score = |o: &Option3| dist(o.0, picked_a) + dist(o.1, picked_b);
    let (ta, tb, center) = found
        .into_iter()
        .filter(|o| radius <= EPS || dist(o.0, o.1) > EPS)
        .min_by(|x, y| score(x).total_cmp(&score(y)))
        .ok_or(CornerError::NoFit)?;
    let round = radius.is_finite() && radius > EPS;
    Ok(Joined {
        a: keep(a, ta, picked_a)?,
        b: keep(b, tb, picked_b)?,
        joint: round.then(|| joint_arc(center, radius, ta, tb)),
    })
}

/// The centre of the circle of `radius` touching `a` and `b` (one an
/// ellipse) whose touching points lie nearest the picks, for CIRCLE TTR.
pub(crate) fn tangent_circle_ellipse(
    a: &Entity,
    pick_a: Point,
    b: &Entity,
    pick_b: Point,
    radius: f64,
) -> Option<Point> {
    let score = |o: &Option3| dist(o.0, pick_a) + dist(o.1, pick_b);
    options(a, b, radius)?
        .into_iter()
        .min_by(|x, y| score(x).total_cmp(&score(y)))
        .map(|o| o.2)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;
    use crate::fillet;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn ellipse(start: f64, end: f64) -> Entity {
        Entity::Ellipse(Ellipse::new(p(0.0, 0.0), p(4.0, 0.0), 0.5, start, end).unwrap())
    }

    /// The fillet arc's ends lie on both objects and its centre is a radius
    /// from each.
    fn assert_touches(a: &Entity, b: &Entity, done: &Joined, radius: f64) {
        let Some(Entity::Arc {
            center,
            radius: r,
            start,
            end,
        }) = done.joint
        else {
            panic!("a fillet arc, got {:?}", done.joint)
        };
        assert!((r - radius).abs() < 1e-12);
        let ends = [
            crate::arc::point_at(center, r, start),
            crate::arc::point_at(center, r, end),
        ];
        for curve in [a, b] {
            let whole = match curve {
                Entity::Ellipse(e) => Entity::Ellipse(whole(e)),
                Entity::Line { start, end } => {
                    let (dx, dy) = (end.x - start.x, end.y - start.y);
                    line(
                        p(start.x - 100.0 * dx, start.y - 100.0 * dy),
                        p(end.x + 100.0 * dx, end.y + 100.0 * dy),
                    )
                }
                other => other.clone(),
            };
            assert!(
                ends.iter().any(|q| whole.distance_to(*q) < 1e-6),
                "touches {curve:?}"
            );
            assert!(
                (whole.distance_to(center) - r).abs() < 1e-6,
                "a radius from {curve:?}"
            );
        }
    }

    #[test]
    fn a_line_and_an_ellipse_are_rounded_and_the_full_ellipse_kept() {
        let (a, b) = (line(p(-10.0, 3.0), p(10.0, 3.0)), ellipse(0.0, TAU));
        let done = fillet(&a, &b, 1.0, p(6.0, 3.0), p(3.0, 1.4)).unwrap();
        assert_touches(&a, &b, &done, 1.0);
        let Some(Entity::Arc { center, .. }) = done.joint else {
            unreachable!()
        };
        assert!(
            (center.y - 2.0).abs() < 1e-9 && center.x > 0.0,
            "below the line, to the right"
        );
        assert_eq!(done.b, b, "a full ellipse is not trimmed, like a circle");
        let Entity::Line { start, end } = done.a else {
            panic!("a line")
        };
        assert!(
            (start.x - center.x).abs() < 1e-9 && end == p(10.0, 3.0),
            "kept the picked side"
        );
    }

    #[test]
    fn an_elliptical_arc_is_cut_back_to_the_fillet() {
        // The upper half, and a wall at x = 3 crossing it.
        let (a, b) = (ellipse(0.0, PI), line(p(3.0, -5.0), p(3.0, 5.0)));
        let done = fillet(&a, &b, 0.5, p(-2.0, 1.7), p(3.0, 4.0)).unwrap();
        assert_touches(&a, &b, &done, 0.5);
        let Entity::Ellipse(kept) = done.a else {
            panic!("an elliptical arc")
        };
        assert!(
            kept.sweep() < PI && (kept.end - PI).abs() < 1e-9,
            "the start was cut back"
        );
        assert!(kept.start_point().x < 3.0);
        // The line keeps its upper part from the fillet up.
        let Entity::Line { start, end } = done.b else {
            panic!("a line")
        };
        assert!(start.y > 0.0 && end == p(3.0, 5.0));
        // With no radius they meet where they cross.
        let sharp = fillet(&a, &b, 0.0, p(-2.0, 1.7), p(3.0, 4.0)).unwrap();
        let Entity::Ellipse(kept) = sharp.a else {
            panic!()
        };
        assert!((kept.start_point().x - 3.0).abs() < 1e-9 && sharp.joint.is_none());
    }

    #[test]
    fn circles_and_other_ellipses_are_touched_too() {
        let e = ellipse(0.0, TAU);
        let circle = Entity::Circle {
            center: p(7.0, 0.0),
            radius: 2.0,
        };
        let done = fillet(&e, &circle, 0.75, p(3.5, 1.0), p(6.0, 1.7)).unwrap();
        assert_touches(&e, &circle, &done, 0.75);
        let other = Entity::Ellipse(Ellipse::new(p(0.0, 5.0), p(0.0, 3.0), 0.4, 0.0, TAU).unwrap());
        let done = fillet(&e, &other, 0.5, p(2.0, 1.7), p(1.0, 4.0)).unwrap();
        assert_touches(&e, &other, &done, 0.5);
        // Either order gives the same circle.
        let back = fillet(&other, &e, 0.5, p(1.0, 4.0), p(2.0, 1.7)).unwrap();
        let (x, y) = (back.joint.unwrap().bounds(), done.joint.unwrap().bounds());
        assert!((x.min.x - y.min.x).abs() < 1e-9 && (x.max.y - y.max.y).abs() < 1e-9);
    }

    #[test]
    fn many_random_fillets_touch_both_objects() {
        let mut seed = 99u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut made = 0;
        for _ in 0..200 {
            let mut r = |lo: f64, hi: f64| lo + (hi - lo) * next();
            let e = Entity::Ellipse(
                Ellipse::full(
                    p(r(-5.0, 5.0), r(-5.0, 5.0)),
                    {
                        let (a, t) = (r(1.0, 6.0), r(0.0, TAU));
                        p(a * t.cos(), a * t.sin())
                    },
                    r(0.2, 1.0),
                )
                .unwrap(),
            );
            let other = match (r(0.0, 3.0)) as u32 {
                0 => line(p(r(-9.0, 9.0), r(-9.0, 9.0)), p(r(-9.0, 9.0), r(-9.0, 9.0))),
                1 => Entity::Circle {
                    center: p(r(-9.0, 9.0), r(-9.0, 9.0)),
                    radius: r(0.5, 4.0),
                },
                _ => Entity::Ellipse(
                    Ellipse::full(
                        p(r(-9.0, 9.0), r(-9.0, 9.0)),
                        p(r(1.0, 4.0), r(-2.0, 2.0)),
                        r(0.2, 1.0),
                    )
                    .unwrap(),
                ),
            };
            let radius = r(0.2, 2.0);
            let (pa, pb) = (p(r(-9.0, 9.0), r(-9.0, 9.0)), p(r(-9.0, 9.0), r(-9.0, 9.0)));
            if let Ok(done) = fillet(&e, &other, radius, pa, pb) {
                assert_touches(&e, &other, &done, radius);
                made += 1;
            }
        }
        assert!(made > 100, "most random pairs can be filleted: {made}");
    }

    #[test]
    fn a_tangent_circle_can_touch_an_ellipse() {
        let e = ellipse(0.0, TAU);
        let wall = line(p(6.0, -5.0), p(6.0, 5.0));
        // Radius 1.5 between the ellipse's right end (x = 4) and the wall.
        let c = crate::tangent_circle(&e, p(4.0, 0.5), &wall, p(6.0, 0.5), 1.5).unwrap();
        assert!((c.x - 4.5).abs() < 1e-6 && c.y > 0.0, "{c:?}");
        assert!((e.distance_to(c) - 1.5).abs() < 1e-6);
    }
}
