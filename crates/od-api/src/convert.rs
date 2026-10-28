//! From write DTOs to core types, with every check that must pass before
//! anything touches the drawing. Patches merge into concrete values and then
//! build through the same functions as new entities, so there is one set of
//! rules. The document is consulted only for what must exist (layers, text
//! styles, linetypes).

use od_core::mtext::MTextAttachment;
use od_core::{
    Attr, Attributes, Color, Document, Ellipse, Entity, LayerId, Lineweight, MText, Point,
    TextStyleId,
};

use crate::entity::{attachment_name, ColorDto, LinetypeDto, LineweightDto, PointDto};
use crate::limits::{POINTS_PER_ENTITY_MAX, STRING_BYTES_MAX};
use crate::write::{AttributesInput, GeometryPatch, NewGeometry};
use crate::{ApiError, ErrorCode};

fn bad(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::InvalidGeometry, message)
}

fn point(what: &str, p: &PointDto) -> Result<Point, ApiError> {
    if p.x.is_finite() && p.y.is_finite() {
        Ok(Point::new(p.x, p.y))
    } else {
        Err(bad(format!("{what} must be finite")))
    }
}

fn finite(what: &str, v: f64) -> Result<f64, ApiError> {
    v.is_finite()
        .then_some(v)
        .ok_or_else(|| bad(format!("{what} must be finite")))
}

fn positive(what: &str, v: f64) -> Result<f64, ApiError> {
    finite(what, v).and_then(|v| {
        (v > 0.0)
            .then_some(v)
            .ok_or_else(|| bad(format!("{what} must be above 0")))
    })
}

/// Whether the kind can be written through the API; others are read-only.
pub fn is_writable(entity: &Entity) -> bool {
    match entity {
        Entity::Line { .. }
        | Entity::Circle { .. }
        | Entity::Arc { .. }
        | Entity::Rectangle { .. }
        | Entity::Polyline { .. }
        | Entity::Ellipse(_)
        | Entity::MText(_) => true,
        Entity::Spline(_)
        | Entity::Point { .. }
        | Entity::Xline { .. }
        | Entity::Ray { .. }
        | Entity::Dimension(_)
        | Entity::MLeader(_)
        | Entity::Leader(_)
        | Entity::Insert(_)
        | Entity::Hatch(_) => false,
    }
}

pub fn unsupported(entity: &Entity) -> ApiError {
    ApiError::new(
        ErrorCode::UnsupportedEntityWrite,
        format!(
            "{}s cannot be written through the API yet.",
            entity.kind().display_name()
        ),
    )
    .detail("kind", entity.kind().wire_name())
}

fn circle(center: Point, radius: f64) -> Result<Entity, ApiError> {
    Ok(Entity::Circle {
        center,
        radius: positive("the radius", radius)?,
    })
}

fn arc(center: Point, radius: f64, start: f64, end: f64) -> Result<Entity, ApiError> {
    Ok(Entity::Arc {
        center,
        radius: positive("the radius", radius)?,
        start: finite("the start angle", start)?,
        end: finite("the end angle", end)?,
    })
}

fn rectangle(min: Point, max: Point) -> Result<Entity, ApiError> {
    if min.x > max.x || min.y > max.y {
        return Err(bad("a rectangle's min must not be above its max"));
    }
    Ok(Entity::Rectangle { min, max })
}

fn polyline(
    points: &[PointDto],
    closed: bool,
    bulges: &[f64],
    widths: &[[f64; 2]],
) -> Result<Entity, ApiError> {
    if points.len() < 2 {
        return Err(bad("a polyline needs at least two points"));
    }
    if points.len() > POINTS_PER_ENTITY_MAX {
        return Err(ApiError::new(
            ErrorCode::PayloadTooLarge,
            format!("A polyline can have at most {POINTS_PER_ENTITY_MAX} points."),
        )
        .detail("limit", POINTS_PER_ENTITY_MAX));
    }
    if !bulges.is_empty() && bulges.len() != points.len() {
        return Err(bad("bulges must be empty or have one value per point"));
    }
    if !widths.is_empty() && widths.len() != points.len() {
        return Err(bad("widths must be empty or have one pair per point"));
    }
    let points = points
        .iter()
        .map(|p| point("a polyline point", p))
        .collect::<Result<Vec<_>, _>>()?;
    for b in bulges {
        finite("a bulge", *b)?;
    }
    let mut pairs = Vec::with_capacity(widths.len());
    for [a, b] in widths {
        if !(finite("a width", *a)? >= 0.0 && finite("a width", *b)? >= 0.0) {
            return Err(bad("widths must not be negative"));
        }
        pairs.push((*a, *b));
    }
    Ok(Entity::Polyline {
        points,
        closed,
        bulges: bulges.to_vec(),
        widths: pairs,
    })
}

