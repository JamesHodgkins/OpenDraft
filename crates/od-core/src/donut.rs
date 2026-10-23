//! Donuts, as AutoCAD's DONUT makes them: a closed wide polyline round a
//! circle, wide enough to fill the ring between the inside and the outside
//! diameter. An inside diameter of 0 fills the circle.
//!
//! AutoCAD uses two vertices and two half circles. Here a closed polyline of
//! two points is one segment (`bulge::segment_count`), so the ring is four
//! quarter circles.

use crate::{Entity, Point};

/// The donut about `center`, or `None` when a diameter is not a finite number,
/// the inside one is negative, or the outside one is not above zero. The
/// diameters may come in either order. A ring with no thickness is a hairline
/// circle.
pub fn donut(center: Point, inside: f64, outside: f64) -> Option<Entity> {
    if !(inside.is_finite() && outside.is_finite() && inside >= 0.0 && outside >= 0.0) {
        return None;
    }
    let (inside, outside) = (inside.min(outside), inside.max(outside));
    if outside <= 0.0 {
        return None;
    }
    // The centreline runs half way between the two circles; the width is the
    // thickness of the ring. A quarter circle's bulge is tan(45 degrees / 2).
    let radius = (inside + outside) / 4.0;
    let width = (outside - inside) / 2.0;
    Some(Entity::Polyline {
        points: vec![
            Point::new(center.x + radius, center.y),
            Point::new(center.x, center.y + radius),
            Point::new(center.x - radius, center.y),
            Point::new(center.x, center.y - radius),
        ],
        closed: true,
        bulges: vec![(std::f64::consts::FRAC_PI_8).tan(); 4],
        widths: vec![(width, width); 4],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bulge;

    fn parts(e: Entity) -> (Vec<Point>, Vec<f64>, Vec<(f64, f64)>) {
        match e {
            Entity::Polyline {
                points,
                closed: true,
                bulges,
                widths,
            } => (points, bulges, widths),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_ring_is_four_quarter_circles_on_the_middle_radius() {
        let (points, bulges, widths) = parts(donut(Point::new(10.0, 5.0), 2.0, 6.0).unwrap());
        assert_eq!(points[0], Point::new(12.0, 5.0));
        assert_eq!(points[2], Point::new(8.0, 5.0));
        assert_eq!(bulges.len(), 4);
        assert_eq!(widths, [(2.0, 2.0); 4]);
        let round = bulge::path_length(&points, &bulges, true);
        assert!(
            (round - std::f64::consts::TAU * 2.0).abs() < 1e-9,
            "{round}"
        );
    }

    #[test]
    fn no_inside_diameter_fills_the_circle() {
        let (points, _, widths) = parts(donut(Point::new(0.0, 0.0), 0.0, 4.0).unwrap());
        assert_eq!(points[0], Point::new(1.0, 0.0));
        assert_eq!(widths[0], (2.0, 2.0));
    }

    #[test]
    fn equal_diameters_make_a_hairline_and_either_order_works() {
        let (_, _, widths) = parts(donut(Point::new(0.0, 0.0), 3.0, 3.0).unwrap());
        assert_eq!(widths[0], (0.0, 0.0));
        assert_eq!(
            donut(Point::new(0.0, 0.0), 6.0, 2.0),
            donut(Point::new(0.0, 0.0), 2.0, 6.0)
        );
    }

    #[test]
    fn nothing_for_a_bad_diameter() {
        let c = Point::new(0.0, 0.0);
        assert_eq!(donut(c, 0.0, 0.0), None);
        assert_eq!(donut(c, -1.0, 2.0), None);
        assert_eq!(donut(c, 1.0, f64::NAN), None);
        assert_eq!(donut(c, f64::INFINITY, 2.0), None);
    }
}
