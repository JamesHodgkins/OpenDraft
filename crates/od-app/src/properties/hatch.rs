//! The Properties rows of a hatch, in AutoCAD's words. The island style,
//! the pattern's angle, scale (or spacing and doubling, for a user-defined
//! pattern) and origin can be typed over; the boundary is edited with grips
//! and the fill kind is chosen by HATCH.

use od_core::hatch::{HatchEntity, HatchFill, HatchPattern, HatchStyle, PatternType};
use od_core::Point;

use super::{row, Field, Row, Unit};

const NAMED: &[Row] = &[
    row(Field::FillKind, "Fill", Unit::Fill, false),
    row(Field::HatchStyle, "Island style", Unit::Style, true),
    row(Field::Angle, "Angle", Unit::Angle, true),
    row(Field::Scale, "Scale", Unit::Factor, true),
    row(Field::X1, "Origin X", Unit::Length, true),
    row(Field::Y1, "Origin Y", Unit::Length, true),
    row(Field::Loops, "Boundaries", Unit::Count, false),
    row(Field::Associative, "Associative", Unit::Flag, false),
    row(Field::Area, "Area", Unit::Area, false),
];

const USER: &[Row] = &[
    row(Field::FillKind, "Fill", Unit::Fill, false),
    row(Field::HatchStyle, "Island style", Unit::Style, true),
    row(Field::Angle, "Angle", Unit::Angle, true),
    row(Field::Spacing, "Spacing", Unit::Length, true),
    row(Field::Double, "Double", Unit::Flag, true),
    row(Field::X1, "Origin X", Unit::Length, true),
    row(Field::Y1, "Origin Y", Unit::Length, true),
    row(Field::Loops, "Boundaries", Unit::Count, false),
    row(Field::Associative, "Associative", Unit::Flag, false),
    row(Field::Area, "Area", Unit::Area, false),
];

const FILLED: &[Row] = &[
    row(Field::FillKind, "Fill", Unit::Fill, false),
    row(Field::HatchStyle, "Island style", Unit::Style, true),
    row(Field::Loops, "Boundaries", Unit::Count, false),
    row(Field::Associative, "Associative", Unit::Flag, false),
    row(Field::Area, "Area", Unit::Area, false),
];

pub(super) fn rows(h: &HatchEntity) -> &'static [Row] {
    match &h.fill {
        HatchFill::Pattern(p) if p.pattern_type == PatternType::UserDefined => USER,
        HatchFill::Pattern(_) => NAMED,
        _ => FILLED,
    }
}

fn flag(on: bool) -> f64 {
    if on {
        1.0
    } else {
        0.0
    }
}

/// A thousandth of the hatch's size: how closely its curves are followed.
fn tolerance(h: &HatchEntity) -> f64 {
    h.bounds().map_or(1e-9, |b| {
        ((b.max.x - b.min.x).hypot(b.max.y - b.min.y) * 1e-3).max(1e-9)
    })
}

