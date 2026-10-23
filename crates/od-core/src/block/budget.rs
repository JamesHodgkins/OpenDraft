//! How much work expanding an insert can ask for. Nesting is a graph, not a
//! tree: a block that inserts the next one twice, ten levels down, expands to
//! a thousand objects from ten small definitions, and thirty levels to a
//! billion. Depth alone ([`MAX_NESTING`](super::MAX_NESTING)) does not bound
//! that, so expansion also has a budget of objects visited.

use std::collections::HashMap;

use super::{BlockDef, BlockId};
use crate::Entity;

/// The most objects one insert's expansion visits, nested ones included.
/// Painting, picking and snapping each expand inserts, so this bounds what a
/// single insert can cost them. Past it the picture is cut short.
pub const MAX_EXPANSION: usize = 1_000_000;

impl BlockDef {
    /// How many objects expanding an insert of this block visits: each object
    /// counts one, and an insert of another block also counts that block's
    /// own. Saturates. `memo` holds the answer per definition, so a doubling
    /// chain costs one step per definition, not one per object; share it
    /// across blocks of one table.
    pub fn expansion_size(&self, memo: &mut HashMap<BlockId, usize>) -> usize {
        if let Some(&size) = memo.get(&self.id) {
            return size;
        }
        let mut size = 0usize;
        for child in &self.entities {
            size = size.saturating_add(1);
            if let Entity::Insert(nested) = &child.entity {
                size = size.saturating_add(nested.block.expansion_size(memo));
            }
        }
        memo.insert(self.id, size);
        size
    }
}
