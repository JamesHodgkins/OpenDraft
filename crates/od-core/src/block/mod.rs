//! Blocks: named definitions of a group of objects, and inserts (block
//! references) that place one with a position, scale and rotation.
//!
//! This follows AutoCAD's model in this project's structures. A definition
//! ([`BlockDef`]) is a record in the drawing's block table: a name, a base
//! point and its objects in their authored coordinates. An insert
//! ([`Insert`]) refers to a definition and never copies its geometry. A
//! definition may hold inserts of other blocks, so blocks nest.
//!
//! An insert carries its definition as a shared, immutable `Arc`, the same
//! one the block table holds, so every insert of a block shares one copy and
//! the context-free queries (bounds, picking, snapping, painting) can reach
//! the contents without the drawing. Nesting is therefore a graph that
//! cannot loop in memory: a definition can only hold inserts of blocks that
//! existed before it. Redefining a block builds new records and repoints
//! every insert in one action ([`Document::redefined_blocks`]).
//!
//! Other systems see an insert through its picture ([`Insert::picture`]):
//! its contents mapped into the drawing, nested blocks expanded.

mod affine;
mod budget;
mod edit;
mod editing;
mod picture;
mod table;

use std::sync::Arc;

pub use affine::Affine;
pub use budget::MAX_EXPANSION;
pub use editing::BlockEditSave;
pub use picture::Piece;
pub use table::{block_name_problem, Redefinition};

use crate::{Attributes, Entity, LayerId, LayerLook, Point};

/// Permanent identity of a block definition. Never reused within a drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(pub u64);

/// Deepest nesting followed when expanding a block. Far beyond any real
/// drawing; it only bounds the work a malformed file could ask for.
pub const MAX_NESTING: usize = 64;

/// One object of a block definition: geometry in the block's own
/// coordinates, and its layer and properties.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockEntity {
    pub entity: Entity,
    pub attrs: Attributes,
}

/// A block definition: the record in the block table.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockDef {
    pub id: BlockId,
    /// Unique in the drawing, ignoring case, as in AutoCAD.
    pub name: String,
    /// The point of the block's coordinates that an insert puts at its
    /// insertion point.
    pub base: Point,
    pub description: String,
    /// The objects, in draw order.
    pub entities: Vec<BlockEntity>,
}

impl BlockDef {
    /// The ids of the blocks this one inserts directly.
    pub fn nested_blocks(&self) -> impl Iterator<Item = BlockId> + '_ {
        self.entities.iter().filter_map(|e| match &e.entity {
            Entity::Insert(i) => Some(i.block.id),
            _ => None,
        })
    }

    /// Whether this block holds `target`, directly or through other blocks.
    /// A block counts as holding itself.
    pub fn holds(&self, target: BlockId) -> bool {
        self.id == target || self.holds_within(target, MAX_NESTING)
    }

    fn holds_within(&self, target: BlockId, depth: usize) -> bool {
        depth > 0
            && self.entities.iter().any(|e| match &e.entity {
                Entity::Insert(i) => {
                    i.block.id == target || i.block.holds_within(target, depth - 1)
                }
                _ => false,
            })
    }

    /// Whether any object in it, at any depth, sits on `layer`.
    pub fn uses_layer(&self, layer: LayerId) -> bool {
        self.entities.iter().any(|e| {
            e.attrs.layer == layer
                || matches!(&e.entity, Entity::Insert(i) if i.block.uses_layer(layer))
        })
    }
}

/// A block reference: a definition placed in the drawing (DXF `INSERT`).
///
/// Its transform maps the block's coordinates into the drawing: first the
/// base point to the origin, then the scale along the block's own axes, the
/// rotation, and finally the move to the insertion point.
#[derive(Clone, Debug, PartialEq)]
pub struct Insert {
    /// The definition, shared with the block table and every other insert.
    pub block: Arc<BlockDef>,
    pub insertion: Point,
    /// Scale along the block's x and y axes. Never zero; negative mirrors.
    pub scale_x: f64,
    pub scale_y: f64,
    /// Radians counter-clockwise.
    pub rotation: f64,
}

impl Insert {
    /// An insert of `block` at `insertion`, full size and unturned.
    pub fn new(block: Arc<BlockDef>, insertion: Point) -> Self {
        Self {
            block,
            insertion,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
        }
    }

    /// The map from the block's coordinates to the drawing's.
    pub fn transform(&self) -> Affine {
        Affine::translation(-self.block.base.x, -self.block.base.y)
            .then(&Affine::scaling(self.scale_x, self.scale_y))
            .then(&Affine::rotation(self.rotation))
            .then(&Affine::translation(self.insertion.x, self.insertion.y))
    }

    /// Whether the scales are finite and not zero, and the rest finite.
    pub fn is_valid(&self) -> bool {
        let finite = [
            self.insertion.x,
            self.insertion.y,
            self.scale_x,
            self.scale_y,
            self.rotation,
        ]
        .iter()
        .all(|v| v.is_finite());
        finite && self.scale_x != 0.0 && self.scale_y != 0.0
    }

    /// Whether the x and y scales have the same size (signs may differ), so
    /// circles stay circles.
    pub fn is_uniform(&self) -> bool {
        let (a, b) = (self.scale_x.abs(), self.scale_y.abs());
        (a - b).abs() <= 1e-9 * a.max(b)
    }
}

/// Where one object of a definition ends up: on the insert's layer when it
/// was drawn on layer 0 (as in AutoCAD), otherwise on its own; and with its
/// ByBlock colour, linetype and lineweight taken from the insert (`parent`),
/// whose ByLayer settings are made concrete with `look` of the insert's layer.
pub fn inherited(
    child: &Attributes,
    parent: &Attributes,
    look: &dyn Fn(LayerId) -> LayerLook,
) -> Attributes {
    let layer_look = look(parent.layer);
    let mut out = Attributes {
        color: child.color.inside(&parent.color, || layer_look.color),
        linetype: child
            .linetype
            .inside(&parent.linetype, || layer_look.linetype),
        lineweight: child
            .lineweight
            .inside(&parent.lineweight, || layer_look.lineweight),
        ..child.clone()
    };
    if child.layer == LayerId::DEFAULT {
        out.layer = parent.layer;
    }
    out
}

#[cfg(test)]
pub(crate) mod tests;
