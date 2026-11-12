//! Hatches: pattern, solid and gradient fills of boundary loops.
//!
//! A hatch follows AutoCAD's `HATCH` (`AcDbHatch`): it is a semantic entity,
//! not a bag of lines. Its boundary loops, island style and fill definition
//! are the authority; the strokes and areas drawn from them are worked out by
//! [`HatchEntity::evaluate`] and thrown away whenever the hatch changes.
//!
//! - **Pattern lines are final.** As in DXF, each [`PatternLine`] already has
//!   the pattern's scale, angle and origin applied (the `.pat` offset is
//!   rotated by the line angle, then scaled and turned with the pattern), so
//!   a stored hatch draws without its pattern file. `angle`, `scale_or_spacing`
//!   and `origin` of [`HatchPattern`] are what the user last asked for, kept
//!   so that Properties can show them and change them
//!   ([`HatchPattern::with_scale`], [`HatchPattern::with_angle`]).
//! - **Evaluation is bounded.** [`HatchLimits`] caps the pattern lines, the
//!   strokes and the crossings, and a hatch past them fails with
//!   [`HatchError::TooDense`] instead of running away.
//! - **Planar.** Hatches here lie in the drawing plane (a normal along +Z);
//!   `elevation` is kept for files.

mod boundary;
mod build;
mod catalog;
mod clip;
mod dash;
mod error;
mod eval;
mod follow;
mod graph;
mod pattern;
mod topology;
mod trace;
mod transform;
mod validate;

pub use build::{loops_and_sources, loops_from_entities, BuildError};
pub use catalog::{builtin, PatternCatalog, PatternDef, PatternDefLine};
pub use error::HatchError;
pub use eval::{EvaluatedHatch, HatchLimits};
pub use follow::{follow, Follow};
pub use pattern::PatternError;
pub use topology::contour_area;
pub use trace::{trace, TraceError, Traced};
pub use validate::{
    MAX_DASHES, MAX_EDGES, MAX_LOOPS, MAX_PATTERN_LINES, MAX_SEEDS, MAX_STOPS, MAX_VERTICES,
};

use crate::{EntityId, Point};

/// How islands inside the outer boundary are filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HatchStyle {
    /// Odd parity: fill, then not inside the first island, fill again inside
    /// an island within it.
    Normal,
    /// Only the outermost band: nothing inside the first island is filled.
    Outer,
    /// Internal boundaries are ignored.
    Ignore,
}

impl HatchStyle {
    /// The DXF code (group 75).
    pub fn code(self) -> i64 {
        match self {
            HatchStyle::Normal => 0,
            HatchStyle::Outer => 1,
            HatchStyle::Ignore => 2,
        }
    }

    pub fn from_code(code: i64) -> Option<Self> {
        match code {
            0 => Some(HatchStyle::Normal),
            1 => Some(HatchStyle::Outer),
            2 => Some(HatchStyle::Ignore),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HatchStyle::Normal => "Normal",
            HatchStyle::Outer => "Outer",
            HatchStyle::Ignore => "Ignore",
        }
    }
}

/// A hatch. See the module notes.
#[derive(Clone, Debug, PartialEq)]
pub struct HatchEntity {
    /// Height of the hatch plane (DXF 30); drawing coordinates ignore it.
    pub elevation: f64,
    /// Boundary loops in stored order.
    pub loops: Vec<HatchLoop>,
    pub style: HatchStyle,
    pub fill: HatchFill,
    /// Whether the hatch follows the objects its loops were made from (see
    /// [`follow`]); the session keeps it in step with them after every edit.
    pub associative: bool,
    /// Points the boundary was found from (DXF 98).
    pub seed_points: Vec<Point>,
    /// DXF 47, kept as read; not a model-space tolerance.
    pub source_pixel_size: Option<f64>,
}

/// One closed boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct HatchLoop {
    pub flags: HatchLoopFlags,
    pub boundary: HatchBoundary,
    /// The objects this loop was made from, when the hatch is associative.
    pub sources: Vec<BoundarySource>,
}

/// The loop type bits of DXF group 92. Bits this program does not act on are
/// kept as they were read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HatchLoopFlags(pub u32);

impl HatchLoopFlags {
    pub const EXTERNAL: u32 = 0x001;
    pub const POLYLINE: u32 = 0x002;
    pub const DERIVED: u32 = 0x004;
    pub const TEXTBOX: u32 = 0x008;
    pub const OUTERMOST: u32 = 0x010;
    pub const NOT_CLOSED: u32 = 0x020;
    pub const SELF_INTERSECTING: u32 = 0x040;
    pub const TEXT_ISLAND: u32 = 0x080;
    pub const DUPLICATE: u32 = 0x100;

    pub fn contains(self, flag: u32) -> bool {
        self.0 & flag != 0
    }

