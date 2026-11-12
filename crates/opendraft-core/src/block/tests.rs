use std::f64::consts::FRAC_PI_2;
use std::sync::Arc;

use super::*;
use crate::{Attr, Color, Document, Layer, LayerLook, Lineweight, Point};

pub(crate) fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

fn plain() -> Attributes {
    Attributes::on_layer(LayerId::DEFAULT)
}

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn on_zero(entity: Entity) -> BlockEntity {
    BlockEntity {
        entity,
        attrs: Attributes::on_layer(LayerId::DEFAULT),
    }
}

/// A unit square drawn with its corner at (10, 10), base point there, and a
/// circle in the middle.
pub(crate) fn square_block() -> Arc<BlockDef> {
    Arc::new(BlockDef {
        unit: Default::default(),
        id: BlockId(1),
        name: "Square".into(),
        base: p(10.0, 10.0),
        description: String::new(),
        entities: vec![
            on_zero(Entity::Rectangle {
                min: p(10.0, 10.0),
                max: p(11.0, 11.0),
            }),
            on_zero(Entity::Circle {
                center: p(10.5, 10.5),
                radius: 0.25,
            }),
        ],
    })
}

#[test]
fn an_insert_puts_the_base_point_at_the_insertion_point() {
    let insert = Insert::new(square_block(), p(0.0, 0.0));
    let b = insert.bounds();
    assert!(near(b.min, p(0.0, 0.0)) && near(b.max, p(1.0, 1.0)));
}

#[test]
fn scale_then_rotation_then_position() {
    let mut insert = Insert::new(square_block(), p(5.0, 5.0));
    insert.scale_x = 2.0;
    insert.scale_y = 2.0;
    insert.rotation = FRAC_PI_2;
    let m = insert.transform();
    // The block's (11, 10) corner is 1 along x: 2 after scaling, turned up.
    assert!(near(m.apply(p(11.0, 10.0)), p(5.0, 7.0)));
    let Entity::Circle { center, radius } = &insert.picture()[1] else {
        panic!("a uniform scale keeps the circle")
    };
    assert!(near(*center, p(4.0, 6.0)) && (radius - 0.5).abs() < 1e-12);
}

#[test]
fn an_uneven_scale_turns_circles_into_exact_ellipses() {
    let mut insert = Insert::new(square_block(), p(0.0, 0.0));
    insert.scale_x = 4.0;
    assert!(!insert.is_uniform());
    let Entity::Ellipse(e) = &insert.picture()[1] else {
        panic!("a circle under an uneven scale is an ellipse")
    };
    assert!(e.is_full());
    assert!(near(e.center, p(2.0, 0.5)));
    assert!((e.major_radius() - 1.0).abs() < 1e-12 && (e.minor_radius() - 0.25).abs() < 1e-12);
    for i in 0..16 {
        let q = e.point_at(f64::from(i));
        let (dx, dy) = ((q.x - 2.0) / 1.0, (q.y - 0.5) / 0.25);
        assert!((dx.hypot(dy) - 1.0).abs() < 1e-9, "on the ellipse");
    }
}

fn nested_doc() -> (Document, Arc<BlockDef>, Arc<BlockDef>) {
    let mut doc = Document::default();
    let inner = square_block();
    doc.push_block(inner.clone());
    let mut placed = Insert::new(inner.clone(), p(100.0, 0.0));
    placed.rotation = FRAC_PI_2;
    let outer = Arc::new(BlockDef {
        unit: Default::default(),
        id: doc.allocate_block_id(),
        name: "Pair".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        entities: vec![
            on_zero(Entity::Insert(Box::new(placed))),
            on_zero(Entity::Line {
                start: p(0.0, 0.0),
                end: p(1.0, 0.0),
            }),
        ],
    });
    doc.push_block(outer.clone());
    (doc, inner, outer)
}

#[test]
fn nested_blocks_combine_their_transforms() {
    let (_, _, outer) = nested_doc();
    let mut insert = Insert::new(outer, p(0.0, 0.0));
    insert.scale_x = 2.0;
    insert.scale_y = 2.0;
    let picture = insert.picture();
    assert_eq!(picture.len(), 3, "the square, its circle and the line");
    // The inner square, turned a quarter, sits at (200, 0) at twice size.
    let b = picture[0].bounds();
    assert!(near(b.min, p(198.0, 0.0)) && near(b.max, p(200.0, 2.0)));
    assert!(near(picture[2].bounds().max, p(2.0, 0.0)));
}

