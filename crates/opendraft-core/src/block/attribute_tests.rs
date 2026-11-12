//! Attributes in blocks: what an insert draws, what the drawing accepts, and
//! what a redefinition leaves behind.

use std::sync::Arc;

use crate::block::attribute::AttributeText;
use crate::block::{
    AttributeDefinition, AttributeDefinitionId, AttributeFlags, BlockDef, BlockEntity, BlockId,
    FieldText, Insert,
};
use crate::{Attributes, Document, DocumentError, Entity, LayerId, Point, TextStyleId};

fn definition(id: u64, tag: &str, flags: u16, at: Point) -> AttributeDefinition {
    AttributeDefinition {
        id: AttributeDefinitionId(id),
        tag: tag.into(),
        prompt: tag.into(),
        default_value: FieldText::from("dflt"),
        flags: AttributeFlags(flags),
        lock_position: false,
        field_length: 0,
        text: AttributeText::new(at, 1.0, TextStyleId::STANDARD),
    }
}

fn member(entity: Entity) -> BlockEntity {
    BlockEntity {
        entity,
        attrs: Attributes::on_layer(LayerId::DEFAULT),
    }
}

/// A block with a line, a plain attribute `A`, a constant `K` and an
/// invisible `H`, in a drawing that has the block.
fn drawing() -> (Document, Arc<BlockDef>) {
    let mut doc = Document::default();
    let ids: Vec<_> = (0..3)
        .map(|_| doc.allocate_attribute_definition_id())
        .collect();
    let at = |x| Point::new(x, 5.0);
    let block = Arc::new(BlockDef {
        unit: Default::default(),
        id: BlockId(1),
        name: "Tag".into(),
        base: Point::default(),
        description: String::new(),
        entities: vec![
            member(Entity::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(4.0, 0.0),
            }),
            member(Entity::AttributeDefinition(Box::new(definition(
                ids[0].0,
                "A",
                0,
                at(0.0),
            )))),
            member(Entity::AttributeDefinition(Box::new(definition(
                ids[1].0,
                "K",
                AttributeFlags::CONSTANT,
                at(10.0),
            )))),
            member(Entity::AttributeDefinition(Box::new(definition(
                ids[2].0,
                "H",
                AttributeFlags::INVISIBLE,
                at(20.0),
            )))),
        ],
    });
    doc.push_block(block.clone());
    (doc, block)
}

