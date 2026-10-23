//! What the Properties panel shows for an entity and how editing a value
//! changes it. Pure functions over [`Entity`]; the panel and undo live
//! elsewhere.

mod ellipse;
mod hatch;
mod infinite;
mod mleader;
mod spline;
mod unit;

pub use unit::{format, parse, Unit};

use od_core::{arc, bulge, Entity, Point};

/// One editable or derived value of an entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    /// First point: a line's start, a circle's center, a rectangle's corner.
    X1,
    Y1,
    /// Second point: a line's end, a rectangle's opposite corner.
    X2,
    Y2,
    Length,
    Angle,
    Radius,
    Diameter,
    Circumference,
    /// Where an arc starts and ends, in degrees counter-clockwise from east.
    StartAngle,
    EndAngle,
    /// Number of vertices of a polyline.
    Vertices,
    Width,
    Height,
    Area,
    /// What a dimension measures, before its scale factor and rounding.
    Measurement,
    /// A block reference's scale along its own x and y axes.
    ScaleX,
    ScaleY,
    /// An ellipse's radii and their ratio.
    MajorRadius,
    MinorRadius,
    RadiusRatio,
    /// Where an elliptical arc starts and ends as parameters (eccentric
    /// anomaly, in degrees), which are not the angles seen from the centre.
    StartParameter,
    EndParameter,
    /// A spline's degree, counts, shape and fit tolerance.
    Degree,
    ControlVertices,
    Knots,
    FitPoints,
    Closed,
    Periodic,
    Rational,
    FitTolerance,
    /// A hatch's fill kind, island style, pattern scale or spacing, doubling,
    /// number of boundaries and whether it follows them.
    FillKind,
    HatchStyle,
    Scale,
    Spacing,
    Double,
    Loops,
    Associative,
    /// A multileader's leader type, arrowhead and size, number of leader
    /// lines, landing, content kind and text frame.
    LeaderType,
    Arrowhead,
    ArrowSize,
    Lines,
    Dogleg,
    DoglegLength,
    LandingGap,
    ContentKind,
    Frame,
}

/// A row in the panel.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub field: Field,
    pub label: &'static str,
    pub unit: Unit,
    /// Derived values such as area can be read but not typed over.
    pub editable: bool,
}

const fn row(field: Field, label: &'static str, unit: Unit, editable: bool) -> Row {
    Row {
        field,
        label,
        unit,
        editable,
    }
}

const POINT: &[Row] = &[
    row(Field::X1, "Position X", Unit::Length, true),
    row(Field::Y1, "Position Y", Unit::Length, true),
];

const LINE: &[Row] = &[
    row(Field::X1, "Start X", Unit::Length, true),
    row(Field::Y1, "Start Y", Unit::Length, true),
    row(Field::X2, "End X", Unit::Length, true),
    row(Field::Y2, "End Y", Unit::Length, true),
    row(Field::Length, "Length", Unit::Length, true),
    row(Field::Angle, "Angle", Unit::Angle, true),
];

const CIRCLE: &[Row] = &[
    row(Field::X1, "Center X", Unit::Length, true),
    row(Field::Y1, "Center Y", Unit::Length, true),
    row(Field::Radius, "Radius", Unit::Length, true),
    row(Field::Diameter, "Diameter", Unit::Length, true),
    row(Field::Circumference, "Circumference", Unit::Length, false),
    row(Field::Area, "Area", Unit::Area, false),
];

const ARC: &[Row] = &[
    row(Field::X1, "Center X", Unit::Length, true),
    row(Field::Y1, "Center Y", Unit::Length, true),
    row(Field::Radius, "Radius", Unit::Length, true),
    row(Field::StartAngle, "Start angle", Unit::Angle, true),
    row(Field::EndAngle, "End angle", Unit::Angle, true),
    row(Field::Length, "Length", Unit::Length, false),
];

const RECTANGLE: &[Row] = &[
    row(Field::X1, "Corner 1 X", Unit::Length, true),
    row(Field::Y1, "Corner 1 Y", Unit::Length, true),
    row(Field::X2, "Corner 2 X", Unit::Length, true),
    row(Field::Y2, "Corner 2 Y", Unit::Length, true),
    row(Field::Width, "Width", Unit::Length, true),
    row(Field::Height, "Height", Unit::Length, true),
    row(Field::Area, "Area", Unit::Area, false),
];

