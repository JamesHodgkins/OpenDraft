use super::*;
use crate::text_style::TextStyleId;
use crate::{Attr, Attributes, Document, Entity, Layer, LayerId, MText, Point};

fn line(x1: f64, x2: f64) -> Entity {
    Entity::Line {
        start: Point::new(x1, 0.0),
        end: Point::new(x2, 0.0),
    }
}

fn names(doc: &Document, category: Category) -> Vec<String> {
    purgeable(doc)
        .into_iter()
        .filter(|p| p.item.category() == category)
        .map(|p| p.name)
        .collect()
}

fn with_layer(doc: &mut Document, name: &str) -> LayerId {
    let id = doc.allocate_layer_id();
    doc.push_layer(Layer::new(id, name));
    id
}

#[test]
fn a_new_drawing_offers_only_its_unused_stock_linetypes() {
    let found = purgeable(&Document::default());
    assert!(!found.is_empty());
    assert!(found
        .iter()
        .all(|p| p.item.category() == Category::Linetypes));
}

#[test]
fn zero_length_geometry_and_empty_text_are_found_exactly_and_the_rest_left() {
    let mut doc = Document::default();
    doc.add(line(0.0, 0.0));
    doc.add(line(0.0, 5.0));
    doc.add(line(0.0, 1e-300));
    doc.add(Entity::Circle {
        center: Point::new(0.0, 0.0),
        radius: 0.0,
    });
    doc.add(Entity::Arc {
        center: Point::new(0.0, 0.0),
        radius: 3.0,
        start: 1.0,
        end: 1.0,
    });
    let empty = MText::new(Point::new(0.0, 0.0), 2.5, "  ", TextStyleId::STANDARD);
    doc.add(Entity::MText(Box::new(empty)));
    let found = unnamed(&doc);
    assert_eq!(found.zero_length.len(), 2, "the line and the circle");
    assert_eq!(found.empty_text.len(), 1);
    assert!(purgeable(&doc)
        .iter()
        .all(|p| p.item.category() < Category::ZeroLength));
}

#[test]
fn unnamed_objects_on_a_locked_layer_stay() {
    let mut doc = Document::default();
    let id = with_layer(&mut doc, "Locked");
    doc.add_with(line(0.0, 0.0), Attributes::on_layer(id));
    let mut layer = doc.layer(id).cloned().expect("the layer");
    layer.locked = true;
    doc.replace_layer(layer);
    assert!(unnamed(&doc).zero_length.is_empty());
}

#[test]
fn a_layer_goes_only_when_nothing_is_on_it_and_it_is_not_current_or_zero() {
    let mut doc = Document::default();
    let id = with_layer(&mut doc, "Spare");
    assert_eq!(names(&doc, Category::Layers), ["Spare"]);
    doc.add_with(line(0.0, 1.0), Attributes::on_layer(id));
    assert!(names(&doc, Category::Layers).is_empty());
    let why = blocked(&doc)
        .into_iter()
        .find(|b| b.name == "Spare")
        .expect("it is listed as blocked");
    assert_eq!(why.reasons, ["1 object on it"]);
    let zero = blocked(&doc)
        .into_iter()
        .find(|b| b.item == Item::Layer(LayerId::DEFAULT))
        .expect("layer 0");
    assert!(zero.reasons.iter().any(|r| r.contains("permanent")));
}

#[test]
fn a_layer_frozen_in_a_viewport_stays() {
    let mut doc = Document::default();
    let id = with_layer(&mut doc, "Hidden");
    let mut layout = doc.new_layout("Sheet", 297.0, 210.0);
    let mut viewport = crate::Viewport::framing(
        crate::ViewportId(1),
        Point::new(100.0, 100.0),
        100.0,
        100.0,
        None,
    );
    viewport.set_frozen(&[id], true);
    layout.viewports.push(viewport);
    let layouts = vec![layout];
    doc.set_layouts(layouts);
    assert!(names(&doc, Category::Layers).is_empty());
    let why = blocked(&doc).into_iter().find(|b| b.name == "Hidden");
    assert!(why.is_some_and(|b| b.reasons == ["Frozen in a viewport"]));
}

#[test]
fn a_linetype_goes_only_when_no_layer_object_or_default_uses_it() {
    let mut doc = Document::default();
    assert!(names(&doc, Category::Linetypes).contains(&"DASHED".to_owned()));
    let mut attrs = Attributes::on_layer(LayerId::DEFAULT);
    attrs.linetype = Attr::Set("DASHED".to_owned());
    let id = doc.add_with(line(0.0, 1.0), attrs);
    let left = names(&doc, Category::Linetypes);
    assert!(!left.contains(&"DASHED".to_owned()) && left.contains(&"DOT".to_owned()));
    let why = blocked(&doc).into_iter().find(|b| b.name == "DASHED");
    assert!(why.is_some_and(|b| b.reasons == ["Used by objects or blocks"]));
    assert_eq!(users_of(&doc, &Item::Linetype("DASHED".to_owned())), [id]);
}

#[test]
fn what_stays_always_has_a_reason_and_what_goes_has_none() {
    let doc = Document::default();
    let goes = purgeable(&doc).len();
    let stays = blocked(&doc);
    assert!(stays.iter().all(|b| !b.reasons.is_empty()));
    let all = doc.blocks().len()
        + doc.dim_styles().len()
        + doc.layers().len()
        + doc.linetypes().records().len()
        + doc.mleader_styles().len()
        + doc.text_styles().len();
    assert_eq!(goes + stays.len(), all);
}
