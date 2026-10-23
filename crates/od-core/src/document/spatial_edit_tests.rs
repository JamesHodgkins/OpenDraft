//! The spatial index through edits that change many objects, thresholds and
//! failures: what it holds must match a scan after each.

use super::spatial_query::Broad;
use super::spatial_tests::{grid, near, object, Rng, SMALL, SPACES};
use super::*;
use crate::{Layer, Point};

#[test]
fn a_small_query_in_a_big_space_looks_at_a_handful_of_objects() {
    let doc = grid(20_000);
    assert_eq!(doc.spatial_stats().builds, 1, "built once, on the way in");
    let before = doc.spatial_stats();
    let found = doc.objects_intersecting(None, near(500.0, 500.0));
    assert_eq!(found.len(), 1);
    let after = doc.spatial_stats();
    assert_eq!(after.tree_queries, before.tree_queries + 1);
    assert_eq!(after.scans, before.scans);
    assert!(after.visited - before.visited < 20, "{after:?}");
}

#[test]
fn small_spaces_and_whole_view_queries_scan() {
    let doc = grid(100);
    doc.objects_intersecting(None, near(5.0, 5.0));
    assert_eq!(doc.spatial_stats().tree_queries, 0);
    assert_eq!(doc.spatial_stats().scans, 1);

    let mut doc = grid(20);
    doc.set_index_config(SMALL);
    let everything = Bounds::from_points(Point::new(-1e9, -1e9), Point::new(1e9, 1e9));
    assert_eq!(doc.objects_intersecting(None, everything).len(), 20);
    assert_eq!(doc.spatial_stats().tree_queries, 0, "covers the root");
    doc.objects_intersecting(None, near(5.0, 5.0));
    assert_eq!(doc.spatial_stats().tree_queries, 1);
}

#[test]
fn an_unusable_query_rectangle_scans_and_finds_what_a_scan_finds() {
    let mut doc = grid(30);
    doc.set_index_config(SMALL);
    for bad in [
        Bounds {
            min: Point::new(f64::NAN, 0.0),
            max: Point::new(10.0, 10.0),
        },
        Bounds {
            min: Point::new(f64::NEG_INFINITY, 0.0),
            max: Point::new(f64::INFINITY, 10.0),
        },
        Bounds {
            min: Point::new(10.0, 10.0),
            max: Point::new(0.0, 0.0),
        },
    ] {
        let got: Vec<EntityId> = doc
            .objects_intersecting(None, bad)
            .iter()
            .map(|o| o.id)
            .collect();
        assert_eq!(got, doc.scan_ids(None, &bad), "{bad:?}");
    }
    assert_eq!(doc.spatial_stats().tree_queries, 0);
}

#[test]
fn objects_with_no_usable_box_are_tested_as_a_scan_tests_them() {
    let mut doc = grid(30);
    let nan = doc.add(Entity::Circle {
        center: Point::new(f64::NAN, 0.0),
        radius: 1.0,
    });
    let wide = doc.add(Entity::Circle {
        center: Point::new(55.0, 55.0),
        radius: f64::INFINITY,
    });
    doc.set_index_config(SMALL);
    doc.check_spatial_index();
    let r = near(25.0, 25.0);
    let ids: Vec<EntityId> = doc
        .objects_intersecting(None, r)
        .iter()
        .map(|o| o.id)
        .collect();
    assert_eq!(ids, doc.scan_ids(None, &r));
    assert!(
        !ids.contains(&nan),
        "a NaN box touches nothing, as in a scan"
    );
    assert!(ids.contains(&wide), "an infinite one touches everything");
    assert_eq!(doc.spatial_stats().tree_queries, 1);
}

#[test]
fn results_come_in_draw_order_even_after_edits_shift_positions() {
    let mut doc = grid(40);
    doc.set_index_config(SMALL);
    let ids: Vec<EntityId> = doc.objects().iter().map(|o| o.id).collect();
    let (at, moved) = doc.remove(ids[3]).unwrap_or_else(|| panic!("there"));
    doc.push(moved); // now last in draw order
    assert!(at < doc.len() - 1);
    let wide = Bounds::from_points(Point::new(-5.0, -5.0), Point::new(35.0, 15.0));
    let found: Vec<usize> = doc
        .objects_intersecting(None, wide)
        .iter()
        .filter_map(|o| doc.position(o.id))
        .collect();
    assert!(found.windows(2).all(|w| w[0] < w[1]), "{found:?}");
    assert_eq!(found.last(), Some(&(doc.len() - 1)));
}

