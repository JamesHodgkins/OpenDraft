//! Standard parts as exact geometry. A recipe takes the numbers that describe
//! a part (the ones a drawing or a standard gives) and returns the shapes that
//! draw it, in a plane with the part's own origin. Nothing here knows about
//! documents or callers; it is plain maths, tested to the number, so whoever
//! asks for a part (a person or an assistant) never has to work a coordinate
//! out by hand.

mod weld_neck_flange;
#[cfg(test)]
mod weld_neck_flange_tests;

pub use weld_neck_flange::WeldNeckFlange;

use crate::Point;

/// A piece of a drawn part.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Line(Point, Point),
    Circle {
        center: Point,
        radius: f64,
    },
    /// A closed outline through these corners, in order.
    Outline(Vec<Point>),
}

impl Shape {
    /// The same shape moved by (`dx`, `dy`).
    pub fn moved(&self, dx: f64, dy: f64) -> Shape {
        let by = |p: &Point| Point::new(p.x + dx, p.y + dy);
        match self {
            Shape::Line(a, b) => Shape::Line(by(a), by(b)),
            Shape::Circle { center, radius } => Shape::Circle {
                center: by(center),
                radius: *radius,
            },
            Shape::Outline(points) => Shape::Outline(points.iter().map(by).collect()),
        }
    }
}

/// Why a part cannot be drawn from the numbers given: worded for the person
/// or assistant who gave them, saying which numbers disagree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartError(pub String);

impl std::fmt::Display for PartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Each of `values` is a positive, finite number no larger than a sane
/// drawing: the checks every recipe starts with.
pub(crate) fn positive(values: &[(&str, f64)]) -> Result<(), PartError> {
    for (name, value) in values {
        if !value.is_finite() || *value <= 0.0 || *value > 1.0e7 {
            return Err(PartError(format!(
                "{name} must be a positive number (it was {value})."
            )));
        }
    }
    Ok(())
}
