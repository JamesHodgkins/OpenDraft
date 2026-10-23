//! An insert's picture: its block's objects mapped into the drawing.
//!
//! Under a map that keeps shapes (a turn, one size, a reflection) every kind
//! of object maps exactly, and a nested insert stays an insert with the maps
//! combined. A map with unequal x and y scales turns circles, arcs and
//! ellipses into exact ellipses ([`Ellipse::mapped`]) and maps splines exactly
//! ([`Spline::transformed`](crate::Spline::transformed)); polyline arc segments
//! become straight pieces close to the curve, and text keeps its shape at
//! the average size.

use crate::block::affine::Similarity;
use crate::block::{inherited, Affine, BlockEntity, Insert, MAX_EXPANSION, MAX_NESTING};
use crate::{bulge, Attributes, Bounds, Ellipse, Entity, Point};

/// One drawn object of an insert, with its layer and properties after
/// inheritance from the insert (see [`inherited`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub entity: Entity,
    pub attrs: Attributes,
}

/// How closely curves are followed when an uneven scale turns them into
/// polylines (arc segments, and ellipses squashed past the flattest one
/// allowed), as a share of their size.
const CURVE_TOLERANCE: f64 = 2e-3;

impl Insert {
    /// Every simple object the insert draws, in drawing coordinates, nested
    /// inserts expanded, with layers inherited from `attrs` (the insert's
    /// own).
    pub fn picture_with(&self, attrs: &Attributes) -> Vec<Piece> {
        let mut out = Vec::new();
        let mut budget = MAX_EXPANSION;
        self.expand(&Affine::IDENTITY, attrs, MAX_NESTING, &mut budget, &mut out);
        out
    }

    /// The shapes of [`picture_with`](Self::picture_with), for queries that
    /// need geometry only.
    pub fn picture(&self) -> Vec<Entity> {
        let attrs = Attributes::on_layer(crate::LayerId::DEFAULT);
        self.picture_with(&attrs)
            .into_iter()
            .map(|p| p.entity)
            .collect()
    }

    /// Add the insert's pieces to `out`, stopping at `depth` levels of nesting
    /// or when `budget` objects have been visited (see [`MAX_EXPANSION`]), so
    /// a block graph that fans out cannot run away.
    fn expand(
        &self,
        outer: &Affine,
        attrs: &Attributes,
        depth: usize,
        budget: &mut usize,
        out: &mut Vec<Piece>,
    ) {
        if depth == 0 {
            return;
        }
        let map = self.transform().then(outer);
        for child in &self.block.entities {
            if *budget == 0 {
                return;
            }
            *budget -= 1;
            let child_attrs = inherited(&child.attrs, attrs);
            match &child.entity {
                Entity::Insert(nested) => {
                    nested.expand(&map, &child_attrs, depth - 1, budget, out);
                }
                entity => out.extend(mapped(entity, &map).into_iter().map(|entity| Piece {
                    entity,
                    attrs: child_attrs.clone(),
                })),
            }
        }
    }

    /// The objects the insert breaks into when exploded, one level down: the
    /// block's objects mapped into the drawing with their own layers and
    /// properties. A nested insert stays an insert unless the scale is
    /// uneven, when it is expanded too.
    pub fn exploded(&self) -> Vec<BlockEntity> {
        let map = self.transform();
        let even = map.similarity().is_some();
        let mut out = Vec::new();
        for child in &self.block.entities {
            let parts = match &child.entity {
                Entity::Insert(nested) if !even => nested.picture_with(&child.attrs),
                entity => vec![Piece {
                    entity: entity.clone(),
                    attrs: child.attrs.clone(),
                }],
            };
            for part in parts {
                out.extend(
                    mapped(&part.entity, &map)
                        .into_iter()
                        .map(|entity| BlockEntity {
                            entity,
                            attrs: part.attrs.clone(),
                        }),
                );
            }
        }
        out
    }

    /// The box around everything the insert draws. An empty block has the
    /// insertion point as its box.
    pub fn bounds(&self) -> Bounds {
        self.picture()
            .iter()
            .map(Entity::bounds)
            .reduce(Bounds::union)
            .unwrap_or_else(|| Bounds::from_points(self.insertion, self.insertion))
    }
}

/// `entity` under `map`: one entity, exactly, when the map keeps shapes;
/// otherwise one entity that follows the mapped shape (see the module notes).
pub(crate) fn mapped(entity: &Entity, map: &Affine) -> Vec<Entity> {
    match map.similarity() {
        Some(s) => vec![similar(entity, &s)],
        None => uneven(entity, map),
    }
}

