//! Placing text from the two corners of a box the user drags out.

use super::MTextAttachment;
use crate::Point;

/// Where the text goes for a box dragged from `first` to `second` with its
/// sides along `rotation` (radians): the insertion point, which sits where
/// `attachment` says on that box, and the box's width. A box with no width
/// gives 0, which turns wrapping off.
pub fn place_in_corners(
    first: Point,
    second: Point,
    rotation: f64,
    attachment: MTextAttachment,
) -> (Point, f64) {
    let (sin, cos) = rotation.sin_cos();
    let (dx, dy) = (second.x - first.x, second.y - first.y);
    // The far corner in the box's own axes, with the first corner at the origin.
    let (x, y) = (dx * cos + dy * sin, -dx * sin + dy * cos);
    let (left, right) = (x.min(0.0), x.max(0.0));
    let (bottom, top) = (y.min(0.0), y.max(0.0));
    let (across, down) = attachment.fractions();
    let local_x = left + across * (right - left);
    let local_y = top - down * (top - bottom);
    let insertion = Point::new(
        first.x + local_x * cos - local_y * sin,
        first.y + local_x * sin + local_y * cos,
    );
    let width = right - left;
    (insertion, if width > 1e-9 { width } else { 0.0 })
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
    }

    #[test]
    fn the_insertion_point_sits_on_the_box_where_the_attachment_says() {
        let (a, b) = (Point::new(10.0, 20.0), Point::new(30.0, 5.0));
        // The box covers x 10..30, y 5..20.
        let at = |attachment| place_in_corners(a, b, 0.0, attachment);
        assert!(near(at(MTextAttachment::TopLeft).0, Point::new(10.0, 20.0)));
        assert!(near(
            at(MTextAttachment::TopRight).0,
            Point::new(30.0, 20.0)
        ));
        assert!(near(
            at(MTextAttachment::MiddleCenter).0,
            Point::new(20.0, 12.5)
        ));
        assert!(near(
            at(MTextAttachment::BottomLeft).0,
            Point::new(10.0, 5.0)
        ));
        assert_eq!(at(MTextAttachment::TopLeft).1, 20.0);
    }

    #[test]
    fn dragging_leftwards_or_upwards_gives_the_same_box() {
        let (a, b) = (Point::new(30.0, 5.0), Point::new(10.0, 20.0));
        let (insertion, width) = place_in_corners(a, b, 0.0, MTextAttachment::TopLeft);
        assert!(near(insertion, Point::new(10.0, 20.0)));
        assert_eq!(width, 20.0);
    }

    #[test]
    fn a_turned_box_is_measured_along_its_own_axes() {
        let first = Point::new(0.0, 0.0);
        // Turned a quarter, "right" is up the page: the far corner is 5 along and 8 across.
        let second = Point::new(-8.0, 5.0);
        let (insertion, width) =
            place_in_corners(first, second, FRAC_PI_2, MTextAttachment::TopLeft);
        assert!((width - 5.0).abs() < 1e-9);
        assert!(near(insertion, Point::new(-8.0, 0.0)), "{insertion:?}");
        let (insertion, _) =
            place_in_corners(first, second, FRAC_PI_2, MTextAttachment::BottomRight);
        assert!(near(insertion, Point::new(0.0, 5.0)), "{insertion:?}");
    }

    #[test]
    fn a_box_with_no_width_does_not_wrap() {
        let (_, width) = place_in_corners(
            Point::new(1.0, 1.0),
            Point::new(1.0, 9.0),
            0.0,
            MTextAttachment::TopLeft,
        );
        assert_eq!(width, 0.0);
    }
}
