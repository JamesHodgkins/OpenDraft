//! The Properties rows of an ellipse or elliptical arc, in AutoCAD's words.
//! A parameter is not an angle (see `od_core::ellipse`), so both are shown
//! and labelled for what they are; angles and parameters are degrees here.

use std::f64::consts::TAU;

use od_core::{arc, Ellipse, Point};

use super::{row, Field, Row, Unit};

const FULL: &[Row] = &[
    row(Field::X1, "Center X", Unit::Length, true),
    row(Field::Y1, "Center Y", Unit::Length, true),
    row(Field::MajorRadius, "Major radius", Unit::Length, true),
    row(Field::MinorRadius, "Minor radius", Unit::Length, true),
    row(Field::Angle, "Rotation", Unit::Angle, true),
    row(Field::RadiusRatio, "Radius ratio", Unit::Factor, false),
    row(Field::Circumference, "Circumference", Unit::Length, false),
    row(Field::Area, "Area", Unit::Area, false),
];

const ARC: &[Row] = &[
    row(Field::X1, "Center X", Unit::Length, true),
    row(Field::Y1, "Center Y", Unit::Length, true),
    row(Field::MajorRadius, "Major radius", Unit::Length, true),
    row(Field::MinorRadius, "Minor radius", Unit::Length, true),
    row(Field::Angle, "Rotation", Unit::Angle, true),
    row(Field::RadiusRatio, "Radius ratio", Unit::Factor, false),
    row(Field::StartAngle, "Start angle", Unit::Angle, true),
    row(Field::EndAngle, "End angle", Unit::Angle, true),
    row(Field::StartParameter, "Start parameter", Unit::Angle, true),
    row(Field::EndParameter, "End parameter", Unit::Angle, true),
    row(Field::Length, "Length", Unit::Length, false),
    row(Field::Area, "Area", Unit::Area, false),
];

pub(super) fn rows(e: &Ellipse) -> &'static [Row] {
    if e.is_full() {
        FULL
    } else {
        ARC
    }
}

/// Degrees in `[0, 360)`.
fn degrees(radians: f64) -> f64 {
    arc::normalize(radians).to_degrees()
}

pub(super) fn value(e: &Ellipse, field: Field) -> f64 {
    match field {
        Field::X1 => e.center.x,
        Field::Y1 => e.center.y,
        Field::MajorRadius => e.major_radius(),
        Field::MinorRadius => e.minor_radius(),
        Field::RadiusRatio => e.radius_ratio,
        Field::Angle => degrees(e.rotation()),
        Field::StartAngle => degrees(e.angle_at(e.start)),
        Field::EndAngle => degrees(e.angle_at(e.end)),
        Field::StartParameter => degrees(e.start),
        Field::EndParameter => degrees(e.end),
        Field::Length | Field::Circumference => e.length(),
        Field::Area => e.area(),
        _ => f64::NAN,
    }
}

/// The arc with one end's parameter changed, going round once more when the
/// end would fall before the start.
fn with_ends(e: &Ellipse, start: f64, end: f64) -> Option<Ellipse> {
    let end = if end <= start { end + TAU } else { end };
    Ellipse::new(e.center, e.major_axis, e.radius_ratio, start, end).ok()
}

/// `e` with `field` set to `new` (degrees for angles and parameters), or
/// `None` when that makes no ellipse or the field is derived. A radius that
/// passes the other one swaps which axis is the major one.
pub(super) fn with_value(e: &Ellipse, field: Field, new: f64) -> Option<Ellipse> {
    let a = e.major_radius();
    let unit = Point::new(e.major_axis.x / a, e.major_axis.y / a);
    match field {
        Field::X1 => Some(e.translated(new - e.center.x, 0.0)),
        Field::Y1 => Some(e.translated(0.0, new - e.center.y)),
        Field::MajorRadius if new > 0.0 => {
            let axis = Point::new(unit.x * new, unit.y * new);
            Ellipse::from_axes(e.center, axis, e.minor_radius(), e.start, e.end).ok()
        }
        Field::MinorRadius if new > 0.0 => {
            Ellipse::from_axes(e.center, e.major_axis, new, e.start, e.end).ok()
        }
        Field::Angle => Some(e.rotated(e.center, new.to_radians() - e.rotation())),
        Field::StartAngle => with_ends(e, e.parameter_at_angle(new.to_radians()), e.end),
        Field::EndAngle => with_ends(e, e.start, e.parameter_at_angle(new.to_radians())),
        Field::StartParameter => with_ends(e, new.to_radians(), e.end),
        Field::EndParameter => with_ends(e, e.start, new.to_radians()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn a_full_ellipse_shows_its_radii_rotation_and_size() {
        let e = Ellipse::full(p(1.0, 2.0), p(0.0, 4.0), 0.5).unwrap();
        assert_eq!(rows(&e).len(), 8);
        assert_eq!(value(&e, Field::MajorRadius), 4.0);
        assert_eq!(value(&e, Field::MinorRadius), 2.0);
        assert!((value(&e, Field::Angle) - 90.0).abs() < 1e-9);
        assert!((value(&e, Field::Area) - 8.0 * PI).abs() < 1e-9);
        assert!(rows(&e).iter().all(|r| r.field != Field::StartParameter));
    }

    #[test]
    fn radii_edits_keep_the_ellipse_and_may_swap_its_axes() {
        let e = Ellipse::full(p(0.0, 0.0), p(4.0, 0.0), 0.5).unwrap();
        let wider = with_value(&e, Field::MajorRadius, 6.0).unwrap();
        assert!(
            (wider.major_radius() - 6.0).abs() < 1e-12
                && (wider.minor_radius() - 2.0).abs() < 1e-12
        );
        let swapped = with_value(&e, Field::MinorRadius, 5.0).unwrap();
        assert!(
            (swapped.major_radius() - 5.0).abs() < 1e-12
                && (swapped.minor_radius() - 4.0).abs() < 1e-12
        );
        assert!(
            (swapped.rotation() - PI / 2.0).abs() < 1e-12,
            "the major axis is now upright"
        );
        assert_eq!(with_value(&e, Field::MinorRadius, 0.0), None);
        assert_eq!(with_value(&e, Field::RadiusRatio, 0.2), None, "derived");
        let turned = with_value(&e, Field::Angle, 30.0).unwrap();
        assert!((value(&turned, Field::Angle) - 30.0).abs() < 1e-9);
    }

    #[test]
    fn an_arc_shows_and_edits_angles_and_parameters_apart() {
        // A 4-by-1 ellipse: the point at parameter 45° is not at 45°.
        let e = Ellipse::new(p(0.0, 0.0), p(4.0, 0.0), 0.25, PI / 4.0, PI).unwrap();
        assert!((value(&e, Field::StartParameter) - 45.0).abs() < 1e-9);
        let seen = (0.25f64).atan().to_degrees();
        assert!((value(&e, Field::StartAngle) - seen).abs() < 1e-9);
        let moved = with_value(&e, Field::StartAngle, 90.0).unwrap();
        assert!(
            (moved.start - PI / 2.0).abs() < 1e-9,
            "straight up is a quarter turn either way"
        );
        let shorter = with_value(&e, Field::EndParameter, 120.0).unwrap();
        assert!((shorter.end - 120f64.to_radians()).abs() < 1e-12);
        let closed = with_value(&e, Field::EndParameter, 45.0).unwrap();
        assert!(closed.is_full(), "an end on the start closes it");
        assert!(value(&e, Field::Length) > 0.0);
    }
}
