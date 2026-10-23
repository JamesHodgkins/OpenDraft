//! `view.render`: a look at the drawing as SVG, labelled with the revision
//! it shows.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::BoundsDto;
use crate::ops::{BoundsArg, SpaceFilter};
use crate::WireId;

/// Default and bounds of the sheet width, in SVG units (about pixels).
pub const WIDTH_DEFAULT: f64 = 800.0;
pub const WIDTH_MIN: f64 = 50.0;

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderArgs {
    pub document_id: String,
    /// `model` (default), or a layout's whole sheet as it would plot.
    #[serde(default)]
    pub space: Option<SpaceFilter>,
    /// Draw only these entities, framed to fit them. Model space only.
    #[serde(default)]
    pub entity_ids: Option<Vec<WireId>>,
    /// The window to draw, in drawing units. Default: everything in model
    /// space, or the entities' bounds.
    #[serde(default)]
    pub bounds: Option<BoundsArg>,
    /// Width of the picture in SVG units, 50 to 4096; default 800.
    #[serde(default)]
    pub width: Option<f64>,
    /// Refuse if the drawing is no longer at this revision.
    #[serde(default)]
    pub expected_revision: Option<WireId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderResult {
    /// The revision that was drawn.
    pub revision: WireId,
    pub svg: String,
    pub width: f64,
    pub height: f64,
    /// The part of model space shown; absent for a layout sheet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<BoundsDto>,
    /// Polyline points in the picture.
    pub vertices: usize,
    /// How this picture differs from the app's canvas.
    pub fidelity: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

pub const FIDELITY: &str = "Drawn by the plotting pipeline, not the canvas: black and white \
    ink on white paper, line weights in plot millimetres at the picture's scale, text as outlines, \
    wide polylines as single lines. Entities on hidden or frozen layers are not drawn.";