const POLYLINE: &[Row] = &[
    row(Field::Vertices, "Vertices", Unit::Count, false),
    row(Field::Length, "Length", Unit::Length, false),
    row(Field::Area, "Area", Unit::Area, false),
];

const DIMENSION: &[Row] = &[row(Field::Measurement, "Measurement", Unit::Length, false)];

const LEADER: &[Row] = &[row(Field::Vertices, "Vertices", Unit::Count, false)];

const MTEXT: &[Row] = &[
    row(Field::X1, "Insertion X", Unit::Length, true),
    row(Field::Y1, "Insertion Y", Unit::Length, true),
    row(Field::Angle, "Rotation", Unit::Angle, true),
    row(Field::Height, "Text height", Unit::Length, true),
    row(Field::Width, "Width", Unit::Length, true),
];

const INSERT: &[Row] = &[
    row(Field::X1, "Position X", Unit::Length, true),
    row(Field::Y1, "Position Y", Unit::Length, true),
    row(Field::Angle, "Rotation", Unit::Angle, true),
    row(Field::ScaleX, "Scale X", Unit::Factor, true),
    row(Field::ScaleY, "Scale Y", Unit::Factor, true),
];

/// The rows shown for an entity, in display order.
pub fn rows(entity: &Entity) -> &'static [Row] {
    match entity {
        Entity::Point { .. } => POINT,
        Entity::Xline { .. } | Entity::Ray { .. } => infinite::ROWS,
        Entity::Line { .. } => LINE,
        Entity::Circle { .. } => CIRCLE,
        Entity::Arc { .. } => ARC,
        Entity::Ellipse(e) => ellipse::rows(e),
        Entity::Spline(s) => spline::rows(s),
        Entity::Rectangle { .. } => RECTANGLE,
        Entity::Polyline { .. } => POLYLINE,
        Entity::Dimension(_) => DIMENSION,
        Entity::MText(_) => MTEXT,
        Entity::MLeader(m) => mleader::rows(m),
        Entity::Leader(_) => LEADER,
        Entity::Insert(_) => INSERT,
        Entity::Hatch(h) => hatch::rows(h),
    }
}

/// Rows that every given entity has (same field and label), in the first
/// entity's order. Entities of different kinds currently share little, since
/// layer, colour and so on do not exist yet.
pub fn common_rows(entities: &[&Entity]) -> Vec<Row> {
    let Some((first, rest)) = entities.split_first() else {
        return Vec::new();
    };
    rows(first)
        .iter()
        .filter(|r| {
            rest.iter().all(|e| {
                rows(e)
                    .iter()
                    .any(|o| o.field == r.field && o.label == r.label)
            })
        })
        .copied()
        .collect()
}

