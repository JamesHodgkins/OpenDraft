//! The outline RECTANGLE draws: a rectangle at any angle, with square, rounded
//! or bevelled corners and a line width. One that is axis-aligned, square and
//! hairline is a plain [`Entity::Rectangle`]; anything else is a closed
//! polyline, as in AutoCAD.

use crate::{Entity, Point};

/// How the corners of a rectangle look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Corners {
    Square,
    /// Rounded with this radius.
    Fillet(f64),
    /// Cut off by these distances: the first along the sides that run in the
    /// rectangle's own x direction, the second along those that run in y.
    Chamfer(f64, f64),
}

/// The rectangle with a corner at `origin`, turned `angle` radians
/// counter-clockwise about it, `dx` long along its own x axis and `dy` along
/// its y axis (either may be negative, which places it on the other side).
/// Corners that do not fit (a radius or bevel taking more than the sides
/// have) are left square, as AutoCAD does. `width` is the line width.
pub fn outline(
    origin: Point,
    angle: f64,
    dx: f64,
    dy: f64,
    corners: Corners,
    width: f64,
) -> Entity {
    let (u, v) = ((angle.cos(), angle.sin()), (-angle.sin(), angle.cos()));
    let at =
        |a: f64, b: f64| Point::new(origin.x + u.0 * a + v.0 * b, origin.y + u.1 * a + v.1 * b);
    let square = [at(0.0, 0.0), at(dx, 0.0), at(dx, dy), at(0.0, dy)];
    let corners = fitting(corners, dx.abs(), dy.abs());
    if angle == 0.0 && corners == Corners::Square && width == 0.0 {
        return Entity::Rectangle {
            min: Point::new(origin.x.min(origin.x + dx), origin.y.min(origin.y + dy)),
            max: Point::new(origin.x.max(origin.x + dx), origin.y.max(origin.y + dy)),
        };
    }
    let (points, bulges) = match corners {
        Corners::Square => (square.to_vec(), Vec::new()),
        round_or_cut => cut_corners(&square, round_or_cut, dx * dy > 0.0),
    };
    let widths = if width > 0.0 {
        vec![(width, width); points.len()]
    } else {
        Vec::new()
    };
    Entity::Polyline {
        points,
        closed: true,
        bulges,
        widths,
    }
}

/// `corners`, or square corners when they would take more than the sides (or
/// nothing at all).
fn fitting(corners: Corners, long: f64, high: f64) -> Corners {
    let fits = match corners {
        Corners::Square => false,
        Corners::Fillet(r) => r > 0.0 && 2.0 * r < long.min(high),
        Corners::Chamfer(a, b) => a > 0.0 && b > 0.0 && 2.0 * a < long && 2.0 * b < high,
    };
    if fits {
        corners
    } else {
        Corners::Square
    }
}

