//! An id of `u64::MAX` (only a damaged file has one) must neither panic nor
//! wrap a counter: the drawing is refused as having a counter behind.

use std::sync::Arc;

use crate::{
    Attributes, BlockDef, BlockId, Document, DocumentError, Entity, Layer, LayerId, Layout,
    LayoutId, Linetype, LinetypeId, LinetypeTable, Object, Point, TextStyle, TextStyleId,
};

const MAX: u64 = u64::MAX;

fn line() -> Entity {
    Entity::Line {
        start: Point::new(0.0, 0.0),
        end: Point::new(1.0, 1.0),
    }
}

fn refused(document: &Document) {
    assert_eq!(document.validate(), Err(DocumentError::CounterBehind));
}

#[test]
fn an_object_with_the_largest_id_is_refused() {
    let mut document = Document::default();
    document.push(Object::new(
        crate::EntityId(MAX),
        line(),
        Attributes::on_layer(LayerId::DEFAULT),
        None,
    ));
    refused(&document);
}

#[test]
fn a_layer_with_the_largest_id_is_refused() {
    let mut document = Document::default();
    document.push_layer(Layer::new(LayerId(MAX), "Last"));
    refused(&document);
}

#[test]
fn a_text_style_with_the_largest_id_is_refused() {
    let mut document = Document::default();
    document.push_text_style(TextStyle::new(TextStyleId(MAX), "Last"));
    refused(&document);
}

#[test]
fn a_block_with_the_largest_id_is_refused() {
    let mut document = Document::default();
    document.push_block(Arc::new(BlockDef {
        id: BlockId(MAX),
        name: "Last".into(),
        base: Point::new(0.0, 0.0),
        description: String::new(),
        entities: Vec::new(),
    }));
    refused(&document);
}

#[test]
fn a_layout_with_the_largest_id_is_refused() {
    let mut document = Document::default();
    document.set_layouts(vec![Layout {
        id: LayoutId(MAX),
        name: "Last".into(),
        width: 297.0,
        height: 210.0,
        viewports: Vec::new(),
    }]);
    refused(&document);
}

#[test]
fn a_linetype_with_the_largest_id_is_refused() {
    let mut table = LinetypeTable::default();
    table
        .push(Linetype {
            id: LinetypeId(MAX),
            name: "Last".into(),
            description: String::new(),
            elements: Vec::new(),
            flags: 0,
        })
        .unwrap();
    let mut document = Document::default();
    document.set_linetypes(table);
    refused(&document);
}
