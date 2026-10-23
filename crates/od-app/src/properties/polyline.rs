//! The Properties rows of a polyline: Closed and Global width can be typed
//! over (the same edits as PEDIT's Close/Open and Width); the rest is derived.

use od_core::pedit::PolyData;
use od_core::{bulge, Entity};

use super::{row, Field, Row, Unit};

pub(super) const ROWS: &[Row] = &[
    row(Field::Vertices, "Vertices", Unit::Count, false),
    row(Field::Closed, "Closed", Unit::Flag, true),
    row(Field::Width, "Global width", Unit::Length, true),
    row(Field::Length, "Length", Unit::Length, false),
    row(Field::Area, "Area", Unit::Area, false),
];

/// The width every segment has, if they all have the same (a hairline is 0).
fn global_width(widths: &[(f64, f64)]) -> Option<f64> {
    let first = widths.first().copied().unwrap_or((0.0, 0.0));
    (first.0 == first.1 && widths.iter().all(|&w| w == first)).then_some(first.0)
}

/// A field's value; not a number when the polyline has none (segments of
/// different widths have no global width).
pub(super) fn value(entity: &Entity, field: Field) -> f64 {
    let Entity::Polyline {
        points,
        closed,
        bulges,
        widths,
    } = entity
    else {
        return f64::NAN;
    };
    match field {
        Field::Vertices => points.len() as f64,
        Field::Closed => f64::from(u8::from(*closed)),
        Field::Width => {
            // The unused last slot of an open polyline does not count.
            let active = if *closed || widths.is_empty() {
                widths
            } else {
                &widths[..widths.len() - 1]
            };
            global_width(active).unwrap_or(f64::NAN)
        }
        Field::Length => bulge::path_length(points, bulges, *closed),
        Field::Area => bulge::path_area(points, bulges),
        _ => f64::NAN,
    }
}

/// The polyline with `field` set to `new`, or `None` when it is derived or
/// the value is not allowed.
pub(super) fn with_value(entity: &Entity, field: Field, new: f64) -> Option<Entity> {
    let data = PolyData::of(entity)?;
    match field {
        Field::Closed => Some(data.with_closed(new != 0.0).into_entity()),
        Field::Width if new >= 0.0 => Some(data.with_width(new).into_entity()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use od_core::Point;

    fn polyline(widths: Vec<(f64, f64)>) -> Entity {
        Entity::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(4.0, 0.0),
                Point::new(4.0, 3.0),
            ],
            closed: false,
            bulges: Vec::new(),
            widths,
        }
    }

    #[test]
    fn closed_and_global_width_are_typed_over() {
        let open = polyline(Vec::new());
        assert_eq!(value(&open, Field::Closed), 0.0);
        assert_eq!(value(&open, Field::Width), 0.0);
        let closed = with_value(&open, Field::Closed, 1.0).unwrap();
        assert_eq!(value(&closed, Field::Closed), 1.0);
        assert!(value(&closed, Field::Length) > value(&open, Field::Length));
        let wide = with_value(&closed, Field::Width, 2.0).unwrap();
        assert_eq!(value(&wide, Field::Width), 2.0);
        let hairline = with_value(&wide, Field::Width, 0.0).unwrap();
        assert!(matches!(hairline, Entity::Polyline { widths, .. } if widths.is_empty()));
    }

    #[test]
    fn segments_of_different_widths_have_no_global_width() {
        let mixed = polyline(vec![(1.0, 1.0), (2.0, 2.0), (0.0, 0.0)]);
        assert!(value(&mixed, Field::Width).is_nan());
        let same = polyline(vec![(1.0, 1.0), (1.0, 1.0), (0.0, 0.0)]);
        assert_eq!(
            value(&same, Field::Width),
            1.0,
            "the unused slot is ignored"
        );
        let tapered = polyline(vec![(1.0, 2.0), (2.0, 2.0), (0.0, 0.0)]);
        assert!(value(&tapered, Field::Width).is_nan());
    }

    #[test]
    fn a_negative_width_and_derived_fields_are_refused() {
        let open = polyline(Vec::new());
        assert_eq!(with_value(&open, Field::Width, -1.0), None);
        assert_eq!(with_value(&open, Field::Length, 5.0), None);
        assert_eq!(with_value(&open, Field::Vertices, 5.0), None);
    }
}