/// Current value of `field`, in drawing units (degrees for angles).
pub fn value(entity: &Entity, field: Field) -> f64 {
    match entity {
        Entity::Dimension(d) => match field {
            Field::Measurement => d.measurement().unwrap_or(f64::NAN),
            _ => f64::NAN,
        },
        Entity::Leader(l) => match field {
            Field::Vertices => l.vertices.len() as f64,
            _ => f64::NAN,
        },
        Entity::MLeader(m) => mleader::value(m, field),
        Entity::Ellipse(e) => ellipse::value(e, field),
        Entity::Spline(s) => spline::value(s, field),
        Entity::Hatch(h) => hatch::value(h, field),
        Entity::Insert(i) => match field {
            Field::X1 => i.insertion.x,
            Field::Y1 => i.insertion.y,
            Field::Angle => i.rotation.to_degrees(),
            Field::ScaleX => i.scale_x,
            Field::ScaleY => i.scale_y,
            _ => f64::NAN,
        },
        Entity::MText(m) => match field {
            Field::X1 => m.insertion.x,
            Field::Y1 => m.insertion.y,
            Field::Angle => m.direction.y.atan2(m.direction.x).to_degrees(),
            Field::Height => m.nominal_height,
            Field::Width => m.reference_width,
            _ => f64::NAN,
        },
        Entity::Xline { origin, direction } | Entity::Ray { origin, direction } => {
            infinite::value(*origin, *direction, field)
        }
        Entity::Point { at } => match field {
            Field::X1 => at.x,
            Field::Y1 => at.y,
            _ => f64::NAN,
        },
        Entity::Line { start, end } => {
            let (dx, dy) = (end.x - start.x, end.y - start.y);
            match field {
                Field::X1 => start.x,
                Field::Y1 => start.y,
                Field::X2 => end.x,
                Field::Y2 => end.y,
                Field::Length => dx.hypot(dy),
                Field::Angle => dy.atan2(dx).to_degrees(),
                _ => f64::NAN,
            }
        }
        Entity::Circle { center, radius } => {
            let r = radius.abs();
            match field {
                Field::X1 => center.x,
                Field::Y1 => center.y,
                Field::Radius => r,
                Field::Diameter => r * 2.0,
                Field::Circumference => std::f64::consts::TAU * r,
                Field::Area => std::f64::consts::PI * r * r,
                _ => f64::NAN,
            }
        }
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => match field {
            Field::X1 => center.x,
            Field::Y1 => center.y,
            Field::Radius => radius.abs(),
            Field::StartAngle => start.to_degrees(),
            Field::EndAngle => end.to_degrees(),
            Field::Length => radius.abs() * arc::sweep(*start, *end),
            _ => f64::NAN,
        },
        Entity::Rectangle { min, max } => {
            let (w, h) = ((max.x - min.x).abs(), (max.y - min.y).abs());
            match field {
                Field::X1 => min.x,
                Field::Y1 => min.y,
                Field::X2 => max.x,
                Field::Y2 => max.y,
                Field::Width => w,
                Field::Height => h,
                Field::Area => w * h,
                _ => f64::NAN,
            }
        }
        Entity::Polyline {
            points,
            closed,
            bulges,
            ..
        } => match field {
            Field::Vertices => points.len() as f64,
            Field::Length => bulge::path_length(points, bulges, *closed),
            Field::Area => bulge::path_area(points, bulges),
            _ => f64::NAN,
        },
    }
}

/// The entity with `field` set to `new`, or `None` when the value is not
/// allowed (not a number, a size of zero or less) or the field is derived.
/// Resizing keeps the first point fixed.
pub fn with_value(entity: &Entity, field: Field, new: f64) -> Option<Entity> {
    if !new.is_finite() {
        return None;
    }
    let positive = new > 0.0;
    let mut out = entity.clone();
    match &mut out {
        // Their points and text are edited with grips and the text editor.
        Entity::Leader(_) => return None,
        Entity::MLeader(m) => **m = mleader::with_value(m, field, new)?,
        Entity::Xline { .. } | Entity::Ray { .. } => {
            return infinite::with_value(entity, field, new)
        }
        Entity::Point { at } => match field {
            Field::X1 => at.x = new,
            Field::Y1 => at.y = new,
            _ => return None,
        },
        Entity::Line { start, end } => {
            let (dx, dy) = (end.x - start.x, end.y - start.y);
            let (length, angle) = (dx.hypot(dy), dy.atan2(dx));
            let along = |len: f64, ang: f64| {
                Point::new(start.x + len * ang.cos(), start.y + len * ang.sin())
            };
            match field {
                Field::X1 => start.x = new,
                Field::Y1 => start.y = new,
                Field::X2 => end.x = new,
                Field::Y2 => end.y = new,
                Field::Length if positive => *end = along(new, angle),
                Field::Angle => *end = along(length, new.to_radians()),
                _ => return None,
            }
        }
        Entity::Circle { center, radius } => match field {
            Field::X1 => center.x = new,
            Field::Y1 => center.y = new,
            Field::Radius if positive => *radius = new,
            Field::Diameter if positive => *radius = new / 2.0,
            _ => return None,
        },
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => match field {
            Field::X1 => center.x = new,
            Field::Y1 => center.y = new,
            Field::Radius if positive => *radius = new,
            Field::StartAngle => *start = arc::normalize(new.to_radians()),
            Field::EndAngle => *end = arc::normalize(new.to_radians()),
            _ => return None,
        },
        Entity::Rectangle { min, max } => {
            let sign = |from: f64, to: f64| if to < from { -1.0 } else { 1.0 };
            match field {
                Field::X1 => min.x = new,
                Field::Y1 => min.y = new,
                Field::X2 => max.x = new,
                Field::Y2 => max.y = new,
                Field::Width if positive => max.x = min.x + sign(min.x, max.x) * new,
                Field::Height if positive => max.y = min.y + sign(min.y, max.y) * new,
                _ => return None,
            }
        }
        Entity::MText(m) => match field {
            Field::X1 => m.insertion.x = new,
            Field::Y1 => m.insertion.y = new,
            Field::Angle => {
                let angle = new.to_radians();
                m.direction = Point::new(angle.cos(), angle.sin());
            }
            Field::Height if positive => m.nominal_height = new,
            // Zero turns wrapping off.
            Field::Width if new >= 0.0 => m.reference_width = new,
            _ => return None,
        },
        Entity::Insert(i) => match field {
            Field::X1 => i.insertion.x = new,
            Field::Y1 => i.insertion.y = new,
            Field::Angle => i.rotation = arc::normalize(new.to_radians()),
            // Negative mirrors; zero would flatten the block.
            Field::ScaleX if new != 0.0 => i.scale_x = new,
            Field::ScaleY if new != 0.0 => i.scale_y = new,
            _ => return None,
        },
        Entity::Ellipse(e) => *e = ellipse::with_value(e, field, new)?,
        Entity::Hatch(h) => **h = hatch::with_value(h, field, new)?,
        Entity::Polyline { .. } | Entity::Dimension(_) | Entity::Spline(_) => return None,
    }
    Some(out)
}