/// The points and bulges of `square` with each corner replaced by an arc or a
/// bevel. `left` is whether the corners turn counter-clockwise.
fn cut_corners(square: &[Point; 4], corners: Corners, left: bool) -> (Vec<Point>, Vec<f64>) {
    // How far back from a corner the cut starts, along side `i` (which runs
    // from corner `i` to corner `i + 1`).
    let reach = |side: usize| match corners {
        Corners::Fillet(r) => r,
        Corners::Chamfer(a, b) => {
            if side.is_multiple_of(2) {
                a
            } else {
                b
            }
        }
        Corners::Square => 0.0,
    };
    let direction = |side: usize| {
        let (a, b) = (square[side], square[(side + 1) % 4]);
        let len = (b.x - a.x).hypot(b.y - a.y);
        ((b.x - a.x) / len, (b.y - a.y) / len)
    };
    let quarter = (std::f64::consts::FRAC_PI_8).tan();
    let mut points = Vec::with_capacity(8);
    let mut bulges = Vec::with_capacity(8);
    for (corner, &p) in square.iter().enumerate() {
        let (before, after) = ((corner + 3) % 4, corner);
        let (e_in, e_out) = (direction(before), direction(after));
        let (r_in, r_out) = (reach(before), reach(after));
        points.push(Point::new(p.x - e_in.0 * r_in, p.y - e_in.1 * r_in));
        points.push(Point::new(p.x + e_out.0 * r_out, p.y + e_out.1 * r_out));
        bulges.push(match corners {
            Corners::Fillet(_) if left => quarter,
            Corners::Fillet(_) => -quarter,
            _ => 0.0,
        });
        bulges.push(0.0);
    }
    if bulges.iter().all(|b| *b == 0.0) {
        bulges.clear();
    }
    (points, bulges)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;
    use crate::bulge;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn parts(e: Entity) -> (Vec<Point>, Vec<f64>, Vec<(f64, f64)>) {
        let Entity::Polyline {
            points,
            closed,
            bulges,
            widths,
        } = e
        else {
            panic!("a polyline, got {e:?}")
        };
        assert!(closed);
        (points, bulges, widths)
    }

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).hypot(a.y - b.y) < 1e-9
    }

    #[test]
    fn a_plain_axis_aligned_rectangle_stays_a_rectangle_whichever_way_it_is_drawn() {
        let want = Entity::Rectangle {
            min: p(2.0, 3.0),
            max: p(12.0, 7.0),
        };
        assert_eq!(
            outline(p(2.0, 3.0), 0.0, 10.0, 4.0, Corners::Square, 0.0),
            want
        );
        assert_eq!(
            outline(p(12.0, 7.0), 0.0, -10.0, -4.0, Corners::Square, 0.0),
            want
        );
    }

    #[test]
    fn rotation_turns_it_about_the_first_corner() {
        let (points, bulges, widths) = parts(outline(
            p(1.0, 1.0),
            FRAC_PI_2,
            4.0,
            2.0,
            Corners::Square,
            0.0,
        ));
        assert!(near(points[0], p(1.0, 1.0)) && near(points[1], p(1.0, 5.0)));
        assert!(near(points[2], p(-1.0, 5.0)) && near(points[3], p(-1.0, 1.0)));
        assert!(bulges.is_empty() && widths.is_empty());
    }

    #[test]
    fn a_width_makes_it_a_polyline_with_that_width_on_every_side() {
        let (points, _, widths) = parts(outline(p(0.0, 0.0), 0.0, 4.0, 2.0, Corners::Square, 0.5));
        assert_eq!(points.len(), 4);
        assert_eq!(widths, vec![(0.5, 0.5); 4]);
    }

    #[test]
    fn fillets_round_the_corners_with_quarter_circle_arcs() {
        let (points, bulges, _) = parts(outline(
            p(0.0, 0.0),
            0.0,
            10.0,
            6.0,
            Corners::Fillet(1.0),
            0.0,
        ));
        assert_eq!(points.len(), 8);
        assert!(near(points[0], p(0.0, 1.0)) && near(points[1], p(1.0, 0.0)));
        assert!(near(points[2], p(9.0, 0.0)) && near(points[3], p(10.0, 1.0)));
        // Counter-clockwise corners bulge positively, and each arc is a quarter circle.
        assert!((bulges[0] - std::f64::consts::FRAC_PI_8.tan()).abs() < 1e-9);
        assert!(bulges[1] == 0.0);
        let arc = bulge::arc_of(points[0], points[1], bulges[0]).unwrap();
        assert!(near(arc.center, p(1.0, 1.0)) && (arc.radius - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_rectangle_drawn_the_other_way_round_has_its_arcs_turn_the_other_way() {
        let (_, bulges, _) = parts(outline(
            p(0.0, 0.0),
            0.0,
            -10.0,
            6.0,
            Corners::Fillet(1.0),
            0.0,
        ));
        assert!(bulges[0] < 0.0);
    }

    #[test]
    fn chamfers_use_the_first_distance_along_x_and_the_second_along_y() {
        let (points, bulges, _) = parts(outline(
            p(0.0, 0.0),
            0.0,
            10.0,
            6.0,
            Corners::Chamfer(2.0, 1.0),
            0.0,
        ));
        assert!(bulges.is_empty());
        assert!(near(points[0], p(0.0, 1.0)) && near(points[1], p(2.0, 0.0)));
        assert!(near(points[2], p(8.0, 0.0)) && near(points[3], p(10.0, 1.0)));
    }

    #[test]
    fn corners_that_do_not_fit_are_left_square() {
        let square = |c| outline(p(0.0, 0.0), 0.0, 10.0, 6.0, c, 0.0);
        let plain = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(10.0, 6.0),
        };
        assert_eq!(
            square(Corners::Fillet(3.0)),
            plain,
            "a radius of half the short side"
        );
        assert_eq!(square(Corners::Fillet(0.0)), plain);
        assert_eq!(square(Corners::Chamfer(0.0, 0.0)), plain);
        assert_eq!(square(Corners::Chamfer(5.0, 1.0)), plain);
        assert_eq!(
            square(Corners::Chamfer(2.0, 0.0)),
            plain,
            "a bevel with no depth"
        );
    }
}