#[test]
fn layer_zero_objects_take_the_inserts_layer() {
    let (mut doc, _, outer) = nested_doc();
    let walls = doc.allocate_layer_id();
    doc.push_layer(Layer::new(walls, "Walls"));
    let red = Attributes {
        color: Attr::Set(Color::Rgb(255, 0, 0)),
        ..Attributes::on_layer(walls)
    };
    let pieces = Insert::new(outer, p(0.0, 0.0)).picture_with(&red, &|l| doc.layer_look(l));
    assert!(pieces.iter().all(|piece| piece.attrs.layer == walls));
    assert!(
        pieces
            .iter()
            .all(|piece| piece.attrs.color == Attr::ByLayer),
        "a colour on the insert is not a colour on its contents"
    );
}

#[test]
fn exploding_goes_one_level_down() {
    let (_, _, outer) = nested_doc();
    let parts = Insert::new(outer, p(1.0, 1.0)).exploded(&plain(), &|_| LayerLook::OUTSIDE_BLOCK);
    assert_eq!(parts.len(), 2);
    let Entity::Insert(inner) = &parts[0].entity else {
        panic!("the nested insert stays an insert")
    };
    assert!(near(inner.insertion, p(101.0, 1.0)));
    let mut squashed = Insert::new(square_block(), p(0.0, 0.0));
    squashed.scale_y = 3.0;
    let (_, _, outer) = nested_doc();
    let mut uneven = Insert::new(outer, p(0.0, 0.0));
    uneven.scale_y = 3.0;
    assert_eq!(
        uneven
            .exploded(&plain(), &|_| LayerLook::OUTSIDE_BLOCK)
            .len(),
        3,
        "an uneven scale expands it"
    );
    assert_eq!(
        squashed
            .exploded(&plain(), &|_| LayerLook::OUTSIDE_BLOCK)
            .len(),
        2
    );
}

#[test]
fn redefining_a_block_repoints_nested_definitions_and_inserts() {
    let (mut doc, inner, outer) = nested_doc();
    let placed = doc.add(Entity::Insert(Box::new(Insert::new(
        outer.clone(),
        p(0.0, 0.0),
    ))));
    assert_eq!(doc.validate(), Ok(()));
    assert_eq!(doc.block_references(inner.id), vec![placed]);
    assert!(doc.block_in_use(inner.id) && doc.block_in_use(outer.id));

    let mut bigger = (*inner).clone();
    bigger.entities.truncate(1);
    let change = doc.redefined_blocks(bigger).unwrap();
    assert_eq!(change.objects.len(), 1, "the insert of the outer block");
    doc.set_blocks(change.blocks);
    for (id, entity) in change.objects {
        doc.replace(id, entity);
    }
    assert_eq!(doc.validate(), Ok(()));
    let Some(Entity::Insert(i)) = doc.get(placed) else {
        panic!()
    };
    assert_eq!(i.picture().len(), 2, "the inner block lost its circle");
}

#[test]
fn a_block_cannot_insert_itself() {
    let (doc, inner, outer) = nested_doc();
    let mut looped = (*inner).clone();
    looped
        .entities
        .push(on_zero(Entity::Insert(Box::new(Insert::new(
            outer,
            p(0.0, 0.0),
        )))));
    assert!(doc.redefined_blocks(looped).is_err());
}

#[test]
fn validation_finds_stale_and_missing_blocks_and_bad_names() {
    let (mut doc, inner, _) = nested_doc();
    let mut stale = (*inner).clone();
    stale.base = p(1.0, 1.0);
    let id = doc.add(Entity::Insert(Box::new(Insert::new(
        Arc::new(stale),
        p(0.0, 0.0),
    ))));
    assert_eq!(doc.validate(), Err(DocumentError::BadInsert(id)));
    doc.remove(id);

    let mut zero = Insert::new(inner.clone(), p(0.0, 0.0));
    zero.scale_x = 0.0;
    let id = doc.add(Entity::Insert(Box::new(zero)));
    assert_eq!(doc.validate(), Err(DocumentError::BadInsert(id)));
    doc.remove(id);

    let mut twin = (*inner).clone();
    twin.id = doc.allocate_block_id();
    twin.name = "SQUARE".into();
    doc.push_block(Arc::new(twin));
    assert!(matches!(doc.validate(), Err(DocumentError::BadBlock(_))));
}

