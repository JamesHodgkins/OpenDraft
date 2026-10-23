//! The Properties rows of a spline, in AutoCAD's words. All are read-only
//! for now: changing the degree, closing a spline or its fit tolerance needs
//! SPLINEDIT's geometry, and the control frame is edited with grips.

use od_core::Spline;

use super::{row, Field, Row, Unit};

const CONTROL: &[Row] = &[
    row(Field::Degree, "Degree", Unit::Count, false),
    row(Field::Closed, "Closed", Unit::Flag, false),
    row(Field::Periodic, "Periodic", Unit::Flag, false),
    row(Field::Rational, "Rational", Unit::Flag, false),
    row(
        Field::ControlVertices,
        "Control vertices",
        Unit::Count,
        false,
    ),
    row(Field::Knots, "Knots", Unit::Count, false),
    row(Field::X1, "Start X", Unit::Length, false),
    row(Field::Y1, "Start Y", Unit::Length, false),
    row(Field::X2, "End X", Unit::Length, false),
    row(Field::Y2, "End Y", Unit::Length, false),
    row(Field::Length, "Length", Unit::Length, false),
];

const FITTED: &[Row] = &[
    row(Field::Degree, "Degree", Unit::Count, false),
    row(Field::Closed, "Closed", Unit::Flag, false),
    row(Field::Periodic, "Periodic", Unit::Flag, false),
    row(Field::Rational, "Rational", Unit::Flag, false),
    row(
        Field::ControlVertices,
        "Control vertices",
        Unit::Count,
        false,
    ),
    row(Field::Knots, "Knots", Unit::Count, false),
    row(Field::FitPoints, "Fit points", Unit::Count, false),
    row(Field::FitTolerance, "Fit tolerance", Unit::Length, false),
    row(Field::X1, "Start X", Unit::Length, false),
    row(Field::Y1, "Start Y", Unit::Length, false),
    row(Field::X2, "End X", Unit::Length, false),
    row(Field::Y2, "End Y", Unit::Length, false),
    row(Field::Length, "Length", Unit::Length, false),
];

pub(super) fn rows(s: &Spline) -> &'static [Row] {
    if s.fit.is_some() {
        FITTED
    } else {
        CONTROL
    }
}

fn flag(on: bool) -> f64 {
    if on {
        1.0
    } else {
        0.0
    }
}

pub(super) fn value(s: &Spline, field: Field) -> f64 {
    match field {
        Field::Degree => f64::from(s.degree),
        Field::Closed => flag(s.closed),
        Field::Periodic => flag(s.periodic),
        Field::Rational => flag(s.is_rational()),
        Field::ControlVertices => s.control_points.len() as f64,
        Field::Knots => s.knots.len() as f64,
        Field::FitPoints => s.fit.as_ref().map_or(0.0, |f| f.points.len() as f64),
        Field::FitTolerance => s.fit.as_ref().map_or(f64::NAN, |f| f.tolerance),
        Field::X1 => s.start_point().x,
        Field::Y1 => s.start_point().y,
        Field::X2 => s.end_point().x,
        Field::Y2 => s.end_point().y,
        // Too much work to measure (a huge spline) shows as no value.
        Field::Length => s.length().unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Entity, KnotParameterization, Point, Units};

    use super::super::{format, rows, value, with_value, Field};

    #[test]
    fn a_fit_spline_shows_its_counts_and_flags_and_nothing_is_typed_over() {
        let points = vec![
            Point::new(0.0, 0.0),
            Point::new(3.0, 4.0),
            Point::new(6.0, 0.0),
        ];
        let s = od_core::Spline::from_fit_points(points, KnotParameterization::Chord).unwrap();
        let e = Entity::Spline(Box::new(s));
        let shown: Vec<&str> = rows(&e).iter().map(|r| r.label).collect();
        assert!(shown.contains(&"Fit points") && shown.contains(&"Control vertices"));
        assert!(rows(&e).iter().all(|r| !r.editable));
        assert_eq!(value(&e, Field::Degree), 3.0);
        assert_eq!(value(&e, Field::ControlVertices), 5.0);
        assert_eq!(value(&e, Field::FitPoints), 3.0);
        assert_eq!((value(&e, Field::X2), value(&e, Field::Y2)), (6.0, 0.0));
        assert!(value(&e, Field::Length) > 10.0);
        let units = Units::default();
        let closed = rows(&e).iter().find(|r| r.field == Field::Closed).unwrap();
        assert_eq!(format(&units, closed.unit, value(&e, Field::Closed)), "No");
        assert_eq!(with_value(&e, Field::X1, 5.0), None);
    }
}
