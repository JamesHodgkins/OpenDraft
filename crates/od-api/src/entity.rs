//! Read-side entity DTOs, separate from native `.odx`. Matches have no wildcard,
//! so a new core entity fails the build until its wire form is decided.

use od_core::hatch::HatchFill;
use od_core::mtext::MTextAttachment;
use od_core::{Attr, Attributes, Color, Document, Entity, LayoutId, Object, Point};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use crate::entity_bounds::BoundsDto;
use crate::entity_dimensions;
use crate::entity_leaders;
use crate::limits::{POINTS_PER_ENTITY_MAX, STRING_BYTES_MAX};
use crate::WireId;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PointDto {
    pub x: f64,
    pub y: f64,
}

/// Negative zero is normalised away so output is stable.
pub(crate) fn num(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

impl From<Point> for PointDto {
    fn from(p: Point) -> Self {
        Self {
            x: num(p.x),
            y: num(p.y),
        }
    }
}

/// The geometry of one entity, tagged by `type`. Angles are radians
/// counter-clockwise. Kinds that are only summarised say so in their fields:
/// a client must not write a summary back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GeometryDto {
    Point {
        at: PointDto,
    },
    /// A line with no ends; `direction` is a unit vector.
    Xline {
        origin: PointDto,
        direction: PointDto,
    },
    /// A line with no far end; `direction` is a unit vector.
    Ray {
        origin: PointDto,
        direction: PointDto,
    },
    Line {
        start: PointDto,
        end: PointDto,
    },
    Circle {
        center: PointDto,
        radius: f64,
    },
    Arc {
        center: PointDto,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },
    Rectangle {
        min: PointDto,
        max: PointDto,
    },
    Polyline {
        points: Vec<PointDto>,
        closed: bool,
        /// Empty, or one per point: `bulges[i]` shapes segment i to i+1.
        bulges: Vec<f64>,
        /// Empty, or one `[start, end]` width pair per point.
        widths: Vec<[f64; 2]>,
    },
    Ellipse {
        center: PointDto,
        /// The vector from the centre to the end of the major axis.
        major_axis: PointDto,
        radius_ratio: f64,
        start_param: f64,
        end_param: f64,
    },
    Spline {
        degree: u8,
        closed: bool,
        periodic: bool,
        control_points: Vec<PointDto>,
        knots: Vec<f64>,
        weights: Option<Vec<f64>>,
        has_fit_points: bool,
    },
    Mtext {
        insertion: PointDto,
        direction: PointDto,
        nominal_height: f64,
        reference_width: f64,
        attachment: String,
        style_id: String,
        /// Untrusted drawing text (inline formatting codes included).
        contents: String,
        contents_truncated: bool,
    },
    Text {
        position: PointDto,
        #[serde(skip_serializing_if = "Option::is_none")]
        alignment_mode: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        alignment_end: Option<PointDto>,
        height: f64,
        rotation: f64,
        horizontal: String,
        vertical: String,
        style_id: String,
        /// Untrusted drawing text.
        contents: String,
        contents_truncated: bool,
    },
    Insert {
        block_id: String,
        block_name: String,
        insertion: PointDto,
        scale_x: f64,
        scale_y: f64,
        rotation: f64,
        /// The values of the block's attributes, in definition order.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        attributes: Vec<AttributeValueDto>,
    },
    /// A template text in a block: what an insert asks for and shows.
    AttributeDefinition {
        tag: String,
        prompt: String,
        /// The default as displayed (untrusted drawing text).
        default_value: String,
        constant: bool,
        invisible: bool,
        verify: bool,
        preset: bool,
        position: PointDto,
        height: f64,
        rotation: f64,
    },
    /// A summary: its defining points by name, and the text mode.
    Dimension {
        dimension: String,
        points: Vec<(String, PointDto)>,
        text: String,
    },
    /// Leader lines (arrowhead first) to a landing, and the text.
    Multileader {
        style: String,
        lines: Vec<Vec<PointDto>>,
        landing_direction: PointDto,
        text_insertion: Option<PointDto>,
        /// Untrusted drawing text (inline formatting codes included).
        text: Option<String>,
        text_truncated: bool,
    },
    /// A legacy leader: its vertices, arrowhead first, and the text it annotates.
    Leader {
        vertices: Vec<PointDto>,
        arrow: bool,
        hookline: bool,
        annotation_id: Option<String>,
    },
    /// A summary: counts and modes, not the boundary geometry.
    Hatch {
        fill: String,
        loops: usize,
        associative: bool,
        elevation: f64,
    },
}