fn ellipse(
    center: Point,
    major: Point,
    ratio: f64,
    start: f64,
    end: f64,
) -> Result<Entity, ApiError> {
    Ellipse::new(center, major, ratio, start, end)
        .map(Entity::Ellipse)
        .map_err(|e| bad(format!("not a valid ellipse: {e:?}")))
}

fn attachment(name: &str) -> Result<MTextAttachment, ApiError> {
    (1..=9)
        .filter_map(MTextAttachment::from_code)
        .find(|a| attachment_name(*a) == name)
        .ok_or_else(|| bad(format!("unknown attachment {name:?}, e.g. \"top_left\"")))
}

fn text_style(doc: &Document, id: Option<crate::WireId>) -> Result<TextStyleId, ApiError> {
    match id {
        None => Ok(doc.current_text_style()),
        Some(id) => {
            let style = TextStyleId(id.0);
            doc.text_style(style).map(|_| style).ok_or_else(|| {
                ApiError::new(ErrorCode::InvalidReference, "No such text style.")
                    .detail("style_id", id)
            })
        }
    }
}

fn check_text(m: &MText) -> Result<(), ApiError> {
    if m.contents.len() > STRING_BYTES_MAX {
        return Err(ApiError::new(
            ErrorCode::PayloadTooLarge,
            format!("Text can be at most {STRING_BYTES_MAX} bytes."),
        )
        .detail("limit", STRING_BYTES_MAX));
    }
    m.validate().map_err(|e| bad(e.to_string()))
}

/// The entity a [`NewGeometry`] describes.
pub fn entity_from_new(doc: &Document, geometry: &NewGeometry) -> Result<Entity, ApiError> {
    match geometry {
        NewGeometry::Line { start, end } => Ok(Entity::Line {
            start: point("the start", start)?,
            end: point("the end", end)?,
        }),
        NewGeometry::Circle { center, radius } => circle(point("the centre", center)?, *radius),
        NewGeometry::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => arc(
            point("the centre", center)?,
            *radius,
            *start_angle,
            *end_angle,
        ),
        NewGeometry::Rectangle { min, max } => rectangle(point("min", min)?, point("max", max)?),
        NewGeometry::Polyline {
            points,
            closed,
            bulges,
            widths,
        } => polyline(points, *closed, bulges, widths),
        NewGeometry::Ellipse {
            center,
            major_axis,
            radius_ratio,
            start_param,
            end_param,
        } => ellipse(
            point("the centre", center)?,
            point("the major axis", major_axis)?,
            *radius_ratio,
            start_param.unwrap_or(0.0),
            end_param.unwrap_or(std::f64::consts::TAU),
        ),
        NewGeometry::Mtext {
            insertion,
            contents,
            nominal_height,
            direction,
            reference_width,
            attachment: attach,
            style_id,
        } => {
            let mut m = MText::new(
                point("the insertion point", insertion)?,
                *nominal_height,
                contents.clone(),
                text_style(doc, *style_id)?,
            );
            if let Some(d) = direction {
                m.direction = point("the direction", d)?;
            }
            if let Some(w) = reference_width {
                m.reference_width = *w;
            }
            if let Some(name) = attach {
                m.attachment = attachment(name)?;
            }
            check_text(&m)?;
            Ok(Entity::MText(Box::new(m)))
        }
    }
}

fn mismatch(old: &Entity, patch: &str) -> ApiError {
    bad(format!(
        "a {patch} patch does not fit a {}",
        old.kind().display_name()
    ))
}