#[test]
fn layer_state_and_viewport_freezing_are_applied_after_the_tree() {
    let mut doc = grid(40);
    let walls = doc.allocate_layer_id();
    doc.push_layer(Layer::new(walls, "Walls"));
    let mine = doc.add_with(
        Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 1.0,
        },
        Attributes::on_layer(walls),
    );
    doc.set_index_config(SMALL);
    let r = near(0.0, 0.0);
    let ids = |v: Vec<&Object>| v.iter().map(|o| o.id).collect::<Vec<_>>();
    assert!(ids(doc.visible_in_rect(None, &[], r)).contains(&mine));
    assert!(!ids(doc.visible_in_rect(None, &[walls], r)).contains(&mine));
    doc.set_active_frozen(vec![walls]);
    assert!(!ids(doc.visible_here_in_rect(r)).contains(&mine));
    assert!(!ids(doc.selectable_here_in_rect(r)).contains(&mine));
    doc.set_active_frozen(Vec::new());

    let mut layer = doc.layer(walls).cloned().unwrap_or_else(|| panic!("layer"));
    layer.locked = true;
    doc.replace_layer(layer.clone());
    assert!(
        ids(doc.visible_here_in_rect(r)).contains(&mine),
        "locked shows"
    );
    assert!(!ids(doc.selectable_here_in_rect(r)).contains(&mine));
    layer.locked = false;
    layer.on = false;
    doc.replace_layer(layer);
    assert!(!ids(doc.visible_here_in_rect(r)).contains(&mine));
    // None of that touched the tree.
    doc.check_spatial_index();
    assert_eq!(doc.spatial_stats().builds, 1);
}

#[test]
fn paper_spaces_and_model_space_are_indexed_apart() {
    let mut rng = Rng(7);
    let mut doc = Document::default();
    doc.set_index_config(SMALL);
    for i in 0..60 {
        let o = object(&mut doc, &mut rng, SPACES[i % 3]);
        doc.push(o);
    }
    assert_eq!(doc.indexed_spaces(), 3);
    let all = Bounds::from_points(Point::new(-50.0, -50.0), Point::new(150.0, 150.0));
    for space in SPACES {
        assert!(doc
            .objects_intersecting(space, all)
            .iter()
            .all(|o| o.layout == space));
    }
}

#[test]
fn a_tree_is_built_at_the_limit_and_dropped_only_well_below_it() {
    let mut doc = grid(11);
    doc.set_index_config(SMALL);
    assert_eq!(doc.indexed_spaces(), 0);
    doc.add(Entity::Circle {
        center: Point::new(0.0, 0.0),
        radius: 1.0,
    });
    assert_eq!(doc.indexed_spaces(), 1, "12 objects");
    let ids: Vec<EntityId> = doc.objects().iter().map(|o| o.id).collect();
    doc.remove(ids[0]);
    doc.remove(ids[1]);
    doc.remove(ids[2]);
    assert_eq!(doc.indexed_spaces(), 1, "9 left: above the lower limit");
    doc.remove(ids[3]);
    doc.remove(ids[4]);
    assert_eq!(doc.indexed_spaces(), 0, "7 left");
    doc.check_spatial_index();
    doc.add(Entity::Circle {
        center: Point::new(0.0, 0.0),
        radius: 1.0,
    });
    assert_eq!(doc.indexed_spaces(), 0, "8 is below the upper limit again");
}

#[test]
fn one_edit_updates_the_tree_and_a_big_one_rebuilds_it_once() {
    let mut doc = grid(200);
    doc.set_index_config(SMALL);
    let builds = |d: &Document| d.spatial_stats().builds;
    let base = builds(&doc);
    let ids: Vec<EntityId> = doc.objects().iter().map(|o| o.id).collect();

    doc.replace(
        ids[5],
        Entity::Circle {
            center: Point::new(1.0, 1.0),
            radius: 1.0,
        },
    );
    doc.add(Entity::Circle {
        center: Point::new(1.0, 1.0),
        radius: 1.0,
    });
    doc.remove(ids[6]);
    assert_eq!(builds(&doc), base, "single edits update in place");

    let changes: Vec<(EntityId, Entity)> = ids[10..110]
        .iter()
        .map(|id| {
            (
                *id,
                Entity::Circle {
                    center: Point::new(3.0, 3.0),
                    radius: 1.0,
                },
            )
        })
        .collect();
    let old = doc.replace_many(changes);
    assert_eq!(builds(&doc), base + 1, "100 of 200 rebuild once");
    doc.check_spatial_index();
    // Undo is the same call with the old entities.
    let back = ids[10..110]
        .iter()
        .zip(old)
        .filter_map(|(id, e)| e.map(|e| (*id, e)))
        .collect();
    doc.replace_many(back);
    assert_eq!(builds(&doc), base + 2);
    doc.check_spatial_index();
}

