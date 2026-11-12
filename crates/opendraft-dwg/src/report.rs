//! What happened to the content of a drawing on the way in, as counts by
//! kind. It is a typed record, not text: the application words it for the
//! user, in a stable order (the maps are ordered).

use std::collections::BTreeMap;

use crate::version::DwgVersion;

/// Content represented differently but equivalently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Normalization {
    /// A 2D POLYLINE with its VERTEX records became one lightweight polyline.
    OldPolylineJoined,
}

/// Content kept with something approximated or lost.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Degradation {
    /// A constant, non-zero elevation was dropped to flatten the entity.
    ElevationFlattened,
    /// An entity named a linetype that is not in the drawing; "by layer" was used.
    MissingLinetype,
    /// Spline-frame control points of a fitted polyline were left out.
    SplineFrameVertices,
    /// Text "aligned" or "fit" between two points was read as left-aligned
    /// text along the line between them, at its own height and width.
    TextFitApproximated,
}

/// Content that did not come across.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Skipped {
    /// An entity type with no counterpart yet (by DXF name).
    Unsupported(String),
    /// An entity in a tilted plane, or whose points do not share one.
    NonPlanar,
    /// An entity marked invisible.
    Invisible,
    /// An entity in paper space (layouts are not imported yet).
    PaperSpace,
    /// A reference to an anonymous block, such as a dynamic block's
    /// (anonymous blocks are not imported).
    AnonymousBlockReference,
    /// A reference to an external drawing (xref).
    ExternalReference,
    /// Geometry that cannot exist: a non-finite or huge coordinate, a zero radius.
    InvalidGeometry,
    /// A polyline with fewer than two usable vertices.
    DegeneratePolyline,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportReport {
    pub source_version: DwgVersion,
    /// Entities carried over with nothing lost.
    pub exact_entities: u64,
    pub normalized: BTreeMap<Normalization, u64>,
    pub degraded: BTreeMap<Degradation, u64>,
    pub skipped: BTreeMap<Skipped, u64>,
}

impl ImportReport {
    /// An empty report for a file of this version.
    pub fn new(source_version: DwgVersion) -> Self {
        ImportReport {
            source_version,
            exact_entities: 0,
            normalized: BTreeMap::new(),
            degraded: BTreeMap::new(),
            skipped: BTreeMap::new(),
        }
    }

    pub(crate) fn skip(&mut self, why: Skipped) {
        *self.skipped.entry(why).or_insert(0) += 1;
    }

    pub(crate) fn degrade(&mut self, how: Degradation) {
        *self.degraded.entry(how).or_insert(0) += 1;
    }

    pub(crate) fn normalize(&mut self, how: Normalization) {
        *self.normalized.entry(how).or_insert(0) += 1;
    }

    pub fn skipped_total(&self) -> u64 {
        self.skipped.values().sum()
    }
}