pub(super) fn value(h: &HatchEntity, field: Field) -> f64 {
    let pattern = match &h.fill {
        HatchFill::Pattern(p) => Some(p),
        _ => None,
    };
    match field {
        Field::FillKind => match h.fill {
            HatchFill::Pattern(_) => 0.0,
            HatchFill::Solid => 1.0,
            HatchFill::Gradient(_) => 2.0,
        },
        Field::HatchStyle => h.style.code() as f64,
        Field::Angle => pattern.map_or(f64::NAN, |p| p.angle.to_degrees()),
        Field::Scale | Field::Spacing => pattern.map_or(f64::NAN, |p| p.scale_or_spacing),
        Field::Double => pattern.map_or(f64::NAN, |p| flag(p.double)),
        Field::X1 => pattern.map_or(f64::NAN, |p| p.origin.x),
        Field::Y1 => pattern.map_or(f64::NAN, |p| p.origin.y),
        Field::Loops => h.loops.len() as f64,
        Field::Associative => flag(h.associative),
        Field::Area => h.area(tolerance(h)).unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

pub(super) fn with_value(h: &HatchEntity, field: Field, new: f64) -> Option<HatchEntity> {
    let mut out = h.clone();
    if field == Field::HatchStyle {
        out.style = HatchStyle::from_code(new.round() as i64)?;
        return Some(out);
    }
    let HatchFill::Pattern(p) = &h.fill else {
        return None;
    };
    let user = p.pattern_type == PatternType::UserDefined;
    out.fill = HatchFill::Pattern(match field {
        Field::Angle => p.with_angle(new.to_radians()).ok()?,
        Field::Scale if !user => p.with_scale(new).ok()?,
        Field::Spacing if user => {
            HatchPattern::user_defined(new, p.angle, p.double, p.origin).ok()?
        }
        Field::Double if user => {
            HatchPattern::user_defined(p.scale_or_spacing, p.angle, new != 0.0, p.origin).ok()?
        }
        Field::X1 => p.with_origin(Point::new(new, p.origin.y)).ok()?,
        Field::Y1 => p.with_origin(Point::new(p.origin.x, new)).ok()?,
        _ => return None,
    });
    Some(out)
}

#[cfg(test)]
mod tests {
    use od_core::hatch::{builtin, HatchBoundary, HatchLoop, HatchLoopFlags, HatchPolyline};
    use od_core::{Entity, Units};

    use super::super::{format, parse, rows, value, with_value, Field, Unit};
    use super::*;

    fn square(x: f64, side: f64) -> HatchLoop {
        let p = Point::new;
        HatchLoop {
            flags: HatchLoopFlags::default(),
            boundary: HatchBoundary::Polyline(HatchPolyline {
                vertices: vec![
                    p(x, x),
                    p(x + side, x),
                    p(x + side, x + side),
                    p(x, x + side),
                ],
                bulges: Vec::new(),
                closed: true,
            }),
            sources: Vec::new(),
        }
    }

    fn ansi31() -> Entity {
        let def = builtin().find("ANSI31").unwrap();
        let p = HatchPattern::from_definition(
            def,
            PatternType::Predefined,
            2.0,
            0.0,
            Point::new(1.0, 1.0),
        )
        .unwrap();
        Entity::Hatch(Box::new(HatchEntity::new(
            vec![square(0.0, 20.0), square(5.0, 10.0)],
            HatchFill::Pattern(p),
        )))
    }

    #[test]
    fn a_named_pattern_shows_and_edits_angle_scale_origin_and_style() {
        let h = ansi31();
        let shown: Vec<&str> = rows(&h).iter().map(|r| r.label).collect();
        assert_eq!(shown[..4], ["Fill", "Island style", "Angle", "Scale"]);
        assert_eq!(value(&h, Field::Scale), 2.0);
        assert_eq!(value(&h, Field::Loops), 2.0);
        // 20 x 20 with a 10 x 10 island.
        assert!((value(&h, Field::Area) - 300.0).abs() < 1e-6);
        let turned = with_value(&h, Field::Angle, 30.0).unwrap();
        assert!((value(&turned, Field::Angle) - 30.0).abs() < 1e-9);
        let bigger = with_value(&h, Field::Scale, 4.0).unwrap();
        assert_eq!(value(&bigger, Field::Scale), 4.0);
        let moved = with_value(&h, Field::X1, 3.0).unwrap();
        assert_eq!(value(&moved, Field::X1), 3.0);
        assert!(with_value(&h, Field::Scale, 0.0).is_none());
        assert!(
            with_value(&h, Field::Spacing, 1.0).is_none(),
            "not user-defined"
        );
        assert!(with_value(&h, Field::Area, 1.0).is_none(), "derived");
        // The island style changes the area: Ignore fills the island too.
        let ignore = with_value(&h, Field::HatchStyle, 2.0).unwrap();
        assert!((value(&ignore, Field::Area) - 400.0).abs() < 1e-6);
        assert!(with_value(&h, Field::HatchStyle, 3.0).is_none());
    }

    #[test]
    fn a_user_defined_pattern_edits_spacing_and_double() {
        let user = HatchPattern::user_defined(2.0, 0.0, false, Point::default()).unwrap();
        let h = Entity::Hatch(Box::new(HatchEntity::new(
            vec![square(0.0, 10.0)],
            HatchFill::Pattern(user),
        )));
        assert_eq!(rows(&h)[3].label, "Spacing");
        let wider = with_value(&h, Field::Spacing, 5.0).unwrap();
        assert_eq!(value(&wider, Field::Spacing), 5.0);
        let double = with_value(&h, Field::Double, 1.0).unwrap();
        let Entity::Hatch(d) = &double else {
            panic!("hatch");
        };
        let HatchFill::Pattern(p) = &d.fill else {
            panic!("pattern");
        };
        assert_eq!(p.lines.len(), 2);
        assert!(with_value(&h, Field::Scale, 3.0).is_none());
    }

    #[test]
    fn a_solid_hatch_only_has_its_style_and_derived_rows() {
        let h = Entity::Hatch(Box::new(HatchEntity::new(
            vec![square(0.0, 10.0)],
            HatchFill::Solid,
        )));
        assert_eq!(rows(&h).len(), 5);
        assert!(value(&h, Field::Angle).is_nan());
        assert!(with_value(&h, Field::Angle, 10.0).is_none());
        let units = Units::default();
        assert_eq!(
            format(&units, Unit::Fill, value(&h, Field::FillKind)),
            "Solid"
        );
        assert_eq!(format(&units, Unit::Style, 1.0), "Outer");
        assert_eq!(parse(&units, Unit::Style, "ignore"), Some(2.0));
        assert_eq!(parse(&units, Unit::Style, "1"), Some(1.0));
        assert_eq!(parse(&units, Unit::Style, "diagonal"), None);
    }
}
