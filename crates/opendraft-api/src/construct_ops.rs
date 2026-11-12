//! `entities.construct`: the drafter's exact operations (a regular polygon,
//! move, rotate, scale, mirror, offset, rectangular and polar arrays), so a
//! caller states what it wants done to shapes and the program works out the
//! coordinates. Steps run in order as one undo step, each able to use what an
//! earlier one made by its name.
//!
//! Only the kinds the batch tool can write are touched (line, circle, arc,
//! rectangle, polyline, ellipse, text).

use schemars::JsonSchema;
use serde::Deserialize;

use crate::entity::PointDto;
use crate::WireId;

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConstructArgs {
    pub document_id: String,
    /// The revision you last read; refused if the drawing has moved on.
    pub expected_revision: WireId,
    /// The undo step's name.
    #[serde(default)]
    pub label: Option<String>,
    /// At most 100, run in order, all or nothing.
    pub steps: Vec<Step>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PolygonFit {
    /// `radius` is to the corners.
    Inscribed,
    /// `radius` is to the middle of each side.
    Circumscribed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArcEnd {
    Start,
    End,
}

/// Angles are radians, counter-clockwise. A target is a string: the id of an
/// entity in the drawing (digits only) or the name of a shape made earlier in
/// this request (never all digits).
/// Steps that take `copy_as` move or change the targets themselves unless it
/// is given; with it the originals stay and copies are added, named
/// `{copy_as}_{n}` where n counts the targets from 1.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    /// A regular polygon as a closed polyline, with its bottom side level
    /// (as AutoCAD's POLYGON draws it), turned about the centre by `rotation`.
    Polygon {
        client_ref: String,
        center: PointDto,
        /// 3 to 1024.
        sides: u32,
        radius: f64,
        /// Default inscribed.
        #[serde(default)]
        fit: Option<PolygonFit>,
        #[serde(default)]
        rotation: Option<f64>,
        /// Default the current layer.
        #[serde(default)]
        layer_id: Option<WireId>,
    },
    Move {
        targets: Vec<String>,
        dx: f64,
        dy: f64,
        #[serde(default)]
        copy_as: Option<String>,
    },
    Rotate {
        targets: Vec<String>,
        center: PointDto,
        angle: f64,
        #[serde(default)]
        copy_as: Option<String>,
    },
    Scale {
        targets: Vec<String>,
        base: PointDto,
        /// Above 0.
        factor: f64,
        #[serde(default)]
        copy_as: Option<String>,
    },
    /// Reflect across the line through `axis_start` and `axis_end`.
    Mirror {
        targets: Vec<String>,
        axis_start: PointDto,
        axis_end: PointDto,
        #[serde(default)]
        copy_as: Option<String>,
    },
    /// The targets stay as the first item; `columns` along x every
    /// `column_spacing` and `rows` along y every `row_spacing` (negative goes
    /// left or down). Copies are named `{ref_prefix}_{k}` (k from 1, row by
    /// row, the second item first), or `{ref_prefix}_{k}_{n}` for several
    /// targets.
    RectArray {
        targets: Vec<String>,
        columns: u32,
        rows: u32,
        column_spacing: f64,
        row_spacing: f64,
        ref_prefix: String,
    },
    /// The targets stay as the first of `count` items about `center` over
    /// `fill_angle` (default a full turn). Items turn as they go round unless
    /// `rotate_items` is false. Named as `RectArray` names them.
    PolarArray {
        targets: Vec<String>,
        center: PointDto,
        /// Including the original.
        count: u32,
        #[serde(default)]
        fill_angle: Option<f64>,
        /// Default true.
        #[serde(default)]
        rotate_items: Option<bool>,
        ref_prefix: String,
    },
    /// A straight line between two points.
    Line {
        client_ref: String,
        start: PointDto,
        end: PointDto,
        /// Default the current layer.
        #[serde(default)]
        layer_id: Option<WireId>,
    },
    /// An arc that leaves the `at` end of a line or arc without a corner (a
    /// pipe bend, a smooth turn), of `radius`, turning through `sweep`:
    /// positive turns left of the way the shape is going, negative right.
    /// It starts exactly at that end.
    TangentArc {
        target: String,
        at: ArcEnd,
        radius: f64,
        sweep: f64,
        client_ref: String,
        /// Default the layer of `target`.
        #[serde(default)]
        layer_id: Option<WireId>,
    },
    /// A copy of `target` `distance` away, on the side `toward` is on.
    Offset {
        target: String,
        distance: f64,
        toward: PointDto,
        client_ref: String,
        /// Default the layer of `target`.
        #[serde(default)]
        layer_id: Option<WireId>,
    },
}

/// Most steps in one request.
pub const STEPS_MAX: usize = 100;
