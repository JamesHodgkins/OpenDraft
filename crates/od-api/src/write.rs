//! Write-side DTOs: what a caller may ask to add, change or delete. Every
//! struct rejects unknown fields and every geometry is a closed, tagged
//! union, so there is no free-form property map to smuggle values through.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::{ColorDto, LinetypeDto, LineweightDto, PointDto};
use crate::ops::SpaceFilter;
use crate::WireId;

/// The geometry of a new or replacement entity. Angles are radians
/// counter-clockwise.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum NewGeometry {
    Point {
        at: PointDto,
    },
    /// A line with no ends; `direction` is any vector but zero (it is made a
    /// unit vector).
    Xline {
        origin: PointDto,
        direction: PointDto,
    },
    /// A line from `origin` with no far end.
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
        #[serde(default)]
        closed: bool,
        /// Empty (straight), or one per point.
        #[serde(default)]
        bulges: Vec<f64>,
        /// Empty, or one `[start, end]` width per point.
        #[serde(default)]
        widths: Vec<[f64; 2]>,
    },
    Ellipse {
        center: PointDto,
        /// From the centre to the end of the major axis.
        major_axis: PointDto,
        /// Minor over major, above 0 and at most 1.
        radius_ratio: f64,
        /// Default 0.
        #[serde(default)]
        start_param: Option<f64>,
        /// Default a full turn.
        #[serde(default)]
        end_param: Option<f64>,
    },
    /// Plain multiline text. `contents` may carry inline formatting codes.
    Mtext {
        insertion: PointDto,
        contents: String,
        nominal_height: f64,
        #[serde(default)]
        direction: Option<PointDto>,
        #[serde(default)]
        reference_width: Option<f64>,
        /// e.g. `top_left` (the default), `middle_center`.
        #[serde(default)]
        attachment: Option<String>,
        /// An existing text style; default the current one.
        #[serde(default)]
        style_id: Option<WireId>,
    },
}

/// Changes to named fields of an existing entity of the same kind.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum GeometryPatch {
    Point {
        #[serde(default)]
        at: Option<PointDto>,
    },
    Xline {
        #[serde(default)]
        origin: Option<PointDto>,
        #[serde(default)]
        direction: Option<PointDto>,
    },
    Ray {
        #[serde(default)]
        origin: Option<PointDto>,
        #[serde(default)]
        direction: Option<PointDto>,
    },
    Line {
        #[serde(default)]
        start: Option<PointDto>,
        #[serde(default)]
        end: Option<PointDto>,
    },
    Circle {
        #[serde(default)]
        center: Option<PointDto>,
        #[serde(default)]
        radius: Option<f64>,
    },
    Arc {
        #[serde(default)]
        center: Option<PointDto>,
        #[serde(default)]
        radius: Option<f64>,
        #[serde(default)]
        start_angle: Option<f64>,
        #[serde(default)]
        end_angle: Option<f64>,
    },
    Rectangle {
        #[serde(default)]
        min: Option<PointDto>,
        #[serde(default)]
        max: Option<PointDto>,
    },
    Polyline {
        #[serde(default)]
        points: Option<Vec<PointDto>>,
        #[serde(default)]
        closed: Option<bool>,
        #[serde(default)]
        bulges: Option<Vec<f64>>,
        #[serde(default)]
        widths: Option<Vec<[f64; 2]>>,
    },
    Ellipse {
        #[serde(default)]
        center: Option<PointDto>,
        #[serde(default)]
        major_axis: Option<PointDto>,
        #[serde(default)]
        radius_ratio: Option<f64>,
        #[serde(default)]
        start_param: Option<f64>,
        #[serde(default)]
        end_param: Option<f64>,
    },
    /// Fields not named keep their value, including any inline formatting
    /// and columns the text already has.
    Mtext {
        #[serde(default)]
        insertion: Option<PointDto>,
        #[serde(default)]
        contents: Option<String>,
        #[serde(default)]
        nominal_height: Option<f64>,
        #[serde(default)]
        direction: Option<PointDto>,
        #[serde(default)]
        reference_width: Option<f64>,
        #[serde(default)]
        attachment: Option<String>,
        #[serde(default)]
        style_id: Option<WireId>,
    },
}

