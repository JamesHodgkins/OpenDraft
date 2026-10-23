//! Fillet and chamfer where one or both objects are construction lines or
//! rays. Each stands in as a very long line, the corner is worked out as for
//! lines, and the result is turned back: a construction line cut at one side
//! becomes a ray, a ray stays a ray, and anything cut at both ends is an
//! ordinary line.

use crate::{Chamfer, CornerError, Entity, Joined, Point};

/// Round the corner of `a` and `b` with an arc of `radius`; see
/// [`crate::fillet::fillet`]. Either may be a construction line or ray.
pub fn fillet(
    a: &Entity,
    b: &Entity,
    radius: f64,
    picked_a: Point,
    picked_b: Point,
) -> Result<Joined, CornerError> {
    through_lines(a, b, |x, y| {
        crate::fillet::fillet(x, y, radius, picked_a, picked_b)
    })
}

/// Cut the corner of `a` and `b` with a straight line; see
/// [`crate::corner::chamfer_lines`]. Either may be a construction line or ray.
pub fn chamfer_lines(
    a: &Entity,
    b: &Entity,
    chamfer: Chamfer,
    picked_a: Point,
    picked_b: Point,
) -> Result<Joined, CornerError> {
    through_lines(a, b, |x, y| {
        crate::corner::chamfer_lines(x, y, chamfer, picked_a, picked_b)
    })
}

fn through_lines(
    a: &Entity,
    b: &Entity,
    corner: impl FnOnce(&Entity, &Entity) -> Result<Joined, CornerError>,
) -> Result<Joined, CornerError> {
    if a.infinite_line().is_none() && b.infinite_line().is_none() {
        return corner(a, b);
    }
    let joined = corner(&a.long_line(), &b.long_line())?;
    Ok(Joined {
        a: a.resumed(joined.a),
        b: b.resumed(joined.b),
        joint: joined.joint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn xline(o: Point, d: Point) -> Entity {
        Entity::Xline {
            origin: o,
            direction: d,
        }
    }

    fn base(e: &Entity) -> (Point, Point) {
        let (o, d, _) = e.infinite_line().unwrap();
        (o, d)
    }

    #[test]
    fn two_construction_lines_with_no_radius_become_rays_from_the_corner() {
        let a = xline(p(0.0, 0.0), p(1.0, 0.0));
        let b = xline(p(0.0, 0.0), p(0.0, 1.0));
        let joined = fillet(&a, &b, 0.0, p(5.0, 0.0), p(0.0, 5.0)).unwrap();
        assert!(matches!(joined.a, Entity::Ray { .. }));
        let (o, d) = base(&joined.a);
        assert!(o.x.abs() < 1e-6 && o.y.abs() < 1e-6, "{o:?}");
        assert!((d.x - 1.0).abs() < 1e-9, "keeps the side that was picked");
        let (_, d) = base(&joined.b);
        assert!((d.y - 1.0).abs() < 1e-9);
        assert!(joined.joint.is_none());
    }

    #[test]
    fn a_radius_cuts_each_back_to_where_the_arc_touches() {
        let a = xline(p(0.0, 0.0), p(1.0, 0.0));
        let b = xline(p(0.0, 0.0), p(0.0, 1.0));
        let joined = fillet(&a, &b, 2.0, p(9.0, 0.0), p(0.0, 9.0)).unwrap();
        let (o, d) = base(&joined.a);
        assert!((o.x - 2.0).abs() < 1e-6 && o.y.abs() < 1e-6, "{o:?}");
        assert!(d.x > 0.99);
        assert!(matches!(joined.joint, Some(Entity::Arc { .. })));
    }

    #[test]
    fn a_ray_cut_at_both_ends_is_a_line_and_a_line_is_cut_as_usual() {
        let ray = Entity::Ray {
            origin: p(1.0, 0.0),
            direction: p(1.0, 0.0),
        };
        let wall = Entity::Line {
            start: p(6.0, -5.0),
            end: p(6.0, 5.0),
        };
        let joined = fillet(&ray, &wall, 0.0, p(3.0, 0.0), p(6.0, 3.0)).unwrap();
        // The ray is cut where it meets the wall, keeping the side picked.
        let Entity::Line { start, end } = joined.a else {
            panic!("{:?}", joined.a);
        };
        assert!((start.x - 1.0).abs() < 1e-6 && (end.x - 6.0).abs() < 1e-6);
        let Entity::Line { start, end } = joined.b else {
            panic!("a line");
        };
        assert!(
            (start.y - 0.0).abs() < 1e-6 && (end.y - 5.0).abs() < 1e-6,
            "{start:?} {end:?}"
        );
    }

    #[test]
    fn a_chamfer_cuts_the_corner_of_a_construction_line_and_a_ray() {
        let a = xline(p(0.0, 0.0), p(1.0, 0.0));
        let b = Entity::Ray {
            origin: p(0.0, -4.0),
            direction: p(0.0, 1.0),
        };
        let joined = chamfer_lines(
            &a,
            &b,
            Chamfer::Distances(2.0, 3.0),
            p(8.0, 0.0),
            p(0.0, 8.0),
        )
        .unwrap();
        let (o, _) = base(&joined.a);
        assert!((o.x - 2.0).abs() < 1e-6);
        let (o, d) = base(&joined.b);
        assert!((o.y - 3.0).abs() < 1e-6 && d.y > 0.99, "{o:?}");
        assert!(matches!(joined.joint, Some(Entity::Line { .. })));
    }

    #[test]
    fn parallel_lines_still_have_no_corner() {
        let a = xline(p(0.0, 0.0), p(1.0, 0.0));
        let b = xline(p(0.0, 3.0), p(1.0, 0.0));
        assert!(fillet(&a, &b, 0.0, p(1.0, 0.0), p(1.0, 3.0)).is_err());
    }
}
