//! A drawing for the attribute command tests: one block "Label" with
//! attributes A and B, and one insert of it.

use std::sync::Arc;

use od_commands::SetBlocks;
use od_core::block::{
    AttributeDefinition, AttributeDefinitionId, AttributeFlags, AttributeText, FieldText,
};
use od_core::{
    Attributes, BlockDef, BlockEntity, BlockId, Entity, EntityId, Insert, LayerId, Point,
    TextStyleId,
};

use crate::state::AppState;

pub(crate) fn definition(id: u64, tag: &str, flags: u16) -> BlockEntity {
    BlockEntity {
        entity: Entity::AttributeDefinition(Box::new(AttributeDefinition {
            id: AttributeDefinitionId(id),
            tag: tag.into(),
            prompt: format!("{tag} please"),
            default_value: FieldText::from("dflt"),
            flags: AttributeFlags(flags),
            lock_position: false,
            field_length: 0,
            text: AttributeText::new(Point::new(0.0, 5.0), 1.0, TextStyleId::STANDARD),
        })),
        attrs: Attributes::on_layer(LayerId::DEFAULT),
    }
}

/// A drawing with block "Label" (attributes A and B) and one insert of it.
pub(crate) fn attributed_insert() -> (AppState, EntityId) {
    let mut state = AppState::new();
    let mut block = BlockDef {
        unit: Default::default(),
        id: BlockId(1),
        name: "Label".into(),
        base: Point::default(),
        description: String::new(),
        entities: vec![definition(1, "A", 0), definition(2, "B", 0)],
    };
    block.entities.push(BlockEntity {
        entity: Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(4.0, 0.0),
        },
        attrs: Attributes::on_layer(LayerId::DEFAULT),
    });
    let block = Arc::new(block);
    state.perform(SetBlocks::new("Block", vec![block.clone()]));
    let mut insert = Insert::new(block.clone(), Point::new(10.0, 10.0));
    insert.attributes = block.new_references(|d| d.default_value.clone());
    state.add_entity(Entity::Insert(Box::new(insert)));
    let id = state
        .session()
        .document()
        .objects()
        .last()
        .map(|o| o.id)
        .unwrap_or(EntityId(0));
    (state, id)
}

/// The displayed values of an insert's attributes.
pub(crate) fn values(state: &AppState, id: EntityId) -> Vec<String> {
    match state.session().document().get(id) {
        Some(Entity::Insert(i)) => i.attributes.iter().map(|r| r.value.display()).collect(),
        _ => panic!("not an insert"),
    }
}
