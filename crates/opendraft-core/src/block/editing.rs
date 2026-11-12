//! Saving the Block Editor's work back into the drawing.
//!
//! The editor works on its own drawing (`Document::block_editor_document`).
//! Saving makes the edited block's objects its new definition, base point at
//! the origin, and brings back what else changed there that the drawing
//! must know: new or changed layers, and blocks made or redefined while
//! editing. Layers deleted in the editor are not deleted from the drawing,
//! which may still use them.
//!
//! Everything is worked out on a copy first, so a save that cannot be made
//! (a block that would hold itself) changes nothing.

use std::sync::Arc;

use crate::block::{BlockDef, BlockEntity, BlockId};
use crate::{Document, Entity, EntityId, Layer, Point};

/// What saving the Block Editor changes in the drawing, ready to be
/// recorded as actions.
#[derive(Clone, Debug)]
pub struct BlockEditSave {
    pub new_layers: Vec<Layer>,
    pub changed_layers: Vec<Layer>,
    /// The whole block table after the save.
    pub blocks: Vec<Arc<BlockDef>>,
    /// Inserts that now point at new records.
    pub objects: Vec<(EntityId, Entity)>,
}

impl Document {
    /// What saving `editor` (the Block Editor's drawing for block `id`)
    /// changes in this drawing.
    pub fn block_edit_save(
        &self,
        editor: &Document,
        id: BlockId,
    ) -> Result<BlockEditSave, &'static str> {
        let old = self
            .block(id)
            .ok_or("The block being edited is no longer in the drawing.")?;
        let mut work = self.clone();
        let (mut new_layers, mut changed_layers) = (Vec::new(), Vec::new());
        for layer in editor.layers() {
            match work.layer(layer.id) {
                None => {
                    new_layers.push(layer.clone());
                    work.push_layer(layer.clone());
                }
                Some(known) if known != layer => {
                    changed_layers.push(layer.clone());
                    work.replace_layer(layer.clone());
                }
                Some(_) => {}
            }
        }
        // Blocks made or redefined in the editor, then the edited block.
        let mut defs: Vec<BlockDef> = editor
            .blocks()
            .iter()
            .filter(|b| b.id != id && work.block(b.id).is_none_or(|known| **known != ***b))
            .map(|b| (**b).clone())
            .collect();
        let mut edited = BlockDef {
            unit: old.unit,
            id,
            name: old.name.clone(),
            base: Point::default(),
            description: old.description.clone(),
            entities: editor
                .model_objects()
                .map(|o| BlockEntity {
                    entity: o.entity.clone(),
                    attrs: o.attrs.clone(),
                })
                .collect(),
        };
        // A copied definition arrives with its original's id.
        edited.renumber_duplicate_attributes(|| work.allocate_attribute_definition_id());
        defs.push(edited);
        for def in defs {
            let change = work.redefined_blocks(def)?;
            work.set_blocks(change.blocks);
            for (object, entity) in change.objects {
                work.replace(object, entity);
            }
        }
        if work.validate().is_err() {
            return Err("The block cannot be saved: it uses something the drawing does not have, such as two layers with one name.");
        }
        let objects = work
            .objects()
            .iter()
            .filter(|o| matches!(o.entity, Entity::Insert(_)) && self.get(o.id) != Some(&o.entity))
            .map(|o| (o.id, o.entity.clone()))
            .collect();
        Ok(BlockEditSave {
            new_layers,
            changed_layers,
            blocks: work.blocks().to_vec(),
            objects,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::tests::{near, square_block};
    use crate::{Attributes, Insert};

    fn drawing() -> (Document, EntityId) {
        let mut doc = Document::default();
        doc.push_block(square_block());
        let placed = doc.add(Entity::Insert(Box::new(Insert::new(
            square_block(),
            Point::new(100.0, 0.0),
        ))));
        (doc, placed)
    }

    #[test]
    fn the_editor_shows_the_block_with_its_base_point_at_the_origin() {
        let (doc, _) = drawing();
        let editor = doc.block_editor_document(square_block().id).unwrap();
        assert_eq!(editor.len(), 2);
        let b = editor.objects()[0].entity.bounds();
        assert!(near(b.min, Point::new(0.0, 0.0)) && near(b.max, Point::new(1.0, 1.0)));
        assert!(editor.layouts().is_empty() && editor.blocks().len() == 1);
        assert_eq!(editor.validate(), Ok(()));
        assert_eq!(doc.block_editor_document(BlockId(9)), None);
    }

    #[test]
    fn saving_redefines_the_block_and_moves_every_reference_with_it() {
        let (doc, placed) = drawing();
        let id = square_block().id;
        let mut editor = doc.block_editor_document(id).unwrap();
        let circle = editor.objects()[1].id;
        editor.remove(circle);
        let walls = editor.allocate_layer_id();
        editor.push_layer(Layer::new(walls, "Walls"));
        editor.add_with(
            Entity::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(5.0, 0.0),
            },
            Attributes::on_layer(walls),
        );
        let save = doc.block_edit_save(&editor, id).unwrap();
        assert_eq!(save.new_layers.len(), 1);
        assert_eq!(save.objects.len(), 1);
        let (object, Entity::Insert(insert)) = &save.objects[0] else {
            panic!("the reference follows the new definition")
        };
        assert_eq!(*object, placed);
        let b = insert.bounds();
        assert!(near(b.min, Point::new(100.0, 0.0)) && near(b.max, Point::new(105.0, 1.0)));
    }

    #[test]
    fn a_block_that_would_hold_itself_is_refused() {
        let (doc, _) = drawing();
        let id = square_block().id;
        let mut editor = doc.block_editor_document(id).unwrap();
        editor.add(Entity::Insert(Box::new(Insert::new(
            square_block(),
            Point::new(3.0, 3.0),
        ))));
        assert!(doc.block_edit_save(&editor, id).is_err());
    }
}