/// One attribute of a block reference.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AttributeValueDto {
    pub tag: String,
    /// The value as displayed (untrusted drawing text); a data field shows
    /// its last value.
    pub value: String,
    pub invisible: bool,
    /// Whether the value comes from a data field.
    pub field: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ColorDto {
    ByLayer,
    ByBlock,
    Foreground,
    Rgb { r: u8, g: u8, b: u8 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum LinetypeDto {
    ByLayer,
    ByBlock,
    Named { name: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum LineweightDto {
    ByLayer,
    ByBlock,
    /// Hundredths of a millimetre.
    Set {
        hundredths_mm: u16,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttributesDto {
    pub layer_id: WireId,
    pub color: ColorDto,
    pub linetype: LinetypeDto,
    pub lineweight: LineweightDto,
    pub linetype_scale: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SpaceDto {
    Model,
    Paper { layout_id: WireId },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityDto {
    pub id: WireId,
    pub kind: String,
    pub geometry: GeometryDto,
    pub attributes: AttributesDto,
    pub space: SpaceDto,
    pub bounds: BoundsDto,
}

pub fn color_dto(color: &Attr<Color>) -> ColorDto {
    match color {
        Attr::ByLayer => ColorDto::ByLayer,
        Attr::ByBlock => ColorDto::ByBlock,
        Attr::Set(Color::Foreground) => ColorDto::Foreground,
        Attr::Set(Color::Rgb(r, g, b)) => ColorDto::Rgb {
            r: *r,
            g: *g,
            b: *b,
        },
    }
}

fn attributes_dto(document: &Document, a: &Attributes) -> AttributesDto {
    AttributesDto {
        layer_id: WireId(a.layer.0),
        color: color_dto(&a.color),
        linetype: match &a.linetype {
            Attr::ByLayer => LinetypeDto::ByLayer,
            Attr::ByBlock => LinetypeDto::ByBlock,
            // The wire speaks names; ids are the drawing's own business.
            Attr::Set(id) => LinetypeDto::Named {
                name: document
                    .linetypes()
                    .get(*id)
                    .map_or_else(|| "CONTINUOUS".to_owned(), |r| r.name.clone()),
            },
        },
        lineweight: match a.lineweight {
            Attr::ByLayer => LineweightDto::ByLayer,
            Attr::ByBlock => LineweightDto::ByBlock,
            Attr::Set(w) => LineweightDto::Set { hundredths_mm: w.0 },
        },
        linetype_scale: num(a.linetype_scale),
    }
}

pub(crate) fn attachment_name(a: MTextAttachment) -> &'static str {
    match a {
        MTextAttachment::TopLeft => "top_left",
        MTextAttachment::TopCenter => "top_center",
        MTextAttachment::TopRight => "top_right",
        MTextAttachment::MiddleLeft => "middle_left",
        MTextAttachment::MiddleCenter => "middle_center",
        MTextAttachment::MiddleRight => "middle_right",
        MTextAttachment::BottomLeft => "bottom_left",
        MTextAttachment::BottomCenter => "bottom_center",
        MTextAttachment::BottomRight => "bottom_right",
    }
}

/// `text` cut to the string limit on a character boundary, and whether it
/// was cut.
pub(crate) fn bounded(text: &str) -> (String, bool) {
    if text.len() <= STRING_BYTES_MAX {
        return (text.to_owned(), false);
    }
    let mut end = STRING_BYTES_MAX;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
}

pub fn geometry_dto(entity: &Entity) -> GeometryDto {
    match entity {
        Entity::Point { at } => GeometryDto::Point { at: (*at).into() },
        Entity::Xline { origin, direction } => GeometryDto::Xline {
            origin: (*origin).into(),
            direction: (*direction).into(),
        },
        Entity::Ray { origin, direction } => GeometryDto::Ray {
            origin: (*origin).into(),
            direction: (*direction).into(),
        },
        Entity::Line { start, end } => GeometryDto::Line {
            start: (*start).into(),
            end: (*end).into(),
        },
        Entity::Circle { center, radius } => GeometryDto::Circle {
            center: (*center).into(),
            radius: num(*radius),
        },
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => GeometryDto::Arc {
            center: (*center).into(),
            radius: num(*radius),
            start_angle: num(*start),
            end_angle: num(*end),
        },
        Entity::Rectangle { min, max } => GeometryDto::Rectangle {
            min: (*min).into(),
            max: (*max).into(),
        },
        Entity::Polyline {
            points,
            closed,
            bulges,
            widths,
        } => GeometryDto::Polyline {
            points: points.iter().map(|p| (*p).into()).collect(),
            closed: *closed,
            bulges: bulges.iter().map(|b| num(*b)).collect(),
            widths: widths.iter().map(|(a, b)| [num(*a), num(*b)]).collect(),
        },
        Entity::Ellipse(e) => GeometryDto::Ellipse {
            center: e.center.into(),
            major_axis: e.major_axis.into(),
            radius_ratio: num(e.radius_ratio),
            start_param: num(e.start),
            end_param: num(e.end),
        },
        Entity::Spline(s) => GeometryDto::Spline {
            degree: s.degree,
            closed: s.closed,
            periodic: s.periodic,
            control_points: s.control_points.iter().map(|p| (*p).into()).collect(),
            knots: s.knots.iter().map(|k| num(*k)).collect(),
            weights: s
                .weights
                .as_ref()
                .map(|w| w.iter().map(|v| num(*v)).collect()),
            has_fit_points: s.fit.is_some(),
        },
        Entity::MText(m) => crate::entity_text::mtext(m),
        Entity::Text(t) => crate::entity_text::single_line(t),
        Entity::Insert(i) => GeometryDto::Insert {
            block_id: i.block.id.0.to_string(),
            block_name: bounded(&i.block.name).0,
            insertion: i.insertion.into(),
            scale_x: num(i.scale_x),
            scale_y: num(i.scale_y),
            rotation: num(i.rotation),
            attributes: i
                .attributes
                .iter()
                .take(POINTS_PER_ENTITY_MAX)
                .map(|r| AttributeValueDto {
                    tag: bounded(&r.tag).0,
                    value: bounded(&r.value.display()).0,
                    invisible: r.flags.invisible(),
                    field: r.value.has_fields(),
                })
                .collect(),
        },
        Entity::AttributeDefinition(d) => GeometryDto::AttributeDefinition {
            tag: bounded(&d.tag).0,
            prompt: bounded(&d.prompt).0,
            default_value: bounded(&d.default_value.display()).0,
            constant: d.flags.constant(),
            invisible: d.flags.invisible(),
            verify: d.flags.verify(),
            preset: d.flags.preset(),
            position: d.text.position.into(),
            height: num(d.text.height),
            rotation: num(d.text.rotation),
        },
        Entity::Dimension(d) => entity_dimensions::geometry(d),
        Entity::Leader(l) => entity_leaders::leader(l),
        Entity::MLeader(m) => entity_leaders::multileader(m),
        Entity::Hatch(h) => GeometryDto::Hatch {
            fill: match h.fill {
                HatchFill::Pattern(_) => "pattern",
                HatchFill::Solid => "solid",
                HatchFill::Gradient(_) => "gradient",
            }
            .to_owned(),
            loops: h.loops.len(),
            associative: h.associative,
            elevation: num(h.elevation),
        },
    }
}

pub fn space_dto(layout: Option<LayoutId>) -> SpaceDto {
    match layout {
        None => SpaceDto::Model,
        Some(id) => SpaceDto::Paper {
            layout_id: WireId(id.0),
        },
    }
}

/// The wire form of an object.
pub fn entity_dto(document: &Document, object: &Object) -> EntityDto {
    EntityDto {
        id: WireId(object.id.0),
        kind: object.entity.kind().wire_name().to_owned(),
        geometry: geometry_dto(&object.entity),
        attributes: attributes_dto(document, &object.attrs),
        space: space_dto(object.layout),
        bounds: object.bounds().into(),
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Document, Entity, Point};

    use super::*;

    fn first(entity: Entity) -> EntityDto {
        let mut doc = Document::default();
        let id = doc.add(entity);
        entity_dto(&doc, doc.object(id).unwrap())
    }

    #[test]
    fn a_line_has_the_documented_shape() {
        let dto = first(Entity::Line {
            start: Point::new(-0.0, 0.0),
            end: Point::new(5000.0, 0.0),
        });
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["kind"], "line");
        assert_eq!(json["geometry"]["type"], "line");
        assert_eq!(json["geometry"]["end"]["x"], 5000.0);
        assert_eq!(json["attributes"]["color"]["mode"], "by_layer");
        assert_eq!(json["space"]["kind"], "model");
        assert_eq!(json["bounds"]["max"]["x"], 5000.0);
        assert!(json["id"].is_string(), "ids are strings");
        let text = serde_json::to_string(&dto).unwrap();
        assert!(
            !text.contains("-0.0"),
            "negative zero is normalised: {text}"
        );
    }

    #[test]
    fn a_polyline_keeps_bulges_and_widths() {
        let dto = first(Entity::Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(1.0, 0.0)],
            closed: false,
            bulges: vec![0.5, 0.0],
            widths: vec![(1.0, 2.0), (0.0, 0.0)],
        });
        let GeometryDto::Polyline { bulges, widths, .. } = dto.geometry else {
            panic!("a polyline");
        };
        assert_eq!(bulges, vec![0.5, 0.0]);
        assert_eq!(widths[0], [1.0, 2.0]);
    }

    #[test]
    fn dto_round_trips_through_json() {
        let dto = first(Entity::Circle {
            center: Point::new(1.0, 2.0),
            radius: 3.0,
        });
        let json = serde_json::to_string(&dto).unwrap();
        assert_eq!(serde_json::from_str::<EntityDto>(&json).unwrap(), dto);
    }

    #[test]
    fn long_text_is_cut_on_a_character_boundary() {
        let text = "é".repeat(STRING_BYTES_MAX);
        let (cut, truncated) = bounded(&text);
        assert!(truncated && cut.len() <= STRING_BYTES_MAX);
        assert!(cut.chars().all(|c| c == 'é'));
        assert!(!bounded("short").1);
    }
}
