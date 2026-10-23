//! The snap points that need more than one entity or more than the pointer:
//! intersections of two curves, the perpendicular and tangent points from
//! the command's last point, and where the extension of a kept end meets
//! another curve.
//!
//! Everything is made of *pieces*: the segments, arcs, circles and ellipses
//! of an entity, and its spline. A block reference or dimension is made of
//! the pieces of what it draws.

use std::f64::consts::TAU;
use std::sync::Arc;

use od_core::{Bounds, Curve, Ellipse, Entity, Object, Point, Spline};

use super::track::{Geom, Track};
use super::{Class, Ranked, SnapKind, SnapPoint};

/// Most pieces in the aperture that are intersected with each other; the
/// pairs grow with the square, and a pointer over a dense patch should still
/// move at frame rate.
const PAIR_LIMIT: usize = 48;

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// One curve of an object.
#[derive(Clone, Copy)]
enum Piece<'a> {
    Curve(Curve),
    Spline(&'a Spline),
}

impl Piece<'_> {
    fn bounds(&self) -> Bounds {
        match self {
            Piece::Curve(c) => c.bounds(),
            Piece::Spline(s) => s.bounds(),
        }
    }

    fn nearest(&self, p: Point) -> Point {
        match self {
            Piece::Curve(c) => c.nearest_point(p),
            Piece::Spline(s) => s.nearest(p).0,
        }
    }

    /// Whether it bends (a straight segment has no tangent to a point).
    fn curved(&self) -> bool {
        !matches!(self, Piece::Curve(Curve::Segment { .. }))
    }

    /// Where it crosses `other`, among the part of each in `window`.
    fn meets(&self, other: &Piece<'_>, window: &Bounds) -> Vec<Point> {
        match (self, other) {
            (Piece::Curve(a), Piece::Curve(b)) => a.intersections(b),
            (Piece::Spline(s), Piece::Curve(c)) | (Piece::Curve(c), Piece::Spline(s)) => {
                s.crossings_with(c, window)
            }
            (Piece::Spline(a), Piece::Spline(b)) => a.crossings_with_spline(b, window),
        }
    }

    fn feet(&self, from: Point) -> Vec<Point> {
        match self {
            Piece::Curve(c) => c.perpendicular_feet(from),
            Piece::Spline(s) => s.perpendicular_feet(from),
        }
    }

    fn tangents(&self, from: Point) -> Vec<Point> {
        match self {
            Piece::Curve(c) => c.tangent_points(from),
            Piece::Spline(s) => s.tangent_points(from),
        }
    }
}

/// What a block reference or dimension draws.
enum Drawn {
    Block(Vec<Entity>),
    Dimension(Arc<[Entity]>),
}

impl Drawn {
    fn of(object: &Object) -> Option<Drawn> {
        match &object.entity {
            Entity::Insert(i) => Some(Drawn::Block(i.picture())),
            Entity::Dimension(_) | Entity::MLeader(_) | Entity::Leader(_) => {
                object.dimension_picture().map(Drawn::Dimension)
            }
            _ => None,
        }
    }

    fn entities(&self) -> &[Entity] {
        match self {
            Drawn::Block(e) => e,
            Drawn::Dimension(e) => e,
        }
    }
}

/// A piece with where it came from.
struct Item<'a> {
    object: &'a Object,
    /// Which of the object's shapes, and which curve of it.
    shape: usize,
    index: usize,
    /// Whether the curves of the shape run one into the next.
    chain: Chain,
    piece: Piece<'a>,
}

#[derive(Clone, Copy, PartialEq)]
enum Chain {
    /// The shape's curves are separate.
    None,
    /// They run one into the next, from the first to the last of `count`.
    Open,
    /// And the last runs into the first.
    Closed(usize),
}

impl Chain {
    /// Whether curves `a` and `b` of one shape meet at a corner of it.
    fn adjacent(self, a: usize, b: usize) -> bool {
        let (lo, hi) = (a.min(b), a.max(b));
        match self {
            Chain::None => false,
            Chain::Open => hi == lo + 1,
            Chain::Closed(n) => hi == lo + 1 || (lo == 0 && hi + 1 == n),
        }
    }
}

/// The pieces of the objects near the pointer.
pub(super) struct Scene<'a> {
    near: &'a [&'a Object],
    drawn: Vec<Option<Drawn>>,
}

impl<'a> Scene<'a> {
    pub fn new(near: &'a [&'a Object]) -> Self {
        Self {
            near,
            drawn: near.iter().map(|o| Drawn::of(o)).collect(),
        }
    }

    /// The pieces that pass through the aperture `capture`.
    fn items(&'a self, capture: &Bounds) -> Vec<Item<'a>> {
        let mut items = Vec::new();
        for (object, drawn) in self.near.iter().zip(&self.drawn) {
            let shapes = match drawn {
                Some(d) => d.entities(),
                None => std::slice::from_ref(&object.entity),
            };
            for (shape, entity) in shapes.iter().enumerate() {
                if let Entity::Spline(s) = entity {
                    if s.bounds().intersects(capture) {
                        items.push(Item {
                            object,
                            shape,
                            index: 0,
                            chain: Chain::None,
                            piece: Piece::Spline(s),
                        });
                    }
                    continue;
                }
                let curves = entity.curves();
                let chain = match entity {
                    Entity::Rectangle { .. } | Entity::Polyline { closed: true, .. } => {
                        Chain::Closed(curves.len())
                    }
                    Entity::Polyline { .. } => Chain::Open,
                    _ => Chain::None,
                };
                for (index, curve) in curves.into_iter().enumerate() {
                    if curve.bounds().intersects(capture) {
                        items.push(Item {
                            object,
                            shape,
                            index,
                            chain,
                            piece: Piece::Curve(curve),
                        });
                    }
                }
            }
        }
        items
    }
}