    /// These flags with `flag` set or cleared.
    pub fn with(self, flag: u32, on: bool) -> Self {
        Self(if on { self.0 | flag } else { self.0 & !flag })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum HatchBoundary {
    /// Vertices with optional bulges.
    Polyline(HatchPolyline),
    /// Typed edges, end to end.
    Edges(Vec<HatchEdge>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct HatchPolyline {
    pub vertices: Vec<Point>,
    /// Empty (all straight) or one per vertex, as for a polyline entity.
    pub bulges: Vec<f64>,
    pub closed: bool,
}

/// An edge of an edge-type boundary, in the direction the loop runs.
#[derive(Clone, Debug, PartialEq)]
pub enum HatchEdge {
    Line {
        start: Point,
        end: Point,
    },
    /// Radians. `start == end` (mod 2π) is a whole circle.
    CircularArc {
        center: Point,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        counter_clockwise: bool,
    },
    /// `major_axis` runs from the centre to the end of the major axis; the
    /// angles are ellipse parameters in radians.
    EllipticArc {
        center: Point,
        major_axis: Point,
        minor_major_ratio: f64,
        start_param: f64,
        end_param: f64,
        counter_clockwise: bool,
    },
    Spline(HatchSpline),
}

#[derive(Clone, Debug, PartialEq)]
pub struct HatchSpline {
    pub degree: u8,
    pub periodic: bool,
    pub knots: Vec<f64>,
    pub control_points: Vec<Point>,
    /// Empty (all 1) or one per control point.
    pub weights: Vec<f64>,
    pub fit_points: Vec<Point>,
    pub start_tangent: Option<Point>,
    pub end_tangent: Option<Point>,
}

/// An object a loop was made from.
#[derive(Clone, Debug, PartialEq)]
pub enum BoundarySource {
    Entity(EntityId),
    /// A reference the file layer could not resolve (a DXF handle).
    External {
        scheme: String,
        value: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum HatchFill {
    Pattern(HatchPattern),
    Solid,
    Gradient(HatchGradient),
}

/// DXF group 76.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternType {
    UserDefined,
    Predefined,
    Custom,
}

impl PatternType {
    pub fn code(self) -> i64 {
        match self {
            PatternType::UserDefined => 0,
            PatternType::Predefined => 1,
            PatternType::Custom => 2,
        }
    }

    pub fn from_code(code: i64) -> Option<Self> {
        match code {
            0 => Some(PatternType::UserDefined),
            1 => Some(PatternType::Predefined),
            2 => Some(PatternType::Custom),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HatchPattern {
    pub pattern_type: PatternType,
    pub name: String,
    /// What the user asked for (radians); [`lines`](Self::lines) already
    /// have it applied.
    pub angle: f64,
    /// The scale of a named pattern, or the spacing of a user-defined one.
    pub scale_or_spacing: f64,
    /// User-defined only: a second family at right angles.
    pub double: bool,
    /// The lines that draw the pattern, final (see the module notes).
    pub lines: Vec<PatternLine>,
    /// Where the pattern is anchored (baked into the line bases).
    pub origin: Point,
}

/// One family of parallel lines, as DXF stores it (groups 53, 43/44, 45/46,
/// 49): the line through `base` at `angle`, repeated at every multiple of
/// `offset`.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternLine {
    /// Radians.
    pub angle: f64,
    pub base: Point,
    /// Step to the next line of the family, in drawing coordinates.
    pub offset: [f64; 2],
    /// Positive draws, negative skips, zero is a dot. Empty is continuous.
    pub dashes: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HatchGradient {
    /// The name as the file gave it (`LINEAR`, `CYLINDER`...).
    pub name: String,
    /// Radians.
    pub angle: f64,
    pub shift: f64,
    pub one_color: bool,
    pub tint: f64,
    /// `(position, red, green, blue)`.
    pub colors: Vec<(f64, [u8; 3])>,
}

impl HatchEntity {
    /// A hatch of `loops` filled with `fill`, in Normal style.
    pub fn new(loops: Vec<HatchLoop>, fill: HatchFill) -> Self {
        Self {
            elevation: 0.0,
            loops,
            style: HatchStyle::Normal,
            fill,
            associative: false,
            seed_points: Vec::new(),
            source_pixel_size: None,
        }
    }

    pub fn is_solid(&self) -> bool {
        !matches!(self.fill, HatchFill::Pattern(_))
    }

    pub fn pattern_name(&self) -> &str {
        match &self.fill {
            HatchFill::Pattern(p) => &p.name,
            HatchFill::Solid => "SOLID",
            HatchFill::Gradient(g) => &g.name,
        }
    }
}

#[cfg(test)]
mod operation_tests;
#[cfg(test)]
mod pattern_tests;
#[cfg(test)]
mod testing;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod trace_tests;
#[cfg(test)]
mod transform_tests;