/// A field's value across several entities: the shared value, or `None` when
/// they differ.
pub fn shared_value(entities: &[&Entity], field: Field) -> Option<f64> {
    let first = value(entities.first()?, field);
    entities
        .iter()
        .all(|e| (value(e, field) - first).abs() < 1e-9)
        .then_some(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use od_core::Units;

    fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> Entity {
        Entity::Line {
            start: Point::new(x1, y1),
            end: Point::new(x2, y2),
        }
    }

    fn rect(x1: f64, y1: f64, x2: f64, y2: f64) -> Entity {
        Entity::Rectangle {
            min: Point::new(x1, y1),
            max: Point::new(x2, y2),
        }
    }

    fn circle(r: f64) -> Entity {
        Entity::Circle {
            center: Point::new(1.0, 2.0),
            radius: r,
        }
    }

    #[test]
    fn an_arc_shows_its_angles_in_degrees_and_edits_them_into_range() {
        let a = Entity::Arc {
            center: Point::new(1.0, 2.0),
            radius: 5.0,
            start: 0.0,
            end: std::f64::consts::FRAC_PI_2,
        };
        assert!((value(&a, Field::EndAngle) - 90.0).abs() < 1e-9);
        let length = value(&a, Field::Length);
        assert!((length - 5.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        let Some(Entity::Arc { start, end, .. }) = with_value(&a, Field::StartAngle, -90.0) else {
            panic!("start angle is editable");
        };
        assert!(
            (start - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-9,
            "wrapped into range"
        );
        assert!((end - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert!(with_value(&a, Field::Radius, 0.0).is_none());
        assert!(
            with_value(&a, Field::Length, 3.0).is_none(),
            "length is derived"
        );
    }

    #[test]
    fn common_rows_keep_only_what_every_kind_has() {
        let (l, c, r) = (
            line(0.0, 0.0, 1.0, 1.0),
            circle(1.0),
            rect(0.0, 0.0, 1.0, 1.0),
        );
        assert!(common_rows(&[&l, &c]).is_empty());
        let shared: Vec<_> = common_rows(&[&c, &r]).iter().map(|r| r.field).collect();
        assert_eq!(shared, [Field::Area]);
        assert_eq!(common_rows(&[&c, &c]).len(), rows(&c).len());
        assert!(common_rows(&[]).is_empty());
    }

    #[test]
    fn polyline_reports_vertices_length_and_area() {
        let square = Entity::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(2.0, 0.0),
                Point::new(2.0, 2.0),
                Point::new(0.0, 2.0),
            ],
            closed: true,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        assert_eq!(value(&square, Field::Vertices), 4.0);
        assert_eq!(value(&square, Field::Length), 8.0);
        assert_eq!(value(&square, Field::Area), 4.0);
        assert!(
            with_value(&square, Field::Length, 3.0).is_none(),
            "read-only"
        );
    }

    #[test]
    fn line_reports_length_and_angle() {
        let e = line(0.0, 0.0, 3.0, 4.0);
        assert!((value(&e, Field::Length) - 5.0).abs() < 1e-12);
        assert!((value(&e, Field::Angle) - 53.130102).abs() < 1e-5);
    }

    #[test]
    fn circle_reports_diameter_and_area() {
        let e = circle(4.0);
        assert_eq!(value(&e, Field::Diameter), 8.0);
        assert!((value(&e, Field::Area) - 50.265482).abs() < 1e-5);
    }

    #[test]
    fn rectangle_size_ignores_corner_order() {
        let e = rect(10.0, 10.0, 4.0, 2.0);
        assert_eq!(value(&e, Field::Width), 6.0);
        assert_eq!(value(&e, Field::Height), 8.0);
    }

    #[test]
    fn line_length_keeps_start_and_direction() {
        let e = with_value(&line(1.0, 1.0, 4.0, 5.0), Field::Length, 10.0).unwrap();
        let Entity::Line { start, end } = e else {
            panic!()
        };
        assert_eq!(start, Point::new(1.0, 1.0));
        assert!((end.x - 7.0).abs() < 1e-9 && (end.y - 9.0).abs() < 1e-9);
    }

    #[test]
    fn line_angle_keeps_length() {
        let e = with_value(&line(0.0, 0.0, 5.0, 0.0), Field::Angle, 90.0).unwrap();
        assert!((value(&e, Field::Length) - 5.0).abs() < 1e-9);
        assert!((value(&e, Field::Y2) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn circle_diameter_sets_radius() {
        let e = with_value(&circle(1.0), Field::Diameter, 10.0).unwrap();
        assert_eq!(value(&e, Field::Radius), 5.0);
    }

    #[test]
    fn rectangle_width_keeps_corner_and_direction() {
        let e = with_value(&rect(10.0, 0.0, 4.0, 3.0), Field::Width, 2.0).unwrap();
        assert_eq!(value(&e, Field::X2), 8.0);
        assert_eq!(value(&e, Field::X1), 10.0);
    }

    #[test]
    fn sizes_must_be_positive_and_derived_fields_are_read_only() {
        assert!(with_value(&circle(1.0), Field::Radius, 0.0).is_none());
        assert!(with_value(&circle(1.0), Field::Radius, -3.0).is_none());
        assert!(with_value(&rect(0.0, 0.0, 1.0, 1.0), Field::Width, 0.0).is_none());
        assert!(with_value(&circle(1.0), Field::Area, 5.0).is_none());
        assert!(with_value(&circle(1.0), Field::X1, f64::NAN).is_none());
    }

    #[test]
    fn parse_accepts_units_and_rejects_text() {
        let units = Units::default();
        let length = |text| parse(&units, Unit::Length, text);
        assert_eq!(length(" 12.5 "), Some(12.5));
        assert_eq!(length("abc"), None);
        assert_eq!(length(""), None);
        assert_eq!(parse(&units, Unit::Angle, "-45\u{b0}"), Some(-45.0));
        let mm = Units {
            insert: od_core::InsertUnits::from_code(4).unwrap(),
            ..units
        };
        assert_eq!(parse(&mm, Unit::Length, "12.5 mm"), Some(12.5));
        assert_eq!(parse(&mm, Unit::Area, "12.5mm\u{b2}"), Some(12.5));
    }

    #[test]
    fn shared_value_is_none_when_entities_differ() {
        let (a, b, c) = (circle(1.0), circle(1.0), circle(2.0));
        assert_eq!(shared_value(&[&a, &b], Field::Radius), Some(1.0));
        assert_eq!(shared_value(&[&a, &c], Field::Radius), None);
        assert_eq!(shared_value(&[&a, &c], Field::X1), Some(1.0));
    }
}
