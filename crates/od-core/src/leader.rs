//! The legacy leader (AutoCAD's LEADER): a polyline of straight segments with
//! an arrowhead at its first point, and optionally the multiline text it
//! annotates, which is a separate object it refers to.
//!
//! The vertices are everything drawn: a hookline, when the leader has one, is
//! the last horizontal segment among them. The reference to the annotation is
//! kept (and written to DXF) but the leader does not follow the text when it
//! moves; the text may also be erased, which leaves the reference pointing at
//! nothing (ids are never reused, so it cannot point at anything else).
//! Spline leaders and tolerance or block annotations are not built.

use std::fmt;

use crate::dimension::{arrow_pieces, Arrow};
use crate::grip::{Grip, GripKind};
use crate::{Bounds, Entity, EntityId, Point};

/// Most points a leader may have.
pub const MAX_VERTICES: usize = 4_096;

#[derive(Clone, Debug, PartialEq)]
pub struct Leader {
    /// The arrowhead end first.
    pub vertices: Vec<Point>,
    /// Whether the arrowhead is drawn (it is not when the first segment is
    /// shorter than two arrowheads).
    pub arrow: bool,
    /// The arrowhead's size, with the dimension style's scale applied.
    pub arrow_size: f64,
    /// The last segment is a horizontal hookline.
    pub hookline: bool,
    /// DXF group 74: the hookline runs the way the horizontal vector does.
    pub hook_same_direction: bool,
    /// The dimension style it was made in (`DIMSTYLE` group 3).
    pub dim_style_name: String,
    /// The text it annotates.
    pub annotation: Option<EntityId>,
    /// The annotation's height and width as the file recorded them.
    pub annotation_size: (f64, f64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeaderError {
    NotFinite,
    BadVertices,
    BadSize,
}

impl fmt::Display for LeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFinite => write!(f, "a number is not finite"),
            Self::BadVertices => write!(f, "a leader needs 2 to {MAX_VERTICES} distinct points"),
            Self::BadSize => write!(f, "a size is negative"),
        }
    }
}

impl std::error::Error for LeaderError {}

impl Leader {
    /// A leader through `vertices` with an arrowhead of `arrow_size`.
    pub fn new(vertices: Vec<Point>, arrow_size: f64) -> Self {
        Self {
            vertices,
            arrow: true,
            arrow_size,
            hookline: false,
            hook_same_direction: false,
            dim_style_name: "Standard".to_owned(),
            annotation: None,
            annotation_size: (0.0, 0.0),
        }
    }

