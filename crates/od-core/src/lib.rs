//! Drawing document model. No UI dependencies: this crate must build and be
//! testable without a window.

pub mod aci;
pub mod arc;
pub mod arc_make;
pub mod array;
pub mod block;
mod breaking;
pub mod bulge;
pub mod closed_pair;
mod corner;
mod corner_infinite;
mod curve_snap;
pub mod dimension;
pub mod divide;
mod document;
pub mod donut;
pub mod ellipse;
mod entity_kind;
mod explode;
mod extend;
mod fillet;
mod fillet_ellipse;
mod grip;
pub mod hatch;
pub mod infinite;
mod intersect;
mod join;
mod layer;
pub mod layout;
pub mod leader;
mod lengthen;
pub mod linetype;
pub mod loop_polyline;
pub mod mleader;
pub mod mtext;
mod offset;
pub mod ortho;
pub mod outline;
pub mod plot;
mod plot_hatch;
pub mod plot_model;
mod plot_text;
pub mod point_style;
mod polycorner;
pub mod polygon;
pub mod rect;
mod spatial_index;
pub mod spline;
mod split;
mod stretch;
mod tangent;
pub mod text;
pub mod text_style;
mod transform;
pub mod units;
mod validate;
#[cfg(test)]
mod validate_id_tests;
pub mod width;

pub use block::{Affine, BlockDef, BlockEntity, BlockId, Insert};
pub use corner::{Chamfer, CornerError, Joined};
pub use corner_infinite::{chamfer_lines, fillet};
pub use dimension::{Arrow, DimKind, DimStyle, DimText, Dimension};
pub use document::{Absorbed, Document, EntityId, Object, Placement, SpatialStats};
pub use ellipse::{Ellipse, EllipseError};
pub use entity_kind::EntityKind;
pub use extend::End;
pub use grip::{Grip, GripKind};
pub use hatch::HatchEntity;
pub use intersect::Curve;
pub use join::{join, Merged};
pub use layer::{Attr, Attributes, Color, Layer, LayerId, Lineweight, LINETYPES};
pub use layout::{
    format_scale, paper_size, paper_size_name, parse_scale, Layout, LayoutId, Viewport, ViewportId,
    MIN_SHEET, PAPER_SIZES,
};
pub use leader::Leader;
pub use linetype::{Linetype, LinetypeError, LinetypeId, LinetypeTable, StrokeElement};
pub use mleader::{MLeader, MLeaderStyle, MLeaderStyleId};
pub use mtext::MText;
pub use point_style::{PointMarks, PointStyle};
pub use polycorner::{round_polyline, Path, Round, Rounded};
pub use spline::{KnotParameterization, Spline, SplineError, SplineFitData};
pub use tangent::tangent_circle;
pub use text_style::{
    FontDescriptor, TextStyle, TextStyleId, DEFAULT_STROKE_WEIGHT, OD_ROMAN_FAMILY,
};
pub use units::{AngleFormat, InsertUnits, LengthFormat, Units};
pub use validate::DocumentError;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Entity {
    Line {
        start: Point,
        end: Point,
    },
    Circle {
        center: Point,
        radius: f64,
    },
    /// A piece of a circle, running counter-clockwise from `start` to `end`
    /// (radians; equal angles make a full circle). See [`arc`].
    Arc {
        center: Point,
        radius: f64,
        start: f64,
        end: f64,
    },
    Rectangle {
        min: Point,
        max: Point,
    },
    /// Connected segments through `points` (at least two); `closed` joins the
    /// last point back to the first. Rotating a rectangle off the axes makes
    /// one of these.
    ///
    /// A segment can be an arc. `bulges` is empty (every segment straight) or
    /// has one value per point: `bulges[i]` shapes the segment from point `i`
    /// to the next (to the first, for the closing segment of a closed
    /// polyline; the last one of an open polyline is unused). A bulge is the
    /// tangent of a quarter of the arc's angle, as in DXF: 0 is straight,
    /// positive turns counter-clockwise from the start of the segment to its
    /// end, negative clockwise, 1 is a half circle. See [`bulge`].
    ///
    /// A segment can also be wide. `widths` is empty (every segment a hairline)
    /// or has one `(start, end)` pair per point, indexed like `bulges`: the
    /// width of the segment from point `i` to the next at its start and end.
    /// Widths taper along a segment. See [`width`].
    Polyline {
        points: Vec<Point>,
        closed: bool,
        bulges: Vec<f64>,
        widths: Vec<(f64, f64)>,
    },
    /// A measurement with its lines, arrows and text. See [`dimension`]. It
    /// is drawn from its defining points and style, so the other kinds of
    /// object only ever see it through [`Dimension::picture`].
    Dimension(Box<Dimension>),
    /// A full ellipse or an elliptical arc. See [`ellipse`].
    Ellipse(Ellipse),
    /// A NURBS curve, with the fit points it was made through if any. See
    /// [`spline`].
    Spline(Box<Spline>),
    /// Multiline text. See [`mtext`]. Its geometry comes from the text
    /// engine, so the other kinds of object only see it as a frame.
    MText(Box<MText>),
    /// A block placed in the drawing. See [`block`]. Other kinds of object
    /// see it through [`Insert::picture`].
    Insert(Box<Insert>),
    /// A pattern, solid or gradient fill of boundary loops. See [`hatch`].
    Hatch(Box<HatchEntity>),
    /// Leader lines to a landing and text. See [`mleader`].
    MLeader(Box<MLeader>),
    /// The legacy leader (LEADER). See [`leader`].
    Leader(Box<Leader>),
    /// A single location (POINT). How it is drawn is the drawing's
    /// [`PointStyle`].
    Point {
        at: Point,
    },
    /// A line with no ends (XLINE) through `origin` along the unit vector
    /// `direction`. See [`infinite`].
    Xline {
        origin: Point,
        direction: Point,
    },
    /// A line from `origin` with no far end (RAY) along the unit vector
    /// `direction`. See [`infinite`].
    Ray {
        origin: Point,
        direction: Point,
    },
}

