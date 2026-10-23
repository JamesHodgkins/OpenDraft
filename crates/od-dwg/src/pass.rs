//! What the first pass over the objects collects: each supported entity as
//! stored, the tables, and the entities of types not read, all keyed by
//! handle, for the second pass to resolve.

use std::collections::BTreeMap;

use crate::entities::{
    ArcData, CircleData, EllipseData, InsertData, LineData, LwPolylineData, PointData,
    Polyline2dData, Vertex2dData,
};
use crate::handles::DwgHandle;
use crate::spline::SplineData;
use crate::tables::{BlockData, DwgLinetypeDef, LayerData};
use crate::text::{AttribData, DwgTextStyle, MTextData, TextData};

pub(crate) enum Stored {
    Point(PointData),
    Line(LineData),
    Circle(CircleData),
    Arc(ArcData),
    Ellipse(EllipseData),
    Spline(SplineData),
    LwPolyline(LwPolylineData),
    Polyline(Polyline2dData),
    Vertex(Vertex2dData),
    Insert(InsertData),
    Text(TextData),
    Attrib(AttribData),
    Attdef(AttribData),
    MText(MTextData),
}

/// What an entity says about its look, before handles are resolved.
pub(crate) struct Look {
    pub(crate) layer: Option<DwgHandle>,
    pub(crate) color_index: u16,
    pub(crate) true_color: Option<u32>,
    pub(crate) linetype_flags: u8,
    pub(crate) linetype: Option<DwgHandle>,
    pub(crate) linetype_scale: f64,
    pub(crate) lineweight: u8,
    pub(crate) invisible: bool,
}

pub(crate) struct Record {
    pub(crate) look: Look,
    pub(crate) stored: Stored,
}

/// Everything decoded in the first pass.
#[derive(Default)]
pub(crate) struct Pass {
    pub(crate) layers: BTreeMap<DwgHandle, LayerData>,
    pub(crate) linetypes: BTreeMap<DwgHandle, DwgLinetypeDef>,
    pub(crate) blocks: BTreeMap<DwgHandle, BlockData>,
    pub(crate) styles: BTreeMap<DwgHandle, DwgTextStyle>,
    pub(crate) records: BTreeMap<DwgHandle, Record>,
    /// Entities of types not supported, by DXF name.
    pub(crate) others: BTreeMap<DwgHandle, String>,
}