#[test]
fn block_names_follow_autocad() {
    assert!(block_name_problem("Door 900").is_none());
    assert!(block_name_problem("  ").is_some());
    assert!(block_name_problem("a*b").is_some());
    assert!(block_name_problem(&"x".repeat(256)).is_some());
}

#[test]
fn a_layer_used_inside_a_block_is_in_use() {
    let mut doc = Document::default();
    let walls = doc.allocate_layer_id();
    doc.push_layer(Layer::new(walls, "Walls"));
    let mut def = (*square_block()).clone();
    def.entities[0].attrs.layer = walls;
    doc.push_block(Arc::new(def));
    assert!(doc.layer_in_use(walls));
}

use crate::validate::DocumentError;

/// `levels` blocks above `leaf`, each inserting the one below twice.
fn doubling_chain(leaf: Arc<BlockDef>, levels: u64) -> Arc<BlockDef> {
    let mut below = leaf;
    for id in 1..=levels {
        let insert = || BlockEntity {
            entity: Entity::Insert(Box::new(Insert::new(below.clone(), p(0.0, 0.0)))),
            attrs: Attributes::on_layer(LayerId::DEFAULT),
        };
        below = Arc::new(BlockDef {
            unit: Default::default(),
            id: BlockId(id + 10),
            name: format!("level{id}"),
            base: p(0.0, 0.0),
            description: String::new(),
            entities: vec![insert(), insert()],
        });
    }
    below
}

#[test]
fn expansion_size_counts_nested_objects_once_per_definition() {
    let mut memo = std::collections::HashMap::new();
    assert_eq!(square_block().expansion_size(&mut memo), 2);
    // Each level holds two inserts (2) and two copies of the level below.
    let chain = doubling_chain(square_block(), 3);
    assert_eq!(
        chain.expansion_size(&mut memo),
        2 + 2 * (2 + 2 * (2 + 2 * 2))
    );
    let huge = doubling_chain(square_block(), 60);
    assert!(huge.expansion_size(&mut memo) > MAX_EXPANSION);
}

#[test]
fn a_block_that_fans_out_is_cut_short_not_expanded_forever() {
    // An empty leaf draws nothing, so only the budget stops the walk.
    let empty = Arc::new(BlockDef {
        unit: Default::default(),
        id: BlockId(1),
        name: "Empty".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        entities: Vec::new(),
    });
    let insert = Insert::new(doubling_chain(empty, 50), p(0.0, 0.0));
    assert!(insert.picture().is_empty());
    // One under the budget is expanded in full.
    let pieces = Insert::new(doubling_chain(square_block(), 12), p(0.0, 0.0)).picture();
    assert_eq!(pieces.len(), 2 << 12);
}

/// A block of one line, ByBlock in all three properties, on `layer`.
fn by_block_line(id: u64, layer: LayerId) -> Arc<BlockDef> {
    let attrs = Attributes {
        color: Attr::ByBlock,
        linetype: Attr::ByBlock,
        lineweight: Attr::ByBlock,
        ..Attributes::on_layer(layer)
    };
    Arc::new(BlockDef {
        unit: Default::default(),
        id: BlockId(id),
        name: format!("Line{id}"),
        base: p(0.0, 0.0),
        description: String::new(),
        entities: vec![BlockEntity {
            entity: Entity::Line {
                start: p(0.0, 0.0),
                end: p(1.0, 0.0),
            },
            attrs,
        }],
    })
}

fn red_dashed_heavy(layer: LayerId) -> Attributes {
    Attributes {
        color: Attr::Set(Color::Rgb(255, 0, 0)),
        linetype: Attr::Set(crate::LinetypeId::DASHED),
        lineweight: Attr::Set(Lineweight(50)),
        ..Attributes::on_layer(layer)
    }
}

