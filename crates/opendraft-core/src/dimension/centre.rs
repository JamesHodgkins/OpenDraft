//! Centre marks and centrelines of circles and arcs (`DIMCEN`): a positive
//! size is a cross whose arms reach that far from the centre, a negative
//! one a pair of centrelines reaching that far beyond the circle.

use crate::Point;

/// The segments marking a circle of `radius` about `center` for the style
/// value `dimcen`; none for zero or a size that is not a number.
pub fn centre_segments(center: Point, radius: f64, dimcen: f64) -> Vec<(Point, Point)> {
    if !(dimcen.is_finite() && radius.is_finite()) || dimcen == 0.0 {
        return Vec::new();
    }
    let reach = if dimcen > 0.0 {
        dimcen
    } else {
        radius.abs() - dimcen
    };
    let at = |dx: f64, dy: f64| Point::new(center.x + dx, center.y + dy);
    vec![
        (at(-reach, 0.0), at(reach, 0.0)),
        (at(0.0, -reach), at(0.0, reach)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_positive_size_is_a_cross_and_a_negative_one_reaches_past_the_circle() {
        let c = Point::new(1.0, 2.0);
        let mark = centre_segments(c, 10.0, 2.5);
        assert_eq!(mark[0], (Point::new(-1.5, 2.0), Point::new(3.5, 2.0)));
        let lines = centre_segments(c, 10.0, -2.5);
        assert_eq!(lines[1], (Point::new(1.0, -10.5), Point::new(1.0, 14.5)));
        assert!(centre_segments(c, 10.0, 0.0).is_empty());
        assert!(centre_segments(c, 10.0, f64::NAN).is_empty());
    }
}