/// `entity` under a shape-keeping map, built from the exact transforms.
pub(crate) fn similar(entity: &Entity, s: &Similarity) -> Entity {
    let origin = Point::default();
    let mut e = if s.mirrored {
        entity.mirrored(origin, Point::new(1.0, 0.0))
    } else {
        entity.clone()
    };
    if s.scale != 1.0 {
        e = e.scaled(origin, s.scale);
    }
    if s.rotation != 0.0 {
        e = e.rotated(origin, s.rotation);
    }
    if s.tx != 0.0 || s.ty != 0.0 {
        e = e.translated(s.tx, s.ty);
    }
    e
}

fn polyline(points: Vec<Point>, closed: bool) -> Entity {
    Entity::Polyline {
        points,
        closed,
        bulges: Vec::new(),
        widths: Vec::new(),
    }
}

fn uneven(entity: &Entity, map: &Affine) -> Vec<Entity> {
    let all = |points: &[Point]| points.iter().map(|p| map.apply(*p)).collect::<Vec<_>>();
    match entity {
        Entity::Line { start, end } => vec![Entity::Line {
            start: map.apply(*start),
            end: map.apply(*end),
        }],
        Entity::Point { at } => vec![Entity::Point { at: map.apply(*at) }],
        // A block cannot hold these (the table refuses them).
        Entity::Xline { .. } | Entity::Ray { .. } => Vec::new(),
        Entity::Circle { center, radius } => {
            curved(Ellipse::circular(*center, *radius, 0.0, 0.0), map)
        }
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => curved(Ellipse::circular(*center, *radius, *start, *end), map),
        Entity::Ellipse(e) => curved(Ok(*e), map),
        Entity::Spline(s) => vec![Entity::Spline(Box::new(s.transformed(map)))],
        Entity::Rectangle { min, max } => {
            let corners = [
                *min,
                Point::new(max.x, min.y),
                *max,
                Point::new(min.x, max.y),
            ];
            vec![polyline(all(&corners), true)]
        }
        Entity::Polyline {
            points,
            closed,
            bulges,
            ..
        } => {
            // Wide segments lose their width: a width has no single size
            // once one direction is stretched more than another.
            if bulge::has_arcs(bulges) {
                let size = bulge::path_bounds(points, bulges, *closed)
                    .map_or(1.0, |b| (b.max.x - b.min.x).max(b.max.y - b.min.y));
                let mut flat = bulge::flatten_path(points, bulges, *closed, size * CURVE_TOLERANCE);
                if *closed && flat.len() > 1 {
                    flat.pop();
                }
                vec![polyline(all(&flat), *closed)]
            } else {
                vec![polyline(all(points), *closed)]
            }
        }
        Entity::Dimension(d) => d.picture().iter().flat_map(|p| uneven(p, map)).collect(),
        Entity::MLeader(m) => m.picture().iter().flat_map(|p| uneven(p, map)).collect(),
        Entity::Leader(l) => l.picture().iter().flat_map(|p| uneven(p, map)).collect(),
        Entity::MText(_) => {
            // Text keeps its shape: placed and turned with the map's x axis,
            // at its average size.
            let x_axis = Point::new(map.xx, map.yx);
            let s = Similarity {
                rotation: x_axis.y.atan2(x_axis.x),
                scale: map.mean_scale(),
                mirrored: false,
                tx: map.tx,
                ty: map.ty,
            };
            vec![similar(entity, &s)]
        }
        Entity::Insert(nested) => nested
            .picture()
            .iter()
            .flat_map(|p| uneven(p, map))
            .collect(),
        // A hatch cannot be placed in a block (the table refuses it), so
        // there is nothing of one to draw here.
        Entity::Hatch(_) => Vec::new(),
    }
}

/// A circle, arc or ellipse under an uneven map: an exact ellipse, or when
/// the map squashes it flatter than an ellipse may be, a polyline along it.
fn curved(shape: Result<Ellipse, crate::EllipseError>, map: &Affine) -> Vec<Entity> {
    let Ok(e) = shape else {
        return Vec::new();
    };
    if let Ok(mapped) = e.mapped(map) {
        return vec![Entity::Ellipse(mapped)];
    }
    let mut points = e.outline(e.major_radius() * CURVE_TOLERANCE);
    if e.is_full() {
        points.pop(); // the closing point repeats the first
    }
    let points = points.iter().map(|p| map.apply(*p)).collect();
    vec![polyline(points, e.is_full())]
}
