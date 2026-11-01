//! What DXF export checks about blocks before it writes them.

use od_core::{Document, Entity};

/// The tag of the first attribute, in the drawing or in a block, whose text
/// holds a data field.
pub(super) fn first_field(document: &Document) -> Option<String> {
    let loose = document.objects().iter().filter_map(|o| match &o.entity {
        Entity::AttributeDefinition(d) => d.default_value.has_fields().then(|| d.tag.clone()),
        Entity::Insert(i) => i
            .attributes
            .iter()
            .find(|r| r.value.has_fields())
            .map(|r| r.tag.clone()),
        _ => None,
    });
    let in_blocks = document.blocks().iter().flat_map(|b| {
        b.attribute_definitions()
            .filter(|(_, d)| d.default_value.has_fields())
            .map(|(_, d)| d.tag.clone())
    });
    loose.chain(in_blocks).next()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use od_core::block::{
        AttributeDefinition, AttributeFlags, AttributeText, DataField, FieldRun, FieldText,
    };
    use od_core::{Attributes, BlockDef, BlockEntity, Insert, LayerId, Point};

    use super::*;
    use crate::{FileError, FileFormat};

    /// A drawing with one insert of a block holding the attribute NAME, whose
    /// value is `value`.
    fn attributed(value: FieldText) -> Document {
        attributed_in(value, od_core::InsertUnits::UNITLESS)
    }

    fn attributed_in(value: FieldText, unit: od_core::InsertUnits) -> Document {
        let mut doc = Document::default();
        let def = AttributeDefinition {
            id: doc.allocate_attribute_definition_id(),
            tag: "NAME".into(),
            prompt: String::new(),
            default_value: "n/a".into(),
            flags: AttributeFlags(0),
            lock_position: false,
            field_length: 0,
            text: AttributeText::new(Point::new(0.0, 1.0), 1.0, od_core::TextStyleId::STANDARD),
        };
        let block = Arc::new(BlockDef {
            unit,
            id: doc.allocate_block_id(),
            name: "Tag".into(),
            base: Point::new(0.0, 0.0),
            description: String::new(),
            entities: vec![BlockEntity {
                entity: Entity::AttributeDefinition(Box::new(def)),
                attrs: Attributes::on_layer(LayerId::DEFAULT),
            }],
        });
        doc.push_block(block.clone());
        let mut insert = Insert::new(block.clone(), Point::new(5.0, 5.0));
        insert.attributes = block.new_references(|_| value.clone());
        doc.add(Entity::Insert(Box::new(insert)));
        doc
    }

    #[test]
    fn export_refuses_a_drawing_with_a_data_field() {
        let field = DataField::new("AcVar", "x", None);
        let doc = attributed(FieldText::Runs(vec![FieldRun::Field(field)]));
        assert_eq!(first_field(&doc), Some("NAME".to_string()));
        let Err(FileError::Unsupported(why)) = crate::DxfFormat.write(&doc) else {
            panic!("a field cannot be written to DXF yet")
        };
        assert!(why.contains("NAME") && why.contains("data field"), "{why}");
    }

    #[test]
    fn a_blocks_units_survive_a_dxf_round_trip() {
        let doc = attributed_in("Bob".into(), od_core::InsertUnits::from_code(5).unwrap());
        let bytes = crate::DxfFormat.write(&doc).unwrap().bytes;
        let back = crate::DxfFormat.read(&bytes).unwrap().document;
        let unit = back.blocks().first().unwrap().unit;
        assert_eq!(unit.label(), "Centimeters");
    }

    #[test]
    fn plain_values_are_not_fields() {
        assert_eq!(first_field(&attributed("Bob".into())), None);
    }
}
