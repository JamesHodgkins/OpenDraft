//! Attributes from a DWG: an insert's attributes as references waiting for
//! their block (`block_import` moves them into its coordinates and matches
//! them to its definitions), and attribute definitions as entities.

use od_core::block::{
    AttributeDefinition, AttributeDefinitionId, AttributeFlags, AttributeReference, FieldText,
};
use od_core::{Document, Entity};
use od_dwg::{DwgAttribute, DwgHandle, Shape};

use super::text::{self, Styles};
use crate::block_import::Item;

/// The entity for an `ATTDEF` shape, or `None` when its values make none.
pub(super) fn definition(shape: &Shape, styles: &Styles) -> Option<Entity> {
    let Shape::AttributeDefinition {
        tag,
        prompt,
        flags,
        lock_position,
        field_length,
        text,
    } = shape
    else {
        return None;
    };
    let (text, value) = text::attribute_text(text, styles)?;
    let definition = AttributeDefinition {
        // Numbered when it is added to the document.
        id: AttributeDefinitionId(0),
        tag: tag.clone(),
        prompt: prompt.clone(),
        default_value: FieldText::Literal(value),
        flags: AttributeFlags(u16::from(*flags)),
        lock_position: *lock_position,
        field_length: *field_length,
        text,
    };
    definition.validate().ok()?;
    Some(Entity::AttributeDefinition(Box::new(definition)))
}

/// An insert's attribute as a reference with no definition yet, its text in
/// the drawing (`block_import` moves it into the block). `None` for one that
/// makes no attribute: a constant one (an insert holds none) or invalid text.
pub(super) fn reference(a: &DwgAttribute, styles: &Styles) -> Option<AttributeReference> {
    let (text, value) = text::attribute_text(&a.text, styles)?;
    let flags = AttributeFlags(u16::from(a.flags));
    let reference = AttributeReference {
        source: None,
        tag: a.tag.clone(),
        value: FieldText::Literal(value),
        flags,
        lock_position: a.lock_position,
        field_length: a.field_length,
        text,
        // The reader does not carry an attribute's own properties yet.
        attrs: None,
    };
    (!flags.constant() && reference.validate().is_ok()).then_some(reference)
}

/// How many of an insert's attributes `item` leaves out.
pub(super) fn dropped(shape: &Shape, styles: &Styles) -> usize {
    match shape {
        Shape::Insert { attributes, .. } => attributes
            .iter()
            .filter(|a| reference(a, styles).is_none())
            .count(),
        _ => 0,
    }
}

/// A definition about to be added to the document gets its own id.
pub(super) fn numbered(item: Item<DwgHandle>, document: &mut Document) -> Item<DwgHandle> {
    match item {
        Item::Entity(Entity::AttributeDefinition(mut d)) => {
            d.id = document.allocate_attribute_definition_id();
            Item::Entity(Entity::AttributeDefinition(d))
        }
        other => other,
    }
}
