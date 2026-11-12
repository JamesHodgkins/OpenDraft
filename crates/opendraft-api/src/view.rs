//! `view.render`: a look at the drawing as SVG or PNG, labelled with the
//! revision it shows.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::BoundsDto;
use crate::ops::{BoundsArg, SpaceFilter};
use crate::WireId;

/// Default and bounds of the sheet width, in SVG units (about pixels).
pub const WIDTH_DEFAULT: f64 = 800.0;
pub const WIDTH_MIN: f64 = 50.0;

/// What the picture is made of. (Plain comments on the variants: their text
/// would be repeated in every tool listing; the tool description says it.)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RenderFormat {
    // Exact vector drawing, as text.
    #[default]
    Svg,
    // Anti-aliased pixels: a picture that can actually be looked at.
    Png,
}

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
    /// Width of the picture, 50 to 4096 (SVG units; pixels for png); default 800.
    #[serde(default)]
    pub width: Option<f64>,
    /// `svg` (default) or `png`.
    #[serde(default)]
    pub format: Option<RenderFormat>,
    /// Refuse if the drawing is no longer at this revision.
    #[serde(default)]
    pub expected_revision: Option<WireId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderResult {
    /// The revision that was drawn.
    pub revision: WireId,
    pub format: RenderFormat,
    /// The drawing as SVG, for `svg`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub svg: Option<String>,
    /// The picture as a base64 PNG, for `png`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png_base64: Option<String>,
    /// SVG units, or pixels for a PNG.
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

pub const FIDELITY_PNG: &str = "Drawn by the plotting pipeline, not the canvas, then rasterised and \
    anti-aliased on white paper, in plot colours (white and the foreground colour print black), no line \
    thinner than one pixel (so lineweights are not to scale), text as outlines, wide polylines as single \
    lines. Entities on hidden or frozen layers are not drawn.";

pub const FIDELITY: &str = "Drawn by the plotting pipeline, not the canvas: on white paper in plot colours \
    (white and the foreground colour print black), line weights in plot millimetres at the picture's scale, \
    text as outlines, wide polylines as single lines. Entities on hidden or frozen layers are not drawn.";