fn texts(pieces: &[Entity]) -> Vec<String> {
    pieces
        .iter()
        .filter_map(|e| match e {
            Entity::MText(m) => Some(m.contents.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn an_insert_draws_its_values_and_the_constant_but_not_invisible_ones() {
    let (_, block) = drawing();
    let mut insert = Insert::new(block.clone(), Point::new(100.0, 0.0));
    insert.attributes = block.new_references(|_| FieldText::from("mine"));
    assert_eq!(insert.attributes.len(), 2, "constant has no reference");
    let shown = texts(&insert.picture());
    assert!(shown.contains(&"mine".to_string()), "{shown:?}");
    assert!(
        shown.contains(&"dflt".to_string()),
        "the constant's value: {shown:?}"
    );
    assert_eq!(shown.len(), 2, "the invisible one is not drawn: {shown:?}");
    // The text lies above the line, so the box grows past it.
    assert!(insert.bounds().max.y > 5.0);
}

#[test]
fn an_insert_with_no_values_draws_only_the_constants() {
    let (_, block) = drawing();
    let insert = Insert::new(block, Point::default());
    assert_eq!(texts(&insert.picture()), ["dflt"]);
}

#[test]
fn exploding_leaves_visible_values_as_text() {
    let (_, block) = drawing();
    let mut insert = Insert::new(block.clone(), Point::default());
    insert.attributes = block.new_references(|_| FieldText::from("v"));
    let parts: Vec<Entity> = insert
        .exploded(&Attributes::on_layer(LayerId::DEFAULT), &|_| {
            crate::LayerLook::OUTSIDE_BLOCK
        })
        .into_iter()
        .map(|p| p.entity)
        .collect();
    assert!(parts
        .iter()
        .all(|e| !matches!(e, Entity::AttributeDefinition(_))));
    assert_eq!(texts(&parts).len(), 2);
}

#[test]
fn the_drawing_accepts_matching_references_and_refuses_the_rest() {
    let (mut doc, block) = drawing();
    let mut insert = Insert::new(block.clone(), Point::default());
    insert.attributes = block.new_references(|d| d.default_value.clone());
    let id = doc.add(Entity::Insert(Box::new(insert.clone())));
    assert_eq!(doc.validate(), Ok(()));

    let mut set = |change: &dyn Fn(&mut Insert)| {
        let mut i = insert.clone();
        change(&mut i);
        doc.replace(id, Entity::Insert(Box::new(i)));
        doc.validate()
    };
    // The same definition twice.
    assert!(matches!(
        set(&|i| i.attributes.push(i.attributes[0].clone())),
        Err(DocumentError::BadAttribute(_))
    ));
    // The constant one, which has no references.
    assert!(set(&|i| i.attributes[0].source = Some(AttributeDefinitionId(2))).is_err());
    // A definition the block does not have.
    assert!(set(&|i| i.attributes[0].source = Some(AttributeDefinitionId(99))).is_err());
    // An orphan is allowed.
    assert_eq!(set(&|i| i.attributes[0].source = None), Ok(()));
    // A bad tag.
    assert!(set(&|i| i.attributes[0].tag = "two words".into()).is_err());
}

#[test]
fn definition_ids_repeat_freely_when_loose_but_not_in_a_block() {
    let (mut doc, block) = drawing();
    let id = doc.allocate_attribute_definition_id();
    let loose = definition(id.0, "T", 0, Point::default());
    doc.add(Entity::AttributeDefinition(Box::new(loose.clone())));
    doc.add(Entity::AttributeDefinition(Box::new(loose)));
    assert_eq!(doc.validate(), Ok(()), "copies of a loose definition");
    assert!(doc.next_attribute_definition_id() > id.0);

    let mut twice = (*block).clone();
    twice.entities.push(twice.entities[1].clone());
    doc.set_blocks(vec![Arc::new(twice)]);
    assert!(
        doc.validate().is_err(),
        "two definitions with one id in a block"
    );
    let mut zero = (*block).clone();
    if let Entity::AttributeDefinition(d) = &mut zero.entities[1].entity {
        d.id = AttributeDefinitionId(0);
    }
    doc.set_blocks(vec![Arc::new(zero)]);
    assert!(doc.validate().is_err());
}

#[test]
fn redefining_a_block_orphans_references_whose_definition_went() {
    let (doc, block) = drawing();
    let mut insert = Insert::new(block.clone(), Point::default());
    insert.attributes = block.new_references(|_| FieldText::from("keep"));
    let mut shorter = (*block).clone();
    shorter
        .entities
        .retain(|e| !matches!(&e.entity, Entity::AttributeDefinition(d) if d.tag == "A"));
    let rebound = insert.rebound(Arc::new(shorter));
    let sources: Vec<_> = rebound.attributes.iter().map(|r| r.source).collect();
    assert_eq!(sources, [None, Some(AttributeDefinitionId(3))]);
    assert_eq!(rebound.attributes[0].value, FieldText::from("keep"));
    drop(doc);
}

/// A drawing with text style "Notes" and a block "Tag" whose one attribute
/// uses it, and an insert whose value is a field pointing at an object.
fn noted_source() -> (Document, crate::EntityId) {
    use crate::block::{DataField, FieldObjectRef, FieldRun, FieldValue};
    let mut doc = Document::default();
    let style = doc.allocate_text_style_id();
    doc.push_text_style(crate::TextStyle::new(style, "Notes"));
    let mut def = definition(
        doc.allocate_attribute_definition_id().0,
        "NAME",
        0,
        Point::new(0.0, 1.0),
    );
    def.text.style = style;
    let block = Arc::new(BlockDef {
        unit: Default::default(),
        id: doc.allocate_block_id(),
        name: "Tag".into(),
        base: Point::default(),
        description: String::new(),
        entities: vec![member(Entity::AttributeDefinition(Box::new(def)))],
    });
    doc.push_block(block.clone());
    let mut insert = Insert::new(block.clone(), Point::new(3.0, 3.0));
    insert.attributes = block.new_references(|_| FieldText::from("x"));
    let field = DataField {
        object_refs: vec![
            FieldObjectRef::Entity(crate::EntityId(7)),
            FieldObjectRef::TextStyle(style),
        ],
        ..DataField::new("AcObjProp", "len", Some(FieldValue::F64(2.5)))
    };
    insert.attributes[0].value = FieldText::Runs(vec![FieldRun::Field(field)]);
    let id = doc.add(Entity::Insert(Box::new(insert)));
    (doc, id)
}

fn paste(src: &Document, id: crate::EntityId, dst: &mut Document) -> crate::EntityId {
    use crate::document::Placement;
    let extract = src.extract(&[id]);
    let absorbed = dst.absorb(&extract, &Placement::Loose { dx: 0.0, dy: 0.0 }, None);
    absorbed.ids()[0]
}

#[test]
fn pasting_into_another_drawing_remaps_styles_and_detaches_field_references() {
    use crate::block::FieldObjectRef;
    let (src, id) = noted_source();
    let mut dst = Document::default();
    let pasted = paste(&src, id, &mut dst);
    assert_eq!(dst.validate(), Ok(()));
    let notes = dst.text_style_named("Notes").unwrap().id;
    let Some(Entity::Insert(i)) = dst.get(pasted) else {
        panic!("an insert")
    };
    assert_eq!(i.attributes[0].text.style, notes);
    assert_eq!(
        i.block.attribute_definitions().next().unwrap().1.text.style,
        notes
    );
    assert_eq!(i.attributes[0].source, Some(AttributeDefinitionId(1)));
    let refs = i.attributes[0].value.object_refs();
    assert!(matches!(refs[0], FieldObjectRef::Unresolved(_)), "{refs:?}");
    assert_eq!(refs[1], &FieldObjectRef::TextStyle(notes));
    assert_eq!(
        i.attributes[0].value.display(),
        "2.5",
        "the last value stays"
    );
}

#[test]
fn a_block_of_the_same_name_with_other_definitions_orphans_the_reference() {
    let (src, id) = noted_source();
    let mut dst = Document::default();
    // The target's "Tag" has definition 1 too, but it is a different tag.
    let other = Arc::new(BlockDef {
        unit: Default::default(),
        id: dst.allocate_block_id(),
        name: "Tag".into(),
        base: Point::default(),
        description: String::new(),
        entities: vec![member(Entity::AttributeDefinition(Box::new(definition(
            1,
            "OTHER",
            0,
            Point::default(),
        ))))],
    });
    dst.reserve_attribute_definition_ids(2);
    dst.push_block(other);
    let pasted = paste(&src, id, &mut dst);
    assert_eq!(dst.validate(), Ok(()));
    let Some(Entity::Insert(i)) = dst.get(pasted) else {
        panic!("an insert")
    };
    assert_eq!(
        i.attributes[0].source, None,
        "not another block's attribute"
    );
    assert_eq!(i.attributes[0].tag, "NAME", "but not lost");
}

#[test]
fn an_attribute_with_properties_of_its_own_draws_with_them_and_holds_its_layer() {
    use crate::{Attr, Color, Layer};
    let (mut doc, block) = drawing();
    let marks = doc.allocate_layer_id();
    doc.push_layer(Layer::new(marks, "Marks"));
    let mut insert = Insert::new(block.clone(), Point::default());
    insert.attributes = block.new_references(|d| d.default_value.clone());
    assert!(!insert.uses_layer_for_attributes(marks));
    insert.attributes[0].attrs = Some(Attributes {
        color: Attr::Set(Color::Rgb(255, 0, 0)),
        ..Attributes::on_layer(marks)
    });
    let id = doc.add(Entity::Insert(Box::new(insert.clone())));
    assert_eq!(doc.validate(), Ok(()));
    assert!(doc.layer_in_use(marks), "an attribute's layer is in use");
    // It is drawn on its own layer; the other follows its definition.
    let pieces = insert.picture_with(&Attributes::on_layer(LayerId::DEFAULT), &|_| {
        crate::LayerLook::OUTSIDE_BLOCK
    });
    let on_marks = pieces.iter().filter(|p| p.attrs.layer == marks).count();
    assert_eq!(on_marks, 1);
    // Without its layer the drawing is not valid.
    insert.attributes[0].attrs = Some(Attributes::on_layer(crate::LayerId(99)));
    doc.replace(id, Entity::Insert(Box::new(insert)));
    assert!(doc.validate().is_err());
}
