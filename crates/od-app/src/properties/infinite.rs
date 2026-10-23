//! The Properties rows of a construction line or ray: the base point and the
//! direction as an angle in degrees, which is the one value typing changes.

use od_core::{infinite, Entity, Point};

use super::{row, Field, Row, Unit};

pub(super) const ROWS: &[Row] = &[
    row(Field::X1, "Base X", Unit::Length, true),
    row(Field::Y1, "Base Y", Unit::Length, true),
    row(Field::Angle, "Angle", Unit::Angle, true),
];

pub(super) fn value(origin: Point, direction: Point, field: Field) -> f64 {
    match field {
        Field::X1 => origin.x,
        Field::Y1 => origin.y,
        Field::Angle => direction.y.atan2(direction.x).to_degrees(),
        _ => f64::NAN,
    }
}

/// The line with `field` set to `new`, in the same kind.
pub(super) fn with_value(entity: &Entity, field: Field, new: f64) -> Option<Entity> {
    let (origin, direction, _) = entity.infinite_line()?;
    let (origin, direction) = match field {
        Field::X1 => (Point::new(new, origin.y), direction),
        Field::Y1 => (Point::new(origin.x, new), direction),
        Field::Angle => (origin, infinite::from_angle(new.to_radians())),
        _ => return None,
    };
    Some(entity.with_infinite_line(origin, direction))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_moves_the_base_or_turns_the_line() {
        let line = Entity::Xline {
            origin: Point::new(1.0, 2.0),
            direction: Point::new(1.0, 0.0),
        };
        let moved = with_value(&line, Field::X1, 5.0).unwrap();
        assert_eq!(moved.infinite_line().unwrap().0, Point::new(5.0, 2.0));
        let turned = with_value(&line, Field::Angle, 90.0).unwrap();
        let (_, d, ray) = turned.infinite_line().unwrap();
        assert!(d.x.abs() < 1e-12 && (d.y - 1.0).abs() < 1e-12 && !ray);
        assert_eq!(value(Point::new(1.0, 2.0), d, Field::Angle), 90.0);
        assert_eq!(with_value(&line, Field::Length, 1.0), None);
    }
}
