//! The user's block definitions as `BLOCK` ... `ENDBLK` in the BLOCKS
//! section, each with its block record in the table. What a block holds is
//! written as it is, with its own insert records; a dimension or leader in a
//! block is written as the pieces it draws, since the pictures and
//! annotation handles those need are kept for the drawing's own objects.

use od_core::{BlockDef, Document, Entity, EntityId, Object};

use super::entities::write_object;
use super::{block_at, Handles, Ids};
use crate::dxf::write::Out;
use crate::FileError;

/// What stands for `entity` inside a block.
fn pieces(entity: &Entity) -> Vec<Entity> {
    match entity {
        Entity::Dimension(d) => d.picture(),
        Entity::Leader(l) => l.picture(),
        Entity::MLeader(m) => m.picture(),
        other => vec![other.clone()],
    }
}

/// One definition.
pub(super) fn write_definition(
    out: &mut Out,
    h: &mut Handles,
    ids: &Ids,
    document: &Document,
    def: &BlockDef,
) -> Result<(), FileError> {
    let Some(&record) = ids.blocks.get(&def.id) else {
        return Ok(());
    };
    block_at(
        out,
        h,
        (&def.name, &def.description),
        (record, def.base),
        |out, h| {
            // Nothing is looked up under this id.
            let none = EntityId(u64::MAX);
            for child in &def.entities {
                for entity in pieces(&child.entity) {
                    let object = Object::new(none, entity, child.attrs.clone(), None);
                    write_object(out, h, ids, document, &object, record, false)?;
                }
            }
            Ok(())
        },
    )
}
