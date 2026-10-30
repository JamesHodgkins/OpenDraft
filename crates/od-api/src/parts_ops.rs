//! `parts.draw`: draw a standard part from its dimensions, exactly. The caller
//! gives the numbers a standard's table or a drawing gives; the shapes come
//! from `od_core::parts`, so no coordinate is worked out by the caller. The app
//! adds the shapes through the same path as `entities.apply_batch`, so the
//! revision check, the one undo step and the attribution are that tool's.

use od_core::parts::{PartDrawing, PartError, Role, Shape, WeldNeckFlange};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::entity::PointDto;
use crate::WireId;

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PartArgs {
    pub document_id: String,
    /// The revision you last read; refused if the drawing has moved on.
    pub expected_revision: WireId,
    /// Where the part's origin goes. Default 0, 0.
    #[serde(default)]
    pub origin: Option<PointDto>,
    /// The undo step's name. Default names the part.
    #[serde(default)]
    pub label: Option<String>,
    /// Hatch the section (a section is hatched). Default true.
    #[serde(default = "yes")]
    pub hatch: bool,
    /// Layers to put the part on, by layer id (from the layers list); what is
    /// left out goes on the current layer.
    #[serde(default)]
    pub layers: Option<PartLayers>,
    pub part: Part,
}

fn yes() -> bool {
    true
}

/// Where each kind of line of a part goes.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PartLayers {
    /// The part's outline and the circles of its face view.
    #[serde(default)]
    pub outline: Option<WireId>,
    /// Centre lines and the bolt circle.
    #[serde(default)]
    pub centre: Option<WireId>,
    /// The section hatching. Default: the outline's layer.
    #[serde(default)]
    pub hatch: Option<WireId>,
}

/// A part and its dimensions, all in the drawing's units.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Part {
    /// A section through the axis (axis horizontal, raised face at the left,
    /// weld end at the right) and the face view to its left, from the same
    /// numbers. The origin is where the raised face meets the axis. The weld
    /// bevel is not drawn. An even bolt count straddles the centrelines.
    WeldNeckFlange {
        outside_diameter: f64,
        /// From the flange face (under the raised face) to the back of the
        /// flange.
        thickness: f64,
        /// Ignored when `raised_face_height` is 0.
        #[serde(default)]
        raised_face_diameter: f64,
        /// 0 for a flat face.
        #[serde(default)]
        raised_face_height: f64,
        /// Diameter of the hub where it meets the back of the flange.
        hub_diameter_base: f64,
        /// Diameter of the hub at the weld end.
        hub_diameter_end: f64,
        /// From the raised face to the weld end.
        overall_length: f64,
        bore: f64,
        bolt_circle_diameter: f64,
        bolt_count: u32,
        bolt_hole_diameter: f64,
    },
}

impl Part {
    /// What to call the undo step.
    pub fn name(&self) -> &'static str {
        match self {
            Part::WeldNeckFlange { .. } => "Weld neck flange",
        }
    }

    /// The shapes that draw it, about its own origin, or why they cannot.
    pub fn shapes(&self) -> Result<Vec<Shape>, PartError> {
        self.drawing().map(|d| d.shapes)
    }

    /// The shapes with what each is for, and what a section hatch fills.
    pub fn drawing(&self) -> Result<PartDrawing, PartError> {
        match self {
            Part::WeldNeckFlange {
                outside_diameter,
                thickness,
                raised_face_diameter,
                raised_face_height,
                hub_diameter_base,
                hub_diameter_end,
                overall_length,
                bore,
                bolt_circle_diameter,
                bolt_count,
                bolt_hole_diameter,
            } => WeldNeckFlange {
                outside_diameter: *outside_diameter,
                thickness: *thickness,
                raised_face_diameter: *raised_face_diameter,
                raised_face_height: *raised_face_height,
                hub_diameter_base: *hub_diameter_base,
                hub_diameter_end: *hub_diameter_end,
                overall_length: *overall_length,
                bore: *bore,
                bolt_circle_diameter: *bolt_circle_diameter,
                bolt_count: *bolt_count,
                bolt_hole_diameter: *bolt_hole_diameter,
            }
            .drawing(),
        }
    }
}

/// The hatch's ANSI31 scale for lines `spacing` apart: at scale 1 they are
/// 0.125 drawing units apart.
fn hatch_scale(spacing: f64) -> f64 {
    ((spacing / 0.125) * 1000.0).round() / 1000.0
}