/// Where two pieces cross, within `tolerance` of `cursor`, as `kind`. A
/// polyline's own corners are its endpoints, not intersections; where it
/// crosses itself is one.
pub(super) fn intersections(
    scene: &Scene<'_>,
    capture: &Bounds,
    cursor: Point,
    tolerance: f64,
    kind: SnapKind,
) -> Vec<Ranked> {
    let items = scene.items(capture);
    let items = &items[..items.len().min(PAIR_LIMIT)];
    let mut found = Vec::new();
    for (i, a) in items.iter().enumerate() {
        for b in &items[i + 1..] {
            let same_shape = a.object.id == b.object.id && a.shape == b.shape;
            if same_shape && a.chain.adjacent(a.index, b.index) {
                continue;
            }
            if !a.piece.bounds().intersects(&b.piece.bounds()) {
                continue;
            }
            for point in a.piece.meets(&b.piece, capture) {
                let dist = distance(point, cursor);
                if dist <= tolerance {
                    let mut snap = SnapPoint::new(point, kind).from(a.object.id);
                    snap.source[1] = Some(b.object.id).filter(|id| *id != a.object.id);
                    found.push(Ranked {
                        snap,
                        dist,
                        class: Class::Point,
                    });
                }
            }
        }
    }
    found
}

/// The perpendicular feet and tangent points from `from` on every piece
/// under the pointer, that are themselves within `tolerance` of `cursor`: the
/// pointer has to be near the point, not just over the curve.
///
/// With no `from` (the command has no last point yet) the point cannot be
/// worked out: the snap is *deferred*, found at the nearest point of a piece
/// that could have one (any for Perpendicular, a curved one for Tangent), and
/// the command settles it when the next point is given.
pub(super) fn from_point(
    scene: &Scene<'_>,
    capture: &Bounds,
    cursor: Point,
    tolerance: f64,
    from: Option<Point>,
    want: (bool, bool),
) -> Vec<Ranked> {
    let (perpendicular, tangent) = want;
    let mut found = Vec::new();
    for item in scene.items(capture) {
        let object = item.object;
        let piece = item.piece;
        let mut add = |points: Vec<Point>, kind, deferred| {
            for point in points {
                let dist = distance(point, cursor);
                if dist <= tolerance {
                    let mut snap = SnapPoint::new(point, kind).from(object.id);
                    snap.deferred = deferred;
                    found.push(Ranked {
                        snap,
                        dist,
                        class: Class::Point,
                    });
                }
            }
        };
        match from {
            Some(from) => {
                if perpendicular {
                    add(piece.feet(from), SnapKind::Perpendicular, false);
                }
                if tangent {
                    add(piece.tangents(from), SnapKind::Tangent, false);
                }
            }
            None => {
                let at = vec![piece.nearest(cursor)];
                if perpendicular {
                    add(at.clone(), SnapKind::Perpendicular, true);
                }
                if tangent && piece.curved() {
                    add(at, SnapKind::Tangent, true);
                }
            }
        }
    }
    found
}

/// The path an acquired end gives, as a curve reaching a little past the
/// pointer.
fn path_curve(track: &Track, cursor: Point, tolerance: f64) -> Option<Curve> {
    match track.geom {
        Geom::Ray { from, dir } => {
            let reach = distance(from, cursor) + 4.0 * tolerance;
            Some(Curve::Segment {
                a: from,
                b: Point::new(from.x + dir.x * reach, from.y + dir.y * reach),
            })
        }
        Geom::Arc {
            center,
            radius,
            from,
            ccw,
            room,
        } => Some(Curve::Arc {
            center,
            radius,
            start: if ccw { from } else { from - room },
            end: if ccw { from + room } else { from },
        }),
        Geom::Ellipse {
            ellipse,
            from,
            ccw,
            room,
        } => {
            let start = if ccw { from } else { from - room }.rem_euclid(TAU);
            Some(Curve::Ellipse(Ellipse {
                start,
                end: start + room,
                ..ellipse
            }))
        }
        Geom::Along { .. } => None,
    }
}

/// Where the extension of an acquired end meets another object, within
/// `tolerance` of `cursor`.
pub(super) fn extended(
    scene: &Scene<'_>,
    capture: &Bounds,
    tracks: &[Track],
    cursor: Point,
    tolerance: f64,
) -> Vec<Ranked> {
    let items = scene.items(capture);
    let mut found = Vec::new();
    for track in tracks {
        let Some(path) = path_curve(track, cursor, tolerance).map(Piece::Curve) else {
            continue;
        };
        let own = track.key.object();
        for item in items.iter().filter(|i| Some(i.object.id) != own) {
            for point in path.meets(&item.piece, capture) {
                let dist = distance(point, cursor);
                if dist <= tolerance {
                    let mut snap =
                        SnapPoint::new(point, SnapKind::ExtendedIntersection).from(item.object.id);
                    snap.source[1] = own;
                    found.push(Ranked {
                        snap,
                        dist,
                        class: Class::TrackCross,
                    });
                }
            }
        }
    }
    found
}