/// `old` with the named fields of `patch` replaced; the kind stays.
pub fn patch_entity(
    doc: &Document,
    old: &Entity,
    patch: &GeometryPatch,
) -> Result<Entity, ApiError> {
    let pt = |what, new: &Option<PointDto>, was: Point| match new {
        Some(p) => point(what, p),
        None => Ok(was),
    };
    match (old, patch) {
        (Entity::Line { start, end }, GeometryPatch::Line { start: s, end: e }) => {
            Ok(Entity::Line {
                start: pt("the start", s, *start)?,
                end: pt("the end", e, *end)?,
            })
        }
        (
            Entity::Circle { center, radius },
            GeometryPatch::Circle {
                center: c,
                radius: r,
            },
        ) => circle(pt("the centre", c, *center)?, r.unwrap_or(*radius)),
        (
            Entity::Arc {
                center,
                radius,
                start,
                end,
            },
            GeometryPatch::Arc {
                center: c,
                radius: r,
                start_angle,
                end_angle,
            },
        ) => arc(
            pt("the centre", c, *center)?,
            r.unwrap_or(*radius),
            start_angle.unwrap_or(*start),
            end_angle.unwrap_or(*end),
        ),
        (Entity::Rectangle { min, max }, GeometryPatch::Rectangle { min: a, max: b }) => {
            rectangle(pt("min", a, *min)?, pt("max", b, *max)?)
        }
        (
            Entity::Polyline {
                points,
                closed,
                bulges,
                widths,
            },
            GeometryPatch::Polyline {
                points: p,
                closed: c,
                bulges: b,
                widths: w,
            },
        ) => {
            let current: Vec<PointDto> = points.iter().map(|p| (*p).into()).collect();
            let current_widths: Vec<[f64; 2]> = widths.iter().map(|(a, b)| [*a, *b]).collect();
            polyline(
                p.as_deref().unwrap_or(&current),
                c.unwrap_or(*closed),
                b.as_deref().unwrap_or(bulges),
                w.as_deref().unwrap_or(&current_widths),
            )
        }
        (
            Entity::Ellipse(e),
            GeometryPatch::Ellipse {
                center,
                major_axis,
                radius_ratio,
                start_param,
                end_param,
            },
        ) => ellipse(
            pt("the centre", center, e.center)?,
            pt("the major axis", major_axis, e.major_axis)?,
            radius_ratio.unwrap_or(e.radius_ratio),
            start_param.unwrap_or(e.start),
            end_param.unwrap_or(e.end),
        ),
        (
            Entity::MText(old_text),
            GeometryPatch::Mtext {
                insertion,
                contents,
                nominal_height,
                direction,
                reference_width,
                attachment: attach,
                style_id,
            },
        ) => {
            let mut m = (**old_text).clone();
            m.insertion = pt("the insertion point", insertion, m.insertion)?;
            m.direction = pt("the direction", direction, m.direction)?;
            if let Some(c) = contents {
                m.contents = c.clone();
            }
            m.nominal_height = nominal_height.unwrap_or(m.nominal_height);
            m.reference_width = reference_width.unwrap_or(m.reference_width);
            if let Some(name) = attach {
                m.attachment = attachment(name)?;
            }
            if style_id.is_some() {
                m.style = text_style(doc, *style_id)?;
            }
            check_text(&m)?;
            Ok(Entity::MText(Box::new(m)))
        }
        (old, _) if !is_writable(old) => Err(unsupported(old)),
        (old, GeometryPatch::Line { .. }) => Err(mismatch(old, "line")),
        (old, GeometryPatch::Circle { .. }) => Err(mismatch(old, "circle")),
        (old, GeometryPatch::Arc { .. }) => Err(mismatch(old, "arc")),
        (old, GeometryPatch::Rectangle { .. }) => Err(mismatch(old, "rectangle")),
        (old, GeometryPatch::Polyline { .. }) => Err(mismatch(old, "polyline")),
        (old, GeometryPatch::Ellipse { .. }) => Err(mismatch(old, "ellipse")),
        (old, GeometryPatch::Mtext { .. }) => Err(mismatch(old, "mtext")),
    }
}

