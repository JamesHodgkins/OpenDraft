//! Ortho: keeping a point on the horizontal or vertical line through a base
//! point. Pure geometry; the canvas decides when to apply it.

use crate::Point;

/// The direction a point is held to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Same y as the base; x is free.
    Horizontal,
    /// Same x as the base; y is free.
    Vertical,
}

/// The axis from `base` that `p` lies nearer to (horizontal when the offsets
/// tie).
pub fn axis_of(base: Point, p: Point) -> Axis {
    if (p.x - base.x).abs() >= (p.y - base.y).abs() {
        Axis::Horizontal
    } else {
        Axis::Vertical
    }
}

/// `p` moved onto `axis` through `base`.
pub fn on_axis(base: Point, p: Point, axis: Axis) -> Point {
    match axis {
        Axis::Horizontal => Point::new(p.x, base.y),
        Axis::Vertical => Point::new(base.x, p.y),
    }
}

/// `p` moved onto whichever axis through `base` it is nearer to.
pub fn constrain(base: Point, p: Point) -> Point {
    on_axis(base, p, axis_of(base, p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn a_mostly_sideways_point_goes_horizontal() {
        assert_eq!(constrain(p(1.0, 1.0), p(10.0, 3.0)), p(10.0, 1.0));
    }

    #[test]
    fn a_mostly_upward_point_goes_vertical() {
        assert_eq!(constrain(p(1.0, 1.0), p(2.0, -9.0)), p(1.0, -9.0));
    }

    #[test]
    fn a_tie_goes_horizontal() {
        assert_eq!(axis_of(p(0.0, 0.0), p(5.0, -5.0)), Axis::Horizontal);
    }

    #[test]
    fn a_point_on_the_base_stays_there() {
        assert_eq!(constrain(p(3.0, 4.0), p(3.0, 4.0)), p(3.0, 4.0));
    }
}