/// `entities.apply_batch` arguments that add the part at the origin the
/// caller asked for, each shape on the layer for what it is, and the section
/// hatch last, over the outlines it fills.
pub fn batch(args: &PartArgs, drawing: &PartDrawing) -> Value {
    let (dx, dy) = args.origin.map_or((0.0, 0.0), |o| (o.x, o.y));
    let layers = args.layers.clone().unwrap_or_default();
    let attributes = |layer: Option<WireId>| layer.map(|id| json!({"layer_id": id}));
    let point = |p: &od_core::Point| json!({"x": p.x, "y": p.y});
    let mut operations: Vec<Value> = drawing
        .shapes
        .iter()
        .zip(&drawing.roles)
        .enumerate()
        .map(|(n, (shape, role))| {
            let geometry = match shape.moved(dx, dy) {
                Shape::Line(a, b) => json!({"type": "line", "start": point(&a), "end": point(&b)}),
                Shape::Circle { center, radius } => {
                    json!({"type": "circle", "center": point(&center), "radius": radius})
                }
                Shape::Outline(points) => json!({
                    "type": "polyline",
                    "points": points.iter().map(point).collect::<Vec<_>>(),
                    "closed": true,
                }),
            };
            let layer = match role {
                Role::Outline => layers.outline,
                Role::Centre => layers.centre,
            };
            let mut entity = json!({"geometry": geometry});
            if let Some(a) = attributes(layer) {
                entity["attributes"] = a;
            }
            json!({"op": "add", "client_ref": format!("p{n}"), "entity": entity})
        })
        .collect();
    if args.hatch && !drawing.hatched.is_empty() {
        let boundary: Vec<Value> = drawing
            .hatched
            .iter()
            .map(|n| json!({"client_ref": format!("p{n}")}))
            .collect();
        let mut entity = json!({"geometry": {
            "type": "hatch",
            "boundary": boundary,
            "pattern": {
                "type": "named",
                "name": "ANSI31",
                "scale": hatch_scale(drawing.hatch_spacing),
                "angle": 0.0,
            },
            "associative": true,
        }});
        if let Some(a) = attributes(layers.hatch.or(layers.outline)) {
            entity["attributes"] = a;
        }
        operations.push(json!({"op": "add", "client_ref": "hatch", "entity": entity}));
    }
    json!({
        "document_id": args.document_id,
        "expected_revision": args.expected_revision,
        "label": args.label.clone().unwrap_or_else(|| args.part.name().to_owned()),
        "operations": operations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(origin: Option<PointDto>) -> PartArgs {
        serde_json::from_value(json!({
            "document_id": "doc_1_1",
            "expected_revision": "4",
            "origin": origin.map(|o| json!({"x": o.x, "y": o.y})),
            "part": {
                "kind": "weld_neck_flange",
                "outside_diameter": 100, "thickness": 14,
                "raised_face_diameter": 60, "raised_face_height": 2,
                "hub_diameter_base": 50, "hub_diameter_end": 38,
                "overall_length": 60, "bore": 30,
                "bolt_circle_diameter": 75, "bolt_count": 4, "bolt_hole_diameter": 10
            }
        }))
        .expect("arguments")
    }

    #[test]
    fn the_arguments_read_as_a_tagged_part() {
        let a = args(None);
        assert_eq!(a.part.name(), "Weld neck flange");
        assert_eq!(a.expected_revision, WireId(4));
    }

    #[test]
    fn an_unknown_field_or_kind_is_refused() {
        let extra = json!({"document_id": "d", "expected_revision": "1", "part": {"kind": "weld_neck_flange", "colour": "red"}});
        assert!(serde_json::from_value::<PartArgs>(extra).is_err());
        let other =
            json!({"document_id": "d", "expected_revision": "1", "part": {"kind": "teapot"}});
        assert!(serde_json::from_value::<PartArgs>(other).is_err());
    }

    #[test]
    fn the_batch_adds_each_shape_and_names_the_step() {
        let a = args(None);
        let shapes = a.part.shapes().unwrap();
        let b = batch(&a, &a.part.drawing().unwrap());
        assert_eq!(b["label"], "Weld neck flange");
        assert_eq!(b["expected_revision"], "4");
        let operations = b["operations"].as_array().unwrap();
        assert_eq!(
            operations.len(),
            shapes.len() + 1,
            "each shape, then the hatch"
        );
        assert_eq!(operations[0]["op"], "add");
        assert_eq!(operations[0]["entity"]["geometry"]["type"], "polyline");
        assert_eq!(operations[0]["entity"]["geometry"]["closed"], true);
        let refs: std::collections::HashSet<&str> = operations
            .iter()
            .map(|o| o["client_ref"].as_str().unwrap())
            .collect();
        assert_eq!(refs.len(), operations.len(), "every client_ref is its own");
    }

    #[test]
    fn the_origin_moves_every_shape() {
        let a = args(Some(PointDto {
            x: 1000.0,
            y: 500.0,
        }));
        let b = batch(&a, &a.part.drawing().unwrap());
        let first = &b["operations"][0]["entity"]["geometry"]["points"][0];
        assert_eq!(
            (first["x"].as_f64(), first["y"].as_f64()),
            (Some(1000.0), Some(515.0))
        );
    }

    #[test]
    fn a_label_overrides_the_part_name() {
        let mut a = args(None);
        a.label = Some("Flange A".to_owned());
        let b = batch(&a, &a.part.drawing().unwrap());
        assert_eq!(b["label"], "Flange A");
    }

    #[test]
    fn numbers_that_cannot_make_the_part_come_back_as_an_error() {
        let mut a = args(None);
        let Part::WeldNeckFlange { bore, .. } = &mut a.part;
        *bore = 80.0;
        assert!(a.part.shapes().is_err());
    }
}