/// `base` with the fields set in `input` replaced, after checking that what
/// they name exists.
pub fn attributes_from(
    doc: &Document,
    mut base: Attributes,
    input: &AttributesInput,
) -> Result<Attributes, ApiError> {
    if let Some(id) = input.layer_id {
        let layer = LayerId(id.0);
        doc.layer(layer).ok_or_else(|| {
            ApiError::new(ErrorCode::InvalidReference, "No such layer.").detail("layer_id", id)
        })?;
        base.layer = layer;
    }
    match &input.color {
        None => {}
        Some(ColorDto::ByLayer) => base.color = Attr::ByLayer,
        Some(ColorDto::Foreground) => base.color = Attr::Set(Color::Foreground),
        Some(ColorDto::Rgb { r, g, b }) => base.color = Attr::Set(Color::Rgb(*r, *g, *b)),
    }
    match &input.linetype {
        None => {}
        Some(LinetypeDto::ByLayer) => base.linetype = Attr::ByLayer,
        Some(LinetypeDto::Named { name }) => {
            let record = doc.linetypes().named(name).ok_or_else(|| {
                ApiError::new(ErrorCode::InvalidReference, "No such linetype.")
                    .detail("linetype", name)
            })?;
            base.linetype = Attr::Set(record.id);
        }
    }
    match &input.lineweight {
        None => {}
        Some(LineweightDto::ByLayer) => base.lineweight = Attr::ByLayer,
        Some(LineweightDto::Set { hundredths_mm }) => {
            let weight = Lineweight(*hundredths_mm);
            if !Lineweight::STANDARD.contains(&weight) {
                return Err(bad(format!(
                    "{hundredths_mm} is not a standard lineweight (hundredths of a mm)"
                )));
            }
            base.lineweight = Attr::Set(weight);
        }
    }
    if let Some(scale) = input.linetype_scale {
        base.linetype_scale = positive("the linetype scale", scale)?;
    }
    Ok(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write::NewGeometry as G;

    fn p(x: f64, y: f64) -> PointDto {
        PointDto { x, y }
    }

    fn code(r: Result<Entity, ApiError>) -> ErrorCode {
        r.unwrap_err().code
    }

    #[test]
    fn nonsense_geometry_is_refused_not_stored() {
        let doc = Document::default();
        let nan = f64::NAN;
        let cases = [
            G::Line {
                start: p(nan, 0.0),
                end: p(1.0, 1.0),
            },
            G::Circle {
                center: p(0.0, 0.0),
                radius: 0.0,
            },
            G::Circle {
                center: p(0.0, 0.0),
                radius: -1.0,
            },
            G::Circle {
                center: p(0.0, 0.0),
                radius: f64::INFINITY,
            },
            G::Arc {
                center: p(0.0, 0.0),
                radius: 1.0,
                start_angle: nan,
                end_angle: 1.0,
            },
            G::Rectangle {
                min: p(2.0, 0.0),
                max: p(1.0, 1.0),
            },
            G::Polyline {
                points: vec![p(0.0, 0.0)],
                closed: false,
                bulges: vec![],
                widths: vec![],
            },
            G::Polyline {
                points: vec![p(0.0, 0.0), p(1.0, 0.0)],
                closed: false,
                bulges: vec![0.1],
                widths: vec![],
            },
            G::Polyline {
                points: vec![p(0.0, 0.0), p(1.0, 0.0)],
                closed: false,
                bulges: vec![],
                widths: vec![[-1.0, 0.0], [0.0, 0.0]],
            },
            G::Ellipse {
                center: p(0.0, 0.0),
                major_axis: p(0.0, 0.0),
                radius_ratio: 0.5,
                start_param: None,
                end_param: None,
            },
            G::Ellipse {
                center: p(0.0, 0.0),
                major_axis: p(2.0, 0.0),
                radius_ratio: 1.5,
                start_param: None,
                end_param: None,
            },
        ];
        for case in cases {
            assert_eq!(
                code(entity_from_new(&doc, &case)),
                ErrorCode::InvalidGeometry,
                "{case:?}"
            );
        }
    }

    #[test]
    fn good_geometry_builds_the_core_entity() {
        let doc = Document::default();
        let line = entity_from_new(
            &doc,
            &G::Line {
                start: p(0.0, 0.0),
                end: p(5.0, 0.0),
            },
        )
        .unwrap();
        assert!(matches!(line, Entity::Line { .. }));
        let full = entity_from_new(
            &doc,
            &G::Ellipse {
                center: p(0.0, 0.0),
                major_axis: p(2.0, 0.0),
                radius_ratio: 0.5,
                start_param: None,
                end_param: None,
            },
        )
        .unwrap();
        let Entity::Ellipse(e) = full else {
            panic!("ellipse")
        };
        assert!((e.end - e.start - std::f64::consts::TAU).abs() < 1e-9);
    }

    #[test]
    fn text_needs_a_real_style_and_a_sane_height() {
        let doc = Document::default();
        let text = |height, style| G::Mtext {
            insertion: p(0.0, 0.0),
            contents: "Hello".to_owned(),
            nominal_height: height,
            direction: None,
            reference_width: None,
            attachment: None,
            style_id: style,
        };
        assert!(entity_from_new(&doc, &text(2.5, None)).is_ok());
        assert_eq!(
            code(entity_from_new(&doc, &text(0.0, None))),
            ErrorCode::InvalidGeometry
        );
        assert_eq!(
            code(entity_from_new(&doc, &text(2.5, Some(crate::WireId(99))))),
            ErrorCode::InvalidReference
        );
        let long = G::Mtext {
            insertion: p(0.0, 0.0),
            contents: "x".repeat(STRING_BYTES_MAX + 1),
            nominal_height: 2.5,
            direction: None,
            reference_width: None,
            attachment: None,
            style_id: None,
        };
        assert_eq!(
            code(entity_from_new(&doc, &long)),
            ErrorCode::PayloadTooLarge
        );
    }

    #[test]
    fn a_patch_changes_only_the_named_fields_and_keeps_the_kind() {
        let doc = Document::default();
        let old = Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(5.0, 0.0),
        };
        let patch = GeometryPatch::Line {
            start: None,
            end: Some(p(9.0, 1.0)),
        };
        let new = patch_entity(&doc, &old, &patch).unwrap();
        assert_eq!(
            new,
            Entity::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(9.0, 1.0)
            }
        );
        let wrong = GeometryPatch::Circle {
            center: None,
            radius: Some(1.0),
        };
        assert_eq!(
            code(patch_entity(&doc, &old, &wrong)),
            ErrorCode::InvalidGeometry
        );
        let ruin = GeometryPatch::Line {
            start: Some(p(f64::NAN, 0.0)),
            end: None,
        };
        assert_eq!(
            code(patch_entity(&doc, &old, &ruin)),
            ErrorCode::InvalidGeometry
        );
    }

    #[test]
    fn a_text_patch_keeps_what_it_does_not_name() {
        let doc = Document::default();
        let mut text = MText::new(Point::new(1.0, 1.0), 2.5, "Old", doc.current_text_style());
        text.reference_width = 40.0;
        text.attachment = MTextAttachment::MiddleCenter;
        let patch = GeometryPatch::Mtext {
            insertion: None,
            contents: Some("New".to_owned()),
            nominal_height: None,
            direction: None,
            reference_width: None,
            attachment: None,
            style_id: None,
        };
        let Entity::MText(new) =
            patch_entity(&doc, &Entity::MText(Box::new(text)), &patch).unwrap()
        else {
            panic!("text")
        };
        assert_eq!((new.contents.as_str(), new.reference_width), ("New", 40.0));
        assert_eq!(new.attachment, MTextAttachment::MiddleCenter);
    }

    #[test]
    fn attributes_must_name_things_that_exist() {
        let doc = Document::default();
        let base = Attributes::on_layer(doc.current_layer());
        let set = |input: AttributesInput| attributes_from(&doc, base.clone(), &input);
        let layer = set(AttributesInput {
            layer_id: Some(crate::WireId(42)),
            ..Default::default()
        });
        assert_eq!(layer.unwrap_err().code, ErrorCode::InvalidReference);
        let ltype = set(AttributesInput {
            linetype: Some(LinetypeDto::Named {
                name: "NOPE".into(),
            }),
            ..Default::default()
        });
        assert_eq!(ltype.unwrap_err().code, ErrorCode::InvalidReference);
        let weight = set(AttributesInput {
            lineweight: Some(LineweightDto::Set { hundredths_mm: 26 }),
            ..Default::default()
        });
        assert_eq!(weight.unwrap_err().code, ErrorCode::InvalidGeometry);
        let ok = set(AttributesInput {
            color: Some(ColorDto::Rgb { r: 1, g: 2, b: 3 }),
            linetype: Some(LinetypeDto::Named {
                name: "DASHED".into(),
            }),
            lineweight: Some(LineweightDto::Set { hundredths_mm: 50 }),
            linetype_scale: Some(2.0),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(ok.color, Attr::Set(Color::Rgb(1, 2, 3)));
        assert_eq!(ok.linetype_scale, 2.0);
        let scale = set(AttributesInput {
            linetype_scale: Some(0.0),
            ..Default::default()
        });
        assert!(scale.is_err());
    }
}
