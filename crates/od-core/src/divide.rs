//! DIVIDE and MEASURE: where marks go along a curve. Both work on the
//! [`Path`] the path array uses, so they follow lines, arcs, circles,
//! polylines, rectangles, ellipses and splines alike. Each mark has the
//! direction the curve runs there, for a block placed along it.

use crate::array::Path;
use crate::Point;

/// The most segments DIVIDE accepts.
pub const MAX_SEGMENTS: usize = 32_767;
/// The most marks MEASURE makes.
pub const MAX_MARKS: usize = 100_000;

/// A mark on a curve: where, and the direction (radians) the curve runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub at: Point,
    pub direction: f64,
}

fn mark(path: &Path, s: f64) -> Mark {
    let (at, direction) = path.at(s);
    Mark { at, direction }
}

/// The marks that cut the curve into `segments` equal parts: the points
/// between them, or on a closed curve (which has no ends) one at its start
/// too. `None` unless `segments` is from 2 to [`MAX_SEGMENTS`].
pub fn divide(path: &Path, segments: usize) -> Option<Vec<Mark>> {
    if !(2..=MAX_SEGMENTS).contains(&segments) {
        return None;
    }
    let step = path.length() / segments as f64;
    let first = usize::from(!path.is_closed());
    Some(
        (first..segments)
            .map(|k| mark(path, step * k as f64))
            .collect(),
    )
}

/// The marks every `distance` along the curve from its start, the first one
/// `distance` from it. A mark that would fall on the end of a closed curve is
/// the start again and is left out. `None` for a distance that is not above
/// zero or one that makes more than [`MAX_MARKS`] marks.
pub fn measure(path: &Path, distance: f64) -> Option<Vec<Mark>> {
    if !(distance.is_finite() && distance > 0.0) {
        return None;
    }
    // A flattened curve is a little shorter than the curve itself.
    let slack = distance * 1e-4;
    let length = path.length();
    if length / distance > MAX_MARKS as f64 {
        return None;
    }
    let mut marks = Vec::new();
    let mut k = 1.0;
    while distance * k <= length + slack {
        if path.is_closed() && distance * k >= length - slack {
            break;
        }
        marks.push(mark(path, distance * k));
        k += 1.0;
    }
    Some(marks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Entity;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn path(e: Entity) -> Path {
        Path::of(&e).unwrap()
    }

    fn line() -> Path {
        path(Entity::Line {
            start: p(0.0, 0.0),
            end: p(10.0, 0.0),
        })
    }

    fn xs(marks: &[Mark]) -> Vec<f64> {
        marks.iter().map(|m| (m.at.x * 1e6).round() / 1e6).collect()
    }

    #[test]
    fn dividing_a_line_marks_the_points_between_the_parts() {
        let marks = divide(&line(), 4).unwrap();
        assert_eq!(xs(&marks), [2.5, 5.0, 7.5]);
        assert_eq!(marks[0].direction, 0.0);
        assert!(divide(&line(), 1).is_none());
        assert!(divide(&line(), MAX_SEGMENTS + 1).is_none());
    }

    #[test]
    fn dividing_a_circle_marks_the_start_too() {
        let circle = path(Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        });
        let marks = divide(&circle, 4).unwrap();
        assert_eq!(marks.len(), 4);
        assert!((marks[0].at.x - 1.0).abs() < 1e-9 && marks[0].at.y.abs() < 1e-9);
        assert!((marks[1].at.y - 1.0).abs() < 1e-3);
    }

    #[test]
    fn measuring_marks_every_distance_from_the_start() {
        assert_eq!(xs(&measure(&line(), 3.0).unwrap()), [3.0, 6.0, 9.0]);
        assert_eq!(xs(&measure(&line(), 5.0).unwrap()), [5.0, 10.0]);
        assert!(measure(&line(), 11.0).unwrap().is_empty());
        assert!(measure(&line(), 0.0).is_none());
        assert!(measure(&line(), 1e-9).is_none(), "too many marks");
    }

    #[test]
    fn a_closed_curve_does_not_get_its_start_marked_again() {
        let square = path(Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(10.0, 10.0),
        });
        let marks = measure(&square, 10.0).unwrap();
        assert_eq!(marks.len(), 3, "the fourth would be the start");
        assert_eq!(marks[0].at, p(10.0, 0.0));
    }
}
