//! Dimensions are tested through the picture their object remembers: the same
//! answers as from the entity, and no picture is built again after the first
//! look.

use od_core::dimension::builds_on_this_thread;
use od_core::{DimStyle, Dimension, Document, Entity, Point};

use crate::hit::{distance_to_entity, distance_to_object, pick_here};
use crate::select::{select_in, selected, selected_object, Region};
use crate::snap::{candidates, object_candidates, snap, SnapModes};

fn dimensions(n: usize) -> Document {
    let style = DimStyle::default();
    let mut doc = Document::default();
    for i in 0..n {
        let (x, y) = ((i % 50) as f64 * 30.0, (i / 50) as f64 * 20.0);
        let (a, b) = (Point::new(x, y), Point::new(x + 20.0, y + (i % 7) as f64));
        let d = match i % 3 {
            0 => Dimension::aligned(a, b, Point::new(x, y + 6.0), &style),
            1 => Dimension::diameter(a, b, &style),
            _ => Dimension::aligned(b, a, Point::new(x + 9.0, y - 5.0), &style),
        };
        doc.add(Entity::Dimension(Box::new(d)));
    }
    doc
}

#[test]
fn the_remembered_picture_gives_what_the_entity_gives() {
    let doc = dimensions(150);
    let regions = [
        Region::Window(Region::rect_corners(
            Point::new(-5.0, -30.0),
            Point::new(400.0, 60.0),
        )),
        Region::Crossing(Region::rect_corners(
            Point::new(100.0, 0.0),
            Point::new(130.0, 25.0),
        )),
        Region::Fence(vec![Point::new(-10.0, 10.0), Point::new(1500.0, 12.0)]),
    ];
    for o in doc.objects() {
        for region in &regions {
            assert_eq!(selected_object(o, region), selected(&o.entity, region));
        }
        for p in [Point::new(15.0, 3.0), Point::new(300.0, 41.0)] {
            assert_eq!(distance_to_object(o, p), distance_to_entity(&o.entity, p));
        }
        let stripped: Vec<_> = object_candidates(o)
            .iter()
            .map(|c| (c.point, c.kind))
            .collect();
        let plain: Vec<_> = candidates(&o.entity)
            .iter()
            .map(|c| (c.point, c.kind))
            .collect();
        assert_eq!(stripped, plain);
    }
}

#[test]
fn after_the_first_look_picking_snapping_and_selecting_build_nothing() {
    let doc = dimensions(3_000);
    let region = Region::Crossing(Region::rect_corners(
        Point::new(-5.0, -30.0),
        Point::new(1600.0, 2000.0),
    ));
    // The first look builds each picture once (the box asks for it).
    select_in(&doc, &region);
    let built = builds_on_this_thread();
    assert!(built >= 3_000);
    let modes = SnapModes::default();
    for i in 0..200 {
        let c = Point::new((i % 50) as f64 * 30.0 + 10.0, (i / 50) as f64 * 20.0 + 1.0);
        pick_here(&doc, c, 5.0);
        snap(
            doc.visible_here_in_rect(crate::hit::probe(c, 5.0)),
            c,
            5.0,
            None,
            &modes,
        );
    }
    select_in(&doc, &region);
    assert_eq!(builds_on_this_thread(), built);
}
