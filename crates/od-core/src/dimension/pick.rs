//! Turning what the user picks into dimension points: the ends of an object
//! to dimension, and which way a linear dimension should run.

use std::f64::consts::FRAC_PI_2;

use super::sketch::{along, dist, sub, unit};
use super::{DimStyle, Dimension};
use crate::{arc, Curve, Entity, Point};

fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let v = sub(b, a);
    let len_sq = v.0 * v.0 + v.1 * v.1;
    if len_sq == 0.0 {
        return dist(p, a);
    }
    let t = (((p.x - a.x) * v.0 + (p.y - a.y) * v.1) / len_sq).clamp(0.0, 1.0);
    dist(p, along(a, v, t))
}

/// The two points to measure between for an object clicked at `at`: a line's
/// ends, an arc's ends, or the nearest straight segment of a polyline or
/// rectangle. `None` for anything else (a circle, a curved polyline
/// segment, another dimension).
pub fn object_ends(entity: &Entity, at: Point) -> Option<(Point, Point)> {
    match entity {
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => Some((
            arc::point_at(*center, *radius, *start),
            arc::point_at(*center, *radius, *end),
        )),
        Entity::Dimension(_) | Entity::Circle { .. } => None,
        other => other
            .curves()
            .into_iter()
            .filter_map(|c| match c {
                Curve::Segment { a, b } => Some((a, b)),
                _ => None,
            })
            .min_by(|x, y| {
                distance_to_segment(at, x.0, x.1).total_cmp(&distance_to_segment(at, y.0, y.1))
            }),
    }
}

/// Which way a linear dimension of `p1` to `p2` runs when its line is put
/// through `at`: horizontal (0) when `at` is above or below the points,
/// vertical (a quarter turn) when it is to the side. The diagonals of the
/// box the points span divide the two.
pub fn auto_rotation(p1: Point, p2: Point, at: Point) -> f64 {
    let centre = ((p1.x + p2.x) / 2.0, (p1.y + p2.y) / 2.0);
    let half = ((p1.x - p2.x).abs() / 2.0, (p1.y - p2.y).abs() / 2.0);
    let rel = ((at.x - centre.0).abs(), (at.y - centre.1).abs());
    if rel.0 * half.1 > rel.1 * half.0 || (half == (0.0, 0.0) && rel.0 > rel.1) {
        FRAC_PI_2
    } else {
        0.0
    }
}

impl Dimension {
    /// A radius or diameter of the circle of `radius` about `center`, its
    /// line pointing toward `toward`. Text clicked outside the circle stays
    /// where it was clicked. `None` for a radius that is not positive.
    pub fn radial_toward(
        center: Point,
        radius: f64,
        toward: Point,
        diameter: bool,
        style: &DimStyle,
    ) -> Option<Dimension> {
        if !(radius.is_finite() && radius > 1e-12) {
            return None;
        }
        let u = unit(sub(toward, center)).unwrap_or((1.0, 0.0));
        let chord = along(center, u, radius);
        let mut dim = if diameter {
            Dimension::diameter(center, chord, style)
        } else {
            Dimension::radius(center, chord, style)
        };
        if dist(center, toward) > radius {
            dim.text_pos = Some(toward);
        }
        Some(dim)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn a_line_gives_its_ends() {
        let line = Entity::Line {
            start: p(1.0, 2.0),
            end: p(5.0, 6.0),
        };
        assert_eq!(
            object_ends(&line, p(3.0, 4.0)),
            Some((p(1.0, 2.0), p(5.0, 6.0)))
        );
    }

    #[test]
    fn a_rectangle_gives_the_side_nearest_the_click() {
        let rect = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(10.0, 4.0),
        };
        let (a, b) = object_ends(&rect, p(5.0, 3.9)).unwrap();
        assert!((a.y - 4.0).abs() < 1e-12 && (b.y - 4.0).abs() < 1e-12);
    }

    #[test]
    fn a_circle_has_no_ends() {
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        };
        assert_eq!(object_ends(&circle, p(1.0, 0.0)), None);
    }

    #[test]
    fn an_arc_gives_its_end_points() {
        let a = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 10.0,
            start: 0.0,
            end: FRAC_PI_2,
        };
        let (s, e) = object_ends(&a, p(7.0, 7.0)).unwrap();
        assert!((s.x - 10.0).abs() < 1e-9 && (e.y - 10.0).abs() < 1e-9);
    }

    #[test]
    fn the_line_goes_across_the_way_the_cursor_is() {
        let (a, b) = (p(0.0, 0.0), p(10.0, 10.0));
        assert_eq!(auto_rotation(a, b, p(5.0, 30.0)), 0.0);
        assert_eq!(auto_rotation(a, b, p(30.0, 5.0)), FRAC_PI_2);
        assert_eq!(auto_rotation(p(0.0, 0.0), p(10.0, 0.0), p(5.0, 5.0)), 0.0);
        assert_eq!(
            auto_rotation(p(0.0, 0.0), p(0.0, 10.0), p(5.0, 5.0)),
            FRAC_PI_2
        );
    }

    #[test]
    fn a_click_outside_keeps_the_text_there() {
        let style = DimStyle::default();
        let inside =
            Dimension::radial_toward(p(0.0, 0.0), 10.0, p(3.0, 0.0), false, &style).unwrap();
        assert_eq!(inside.text_pos, None);
        assert_eq!(inside.measurement(), Some(10.0));
        let outside =
            Dimension::radial_toward(p(0.0, 0.0), 10.0, p(30.0, 0.0), true, &style).unwrap();
        assert_eq!(outside.text_pos, Some(p(30.0, 0.0)));
        assert_eq!(outside.measurement(), Some(20.0));
        assert!(Dimension::radial_toward(p(0.0, 0.0), 0.0, p(1.0, 0.0), false, &style).is_none());
    }
}