    pub fn validate(&self) -> Result<(), LeaderError> {
        let finite = |p: &Point| p.x.is_finite() && p.y.is_finite();
        let sizes = [
            self.arrow_size,
            self.annotation_size.0,
            self.annotation_size.1,
        ];
        if !self.vertices.iter().all(finite) || sizes.iter().any(|v| !v.is_finite()) {
            return Err(LeaderError::NotFinite);
        }
        let distinct = self
            .vertices
            .windows(2)
            .all(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y) > 1e-12);
        if self.vertices.len() < 2 || self.vertices.len() > MAX_VERTICES || !distinct {
            return Err(LeaderError::BadVertices);
        }
        if sizes.iter().any(|v| *v < 0.0) {
            return Err(LeaderError::BadSize);
        }
        Ok(())
    }

    /// The first segment's length.
    fn first_segment(&self) -> f64 {
        match self.vertices.as_slice() {
            [a, b, ..] => (b.x - a.x).hypot(b.y - a.y),
            _ => 0.0,
        }
    }

    /// Whether the arrowhead shows: asked for, and the first segment is at
    /// least two arrowheads long (as AutoCAD does).
    pub fn shows_arrow(&self) -> bool {
        self.arrow && self.arrow_size > 0.0 && self.first_segment() >= 2.0 * self.arrow_size
    }

    /// What draws it: the arrowhead and the line through the vertices.
    pub fn picture(&self) -> Vec<Entity> {
        let mut pieces = Vec::new();
        if let [tip, next, ..] = self.vertices.as_slice() {
            if self.shows_arrow() {
                let length = self.first_segment();
                pieces.extend(arrow_pieces(
                    Arrow::Closed,
                    *tip,
                    ((next.x - tip.x) / length, (next.y - tip.y) / length),
                    self.arrow_size,
                ));
            }
        }
        pieces.push(Entity::Polyline {
            points: self.vertices.clone(),
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        });
        pieces
    }

    pub fn bounds(&self) -> Bounds {
        self.bounds_of(&self.picture())
    }

    pub fn bounds_of(&self, picture: &[Entity]) -> Bounds {
        picture
            .iter()
            .map(Entity::bounds)
            .reduce(Bounds::union)
            .unwrap_or_else(|| Bounds::from_points(Point::default(), Point::default()))
    }

    /// Where an object snap can land: every vertex.
    pub fn snap_points(&self) -> Vec<Point> {
        self.vertices.clone()
    }

    fn map(&self, f: impl Fn(Point) -> Point) -> Leader {
        Leader {
            vertices: self.vertices.iter().map(|p| f(*p)).collect(),
            ..self.clone()
        }
    }

    pub fn translated(&self, dx: f64, dy: f64) -> Leader {
        self.map(|p| p.translated(dx, dy))
    }

    pub fn rotated(&self, center: Point, angle: f64) -> Leader {
        self.map(|p| p.rotated(center, angle))
    }

    /// Enlarged by `factor` about `base`: the points and the sizes.
    pub fn scaled(&self, base: Point, factor: f64) -> Leader {
        Leader {
            arrow_size: self.arrow_size * factor,
            annotation_size: (
                self.annotation_size.0 * factor,
                self.annotation_size.1 * factor,
            ),
            ..self.map(|p| p.scaled(base, factor))
        }
    }

    pub fn mirrored(&self, a: Point, b: Point) -> Leader {
        self.map(|p| p.mirrored(a, b))
    }

    /// Every vertex.
    pub fn grips(&self) -> Vec<Grip> {
        self.vertices
            .iter()
            .map(|p| Grip {
                at: *p,
                kind: GripKind::Stretch,
            })
            .collect()
    }

    /// The leader with vertex `index` dragged to `to`.
    pub fn with_grip_moved(&self, index: usize, to: Point) -> Option<Leader> {
        let mut moved = self.clone();
        *moved.vertices.get_mut(index)? = to;
        (to.x.is_finite() && to.y.is_finite()).then_some(moved)
    }

    /// The leader with the vertices for which `inside` is true moved by
    /// `(dx, dy)`; `None` when none is.
    pub fn stretched(&self, inside: &impl Fn(Point) -> bool, dx: f64, dy: f64) -> Option<Leader> {
        self.vertices
            .iter()
            .any(|p| inside(*p))
            .then(|| self.map(|p| if inside(p) { p.translated(dx, dy) } else { p }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn leader() -> Leader {
        Leader::new(vec![p(0.0, 0.0), p(10.0, 10.0), p(14.0, 10.0)], 2.5)
    }

    #[test]
    fn it_draws_an_arrowhead_and_the_line() {
        let pieces = leader().picture();
        assert_eq!(pieces.len(), 2);
        assert!(matches!(&pieces[1], Entity::Polyline { points, .. } if points.len() == 3));
        let b = leader().bounds();
        assert!(b.min.x <= 0.0 && b.max.x >= 14.0 && b.max.y >= 10.0);
    }

    #[test]
    fn the_arrowhead_goes_when_the_first_segment_is_under_two_arrowheads() {
        let mut short = Leader::new(vec![p(0.0, 0.0), p(4.0, 0.0), p(9.0, 3.0)], 2.5);
        assert!(!short.shows_arrow());
        assert_eq!(short.picture().len(), 1);
        short.vertices[1] = p(5.0, 0.0);
        assert!(
            short.shows_arrow(),
            "exactly two arrowheads long still shows"
        );
        short.arrow = false;
        assert_eq!(short.picture().len(), 1);
    }

    #[test]
    fn bad_leaders_are_refused() {
        assert_eq!(leader().validate(), Ok(()));
        let mut bad = leader();
        bad.vertices.truncate(1);
        assert_eq!(bad.validate(), Err(LeaderError::BadVertices));
        let mut bad = leader();
        bad.vertices[1] = bad.vertices[0];
        assert_eq!(bad.validate(), Err(LeaderError::BadVertices));
        let mut bad = leader();
        bad.vertices[2] = p(f64::NAN, 0.0);
        assert_eq!(bad.validate(), Err(LeaderError::NotFinite));
        let mut bad = leader();
        bad.arrow_size = -1.0;
        assert_eq!(bad.validate(), Err(LeaderError::BadSize));
    }

    #[test]
    fn moving_turning_and_scaling_change_the_points_and_sizes_once() {
        let l = leader();
        assert_eq!(l.translated(3.0, 1.0).translated(-3.0, -1.0), l);
        let s = l.scaled(p(0.0, 0.0), 2.0);
        assert_eq!(s.vertices[2], p(28.0, 20.0));
        assert_eq!(s.arrow_size, 5.0);
        let m = l.mirrored(p(0.0, 0.0), p(0.0, 1.0));
        assert_eq!(m.vertices[1], p(-10.0, 10.0));
        let t = l.rotated(p(0.0, 0.0), std::f64::consts::PI);
        assert!((t.vertices[1].x + 10.0).abs() < 1e-9);
    }

    #[test]
    fn grips_are_the_vertices_and_dragging_one_moves_it_alone() {
        let l = leader();
        assert_eq!(l.grips().len(), 3);
        let moved = l.with_grip_moved(1, p(10.0, 12.0)).unwrap();
        assert_eq!(moved.vertices, [p(0.0, 0.0), p(10.0, 12.0), p(14.0, 10.0)]);
        assert!(l.with_grip_moved(3, p(0.0, 0.0)).is_none());
        assert!(l.stretched(&|q: Point| q.x < -5.0, 1.0, 0.0).is_none());
        let s = l.stretched(&|q: Point| q.x > 12.0, 0.0, 2.0).unwrap();
        assert_eq!(s.vertices[2], p(14.0, 12.0));
        assert_eq!(s.vertices[0], l.vertices[0]);
    }
}