impl Entity {
    pub fn kind(&self) -> EntityKind {
        match self {
            Entity::Insert(_) => EntityKind::Insert,
            Entity::Line { .. } => EntityKind::Line,
            Entity::Circle { .. } => EntityKind::Circle,
            Entity::Arc { .. } => EntityKind::Arc,
            Entity::Ellipse(_) => EntityKind::Ellipse,
            Entity::Spline(_) => EntityKind::Spline,
            Entity::Rectangle { .. } => EntityKind::Rectangle,
            Entity::Polyline { .. } => EntityKind::Polyline,
            Entity::Dimension(_) => EntityKind::Dimension,
            Entity::MText(_) => EntityKind::MText,
            Entity::Hatch(_) => EntityKind::Hatch,
            Entity::MLeader(_) => EntityKind::MLeader,
            Entity::Leader(_) => EntityKind::Leader,
            Entity::Point { .. } => EntityKind::Point,
            Entity::Xline { .. } => EntityKind::Xline,
            Entity::Ray { .. } => EntityKind::Ray,
        }
    }
}

/// Axis-aligned bounding box in drawing units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}

impl Bounds {
    pub fn from_points(a: Point, b: Point) -> Self {
        Self {
            min: Point::new(a.x.min(b.x), a.y.min(b.y)),
            max: Point::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    pub fn union(self, other: Bounds) -> Bounds {
        Bounds {
            min: Point::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: Point::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    /// Whether all four sides are finite numbers.
    pub fn is_finite(&self) -> bool {
        [self.min.x, self.min.y, self.max.x, self.max.y]
            .iter()
            .all(|v| v.is_finite())
    }

    pub fn intersects(&self, other: &Bounds) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }
}

impl Entity {
    pub fn bounds(&self) -> Bounds {
        match self {
            Entity::Line { start, end } => Bounds::from_points(*start, *end),
            Entity::Circle { center, radius } => {
                let r = radius.abs();
                Bounds::from_points(
                    Point::new(center.x - r, center.y - r),
                    Point::new(center.x + r, center.y + r),
                )
            }
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => arc::bounds(*center, *radius, *start, *end),
            Entity::Ellipse(e) => e.bounds(),
            Entity::Spline(s) => s.bounds(),
            Entity::Rectangle { min, max } => Bounds::from_points(*min, *max),
            Entity::Polyline {
                points,
                closed,
                bulges,
                ..
            } => bulge::path_bounds(points, bulges, *closed)
                .unwrap_or_else(|| Bounds::from_points(Point::default(), Point::default())),
            Entity::Dimension(d) => d.bounds(),
            Entity::MText(m) => m.bounds(),
            Entity::Insert(i) => i.bounds(),
            Entity::MLeader(m) => m.bounds(),
            Entity::Leader(l) => l.bounds(),
            Entity::Point { at } => Bounds::from_points(*at, *at),
            Entity::Xline { origin, direction } => infinite::bounds(*origin, *direction, false),
            Entity::Ray { origin, direction } => infinite::bounds(*origin, *direction, true),
            Entity::Hatch(h) => h
                .bounds()
                .unwrap_or_else(|| Bounds::from_points(Point::default(), Point::default())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersects_detects_overlap_and_gap() {
        let a = Bounds::from_points(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        let b = Bounds::from_points(Point::new(5.0, 5.0), Point::new(20.0, 20.0));
        let c = Bounds::from_points(Point::new(11.0, 0.0), Point::new(20.0, 10.0));
        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }
}
