use super::*;
use crate::{Attributes, Document, Entity, Layer, LayerId, Point};

fn line(x: f64) -> Entity {
    Entity::Line {
        start: Point::new(x, 0.0),
        end: Point::new(x + 1.0, 0.0),
    }
}

fn with_layer(doc: &mut Document, name: &str) -> LayerId {
    let id = doc.allocate_layer_id();
    doc.push_layer(Layer::new(id, name));
    id
}

fn item(entity: Entity, group: usize) -> Candidate {
    Candidate {
        entity,
        group,
        protected: false,
    }
}

#[test]
fn objects_on_different_layers_are_compared_apart_unless_layers_are_ignored() {
    let mut doc = Document::default();
    let other = with_layer(&mut doc, "Other");
    let a = doc.add(line(0.0));
    let b = doc.add_with(line(0.0), Attributes::on_layer(other));
    let chosen = [a, b];
    let apart = prepare(&doc, &chosen, &Settings::default(), false);
    assert_ne!(apart.items[0].group, apart.items[1].group);
    let mut together = Settings::default();
    together.ignore.layer = true;
    let same = prepare(&doc, &chosen, &together, false);
    assert_eq!(same.items[0].group, same.items[1].group);
    assert_eq!(same.ids, chosen);
}

#[test]
fn what_cannot_be_picked_is_skipped_unless_a_look_asks_for_everything() {
    let mut doc = Document::default();
    let locked = with_layer(&mut doc, "Locked");
    let id = doc.add_with(line(0.0), Attributes::on_layer(locked));
    let mut layer: Layer = doc.layer(locked).cloned().expect("the layer");
    layer.locked = true;
    doc.replace_layer(layer);
    let kept = doc.add(line(5.0));
    let chosen = [id, kept, crate::EntityId(9_999)];
    let command = prepare(&doc, &chosen, &Settings::default(), false);
    assert_eq!(
        (command.ids.len(), command.skipped),
        (1, 1),
        "gone ones just drop"
    );
    let look = prepare(&doc, &chosen, &Settings::default(), true);
    assert_eq!((look.ids.len(), look.skipped), (2, 0));
}

#[test]
fn a_cap_stops_a_comparison_that_would_take_too_long() {
    let items: Vec<Candidate> = (0..40)
        .map(|i| item(line(f64::from(i) * 10.0), 0))
        .collect();
    let settings = Settings::default();
    assert!(overkill(&items, &settings).is_ok());
    let too_many = overkill_capped(&items, &settings, 10, u64::MAX);
    assert_eq!(
        too_many,
        Err(OverkillError::TooManyObjects {
            count: 40,
            limit: 10
        })
    );
    let too_much = overkill_capped(&items, &settings, 100, 50);
    assert_eq!(too_much, Err(OverkillError::TooMuchWork { limit: 50 }));
}