#[test]
fn by_block_contents_take_the_inserts_properties() {
    let doc = Document::default();
    let insert = Insert::new(by_block_line(10, LayerId::DEFAULT), p(0.0, 0.0));
    let pieces = insert.picture_with(&red_dashed_heavy(LayerId::DEFAULT), &|l| doc.layer_look(l));
    assert_eq!(
        pieces[0].attrs,
        Attributes {
            layer: LayerId::DEFAULT,
            ..red_dashed_heavy(LayerId::DEFAULT)
        }
    );
}

#[test]
fn by_block_under_a_by_layer_insert_takes_that_layers_values_not_its_own() {
    let mut doc = Document::default();
    let (walls, notes) = (doc.allocate_layer_id(), doc.allocate_layer_id());
    let mut layer = Layer::new(walls, "Walls");
    layer.color = Color::Rgb(0, 0, 255);
    layer.lineweight = Lineweight(70);
    doc.push_layer(layer);
    let mut other = Layer::new(notes, "Notes");
    other.color = Color::Rgb(0, 255, 0);
    doc.push_layer(other);
    // The line sits on Notes but is ByBlock; its insert is ByLayer on Walls.
    let insert = Insert::new(by_block_line(11, notes), p(0.0, 0.0));
    let pieces = insert.picture_with(&Attributes::on_layer(walls), &|l| doc.layer_look(l));
    assert_eq!(pieces[0].attrs.layer, notes);
    assert_eq!(pieces[0].attrs.color, Attr::Set(Color::Rgb(0, 0, 255)));
    assert_eq!(pieces[0].attrs.lineweight, Attr::Set(Lineweight(70)));
}

#[test]
fn by_block_passes_through_nested_inserts_to_the_outermost_setting() {
    let doc = Document::default();
    let inner = by_block_line(12, LayerId::DEFAULT);
    let middle = Arc::new(BlockDef {
        unit: Default::default(),
        id: BlockId(13),
        name: "Middle".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        entities: vec![BlockEntity {
            entity: Entity::Insert(Box::new(Insert::new(inner, p(0.0, 0.0)))),
            attrs: Attributes {
                color: Attr::ByBlock,
                ..plain()
            },
        }],
    });
    let outer = Insert::new(middle, p(0.0, 0.0));
    let pieces = outer.picture_with(&red_dashed_heavy(LayerId::DEFAULT), &|l| doc.layer_look(l));
    assert_eq!(pieces[0].attrs.color, Attr::Set(Color::Rgb(255, 0, 0)));
    // The middle insert set no linetype of its own (ByLayer), so the line's
    // ByBlock linetype is its layer's.
    assert_eq!(
        pieces[0].attrs.linetype,
        Attr::Set(crate::LinetypeId::CONTINUOUS)
    );
}

#[test]
fn a_by_block_insert_leaves_contents_by_block_for_the_drawing_defaults() {
    let mut doc = Document::default();
    let by_block = Attributes {
        color: Attr::ByBlock,
        ..plain()
    };
    let insert = Insert::new(by_block_line(14, LayerId::DEFAULT), p(0.0, 0.0));
    let pieces = insert.picture_with(&by_block, &|l| doc.layer_look(l));
    assert_eq!(pieces[0].attrs.color, Attr::ByBlock);
    let id = doc.add_with(pieces[0].entity.clone(), pieces[0].attrs.clone());
    let object = doc.object(id).unwrap();
    assert_eq!(doc.color_of(object), Color::Foreground);
    assert_eq!(doc.linetype_of(object), crate::LinetypeId::CONTINUOUS);
    assert_eq!(doc.lineweight_of(object), Lineweight::DEFAULT);
}

#[test]
fn exploding_resolves_by_block_and_keeps_the_objects_own_layer() {
    let mut doc = Document::default();
    let notes = doc.allocate_layer_id();
    doc.push_layer(Layer::new(notes, "Notes"));
    let insert = Insert::new(by_block_line(15, notes), p(0.0, 0.0));
    let parts = insert.exploded(&red_dashed_heavy(LayerId::DEFAULT), &|l| doc.layer_look(l));
    assert_eq!(parts[0].attrs.layer, notes);
    assert_eq!(parts[0].attrs.color, Attr::Set(Color::Rgb(255, 0, 0)));
    assert_eq!(parts[0].attrs.lineweight, Attr::Set(Lineweight(50)));
}