#[test]
fn an_entry_that_names_nothing_falls_back_to_a_scan_and_a_failed_removal_repairs() {
    let mut doc = grid(40);
    doc.set_index_config(SMALL);
    let r = near(0.0, 0.0);
    let ids: Vec<EntityId> = doc.objects().iter().map(|o| o.id).collect();

    // The tree holds an id the document no longer has.
    doc.positions.remove(&ids[0]);
    assert!(matches!(doc.broad(None, &r), Broad::Stale));
    doc.positions.insert(ids[0], 0);
    assert!(matches!(doc.broad(None, &r), Broad::Found(_)));

    // The tree has lost an entry the map still has: removing it fails and the
    // space is rebuilt rather than left wrong.
    let builds = doc.spatial_stats().builds;
    doc.corrupt_index(None, ids[7]);
    doc.remove(ids[7]);
    assert_eq!(doc.spatial_stats().builds, builds + 1);
    doc.check_spatial_index();
}

#[test]
fn the_index_is_not_part_of_what_a_drawing_is() {
    let plain = grid(30);
    let mut indexed = plain.clone();
    indexed.set_index_config(SMALL);
    assert_eq!(indexed.indexed_spaces(), 1);
    assert_eq!(plain.indexed_spaces(), 0);
    assert_eq!(plain, indexed);
    // A clone carries the tree, and it still answers.
    let copy = indexed.clone();
    copy.check_spatial_index();
    assert_eq!(copy.indexed_spaces(), 1);
    assert_eq!(
        copy.spatial_stats(),
        crate::SpatialStats::default(),
        "counters start again"
    );
}

#[test]
fn a_bulk_load_builds_once_at_the_end() {
    let mut doc = Document::default();
    doc.set_index_config(SMALL);
    doc.begin_bulk_load();
    for i in 0..500 {
        doc.add(Entity::Circle {
            center: Point::new(i as f64, 0.0),
            radius: 0.4,
        });
    }
    let r = near(250.0, 0.0);
    assert_eq!(
        doc.objects_intersecting(None, r).len(),
        5,
        "scans meanwhile"
    );
    assert_eq!(doc.spatial_stats().builds, 0);
    doc.finish_bulk_load();
    assert_eq!(doc.spatial_stats().builds, 1);
    doc.check_spatial_index();
    assert_eq!(doc.objects_intersecting(None, r).len(), 5);
    assert_eq!(doc.spatial_stats().tree_queries, 1);
}

#[test]
fn replacing_an_object_moves_it_in_the_tree() {
    let mut doc = grid(40);
    doc.set_index_config(SMALL);
    // Redefining a block replaces each insert through `replace`, so this is
    // the path that keeps an insert's box right.
    let id = doc.objects()[3].id;
    doc.replace(
        id,
        Entity::Circle {
            center: Point::new(900.0, 900.0),
            radius: 5.0,
        },
    );
    doc.check_spatial_index();
    assert_eq!(doc.objects_intersecting(None, near(900.0, 900.0)).len(), 1);
}

/// Architecture rule 8: a change to many objects is one pass. A loop of the
/// single-object calls, or a check for repeats that compares each id with
/// every other, would take minutes here; the limit is loose enough for an
/// unoptimised build.
#[test]
fn changing_and_adding_many_objects_is_one_pass() {
    let start = std::time::Instant::now();
    let mut doc = grid(50_000);
    let ids: Vec<EntityId> = doc.objects().iter().map(|o| o.id).collect();
    let changes = ids
        .iter()
        .map(|id| {
            (
                *id,
                Entity::Circle {
                    center: Point::new(1.0, 1.0),
                    radius: 1.0,
                },
            )
        })
        .collect();
    doc.replace_many(changes);
    let copies: Vec<Object> = (0..50_000)
        .map(|i| {
            let e = Entity::Circle {
                center: Point::new(i as f64, 2.0),
                radius: 1.0,
            };
            Object::new(
                doc.allocate_id(),
                e,
                Attributes::on_layer(LayerId::DEFAULT),
                None,
            )
        })
        .collect();
    doc.push_many(copies);
    assert_eq!(doc.len(), 100_000);
    assert!(start.elapsed().as_secs() < 10, "{:?}", start.elapsed());
    assert!(doc.spatial_stats().builds <= 3, "{:?}", doc.spatial_stats());
}