/// Display properties to set; fields left out are not changed (on add, they
/// take the defaults: the current layer, everything by layer).
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttributesInput {
    #[serde(default)]
    pub layer_id: Option<WireId>,
    #[serde(default)]
    pub color: Option<ColorDto>,
    #[serde(default)]
    pub linetype: Option<LinetypeDto>,
    #[serde(default)]
    pub lineweight: Option<LineweightDto>,
    /// Above 0.
    #[serde(default)]
    pub linetype_scale: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NewEntity {
    pub geometry: NewGeometry,
    #[serde(default)]
    pub attributes: Option<AttributesInput>,
    /// Default: the space being edited in the app.
    #[serde(default)]
    pub space: Option<SpaceFilter>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityPatch {
    #[serde(default)]
    pub geometry: Option<GeometryPatch>,
    #[serde(default)]
    pub attributes: Option<AttributesInput>,
}

/// An existing entity, or one added earlier in the same batch.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum Target {
    Id { entity_id: WireId },
    Ref { client_ref: String },
}

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// `client_ref` names the new entity for later operations and in the
    /// result.
    Add {
        client_ref: String,
        entity: NewEntity,
    },
    Patch {
        target: Target,
        patch: EntityPatch,
    },
    /// New geometry (the kind may change). Attributes and space stay unless
    /// attributes are given.
    Replace {
        target: Target,
        entity: NewEntity,
    },
    Delete {
        target: Target,
    },
}

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BatchArgs {
    pub document_id: String,
    /// The revision the caller last read; the batch is refused if the
    /// drawing has moved on.
    pub expected_revision: WireId,
    /// The undo step's name.
    #[serde(default)]
    pub label: Option<String>,
    /// All or nothing; at most 1000.
    pub operations: Vec<Operation>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChangesDto {
    pub added: Vec<WireId>,
    pub removed: Vec<WireId>,
    pub changed: Vec<WireId>,
    pub layers: bool,
    pub units: bool,
    pub dim_style: bool,
    pub layouts: bool,
    pub blocks: bool,
    pub linetypes: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BatchResult {
    pub document_id: String,
    pub revision_before: WireId,
    pub revision_after: WireId,
    /// False when the batch made no difference: no undo step, same revision.
    pub changed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_step: Option<String>,
    /// Pass to `history.undo` to take this edit back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_token: Option<WireId>,
    /// Each added entity's id, by its `client_ref`.
    pub ids: BTreeMap<String, WireId>,
    pub changes: ChangesDto,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Result<BatchArgs, serde_json::Error> {
        serde_json::from_str(json)
    }

    #[test]
    fn a_mixed_batch_parses() {
        let args = parse(
            r#"{"document_id":"d","expected_revision":"41","label":"Room","operations":[
              {"op":"add","client_ref":"a","entity":{"geometry":{"type":"line","start":{"x":0,"y":0},"end":{"x":5,"y":0}}}},
              {"op":"patch","target":{"client_ref":"a"},"patch":{"geometry":{"type":"line","end":{"x":9,"y":0}}}},
              {"op":"replace","target":{"entity_id":"77"},"entity":{"geometry":{"type":"circle","center":{"x":0,"y":0},"radius":2}}},
              {"op":"delete","target":{"entity_id":"80"}}]}"#,
        )
        .unwrap();
        assert_eq!(args.operations.len(), 4);
        assert!(matches!(
            &args.operations[1],
            Operation::Patch { target: Target::Ref { client_ref }, .. } if client_ref == "a"
        ));
    }

    #[test]
    fn unknown_fields_and_ambiguous_targets_are_refused() {
        let bad = [
            r#"{"document_id":"d","expected_revision":"1","operations":[],"force":true}"#,
            r#"{"document_id":"d","expected_revision":1,"operations":[]}"#,
            r#"{"document_id":"d","expected_revision":"1","operations":[{"op":"delete","target":{"entity_id":"1","client_ref":"a"}}]}"#,
            r#"{"document_id":"d","expected_revision":"1","operations":[{"op":"delete","target":{}}]}"#,
            r#"{"document_id":"d","expected_revision":"1","operations":[{"op":"add","client_ref":"a","entity":{"geometry":{"type":"line","start":{"x":0,"y":0},"end":{"x":1,"y":1},"id":"5"}}}]}"#,
            r#"{"document_id":"d","expected_revision":"1","operations":[{"op":"add","client_ref":"a","entity":{"geometry":{"type":"spline"}}}]}"#,
        ];
        for json in bad {
            assert!(parse(json).is_err(), "{json}");
        }
    }

    #[test]
    fn the_batch_schema_is_an_object_with_the_operation_union_inline() {
        let generator = schemars::generate::SchemaSettings::draft2020_12()
            .with(|s| s.inline_subschemas = true)
            .into_generator();
        let schema = serde_json::to_value(generator.into_root_schema_for::<BatchArgs>()).unwrap();
        assert_eq!(schema["type"], "object");
        assert!(!schema.to_string().contains("$ref"));
    }
}
