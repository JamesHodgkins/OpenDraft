//! Geometric centre, apparent and extended intersection, and the pairwise
//! modes over splines, block contents and a polyline's own crossings.

use std::sync::Arc;

use od_core::{Attributes, BlockDef, BlockEntity, BlockId, Entity, Insert, LayerId, Point, Spline};

use super::*;
use crate::testing::objs;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> Entity {
    Entity::Line {
        start: p(x1, y1),
        end: p(x2, y2),
    }
}

fn polyline(points: &[(f64, f64)], closed: bool) -> Entity {
    Entity::Polyline {
        points: points.iter().map(|(x, y)| p(*x, *y)).collect(),
        closed,
        bulges: Vec::new(),
        widths: Vec::new(),
    }
}

fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).hypot(a.y - b.y) < 1e-6
}

/// Only `kind`, no grid.
fn only(kind: SnapKind) -> SnapModes {
    let mut modes = SnapModes {
        grid: false,
        ..SnapModes::default()
    };
    for k in SnapKind::OBJECT {
        modes.set(k, k == kind);
    }
    modes
}

fn all(
    entities: &[Entity],
    cursor: Point,
    modes: &SnapModes,
    from: Option<Point>,
) -> Vec<SnapPoint> {
    let probe = Probe {
        cursor,
        tolerance: 1.0,
        grid_step: None,
        modes,
        from,
        tracks: &[],
        track: None,
    };
    snap_all(&objs(entities), &probe)
}

/// y = 2x - x²/5: a cap from (0, 0) over (5, 5) to (10, 0).
fn cap() -> Entity {
    Entity::Spline(Box::new(
        Spline::from_control_points(2, vec![p(0.0, 0.0), p(5.0, 10.0), p(10.0, 0.0)]).unwrap(),
    ))
}

#[test]
fn a_closed_polyline_offers_the_centre_of_its_area() {
    let triangle = polyline(&[(0.0, 0.0), (6.0, 0.0), (0.0, 3.0)], true);
    let modes = only(SnapKind::GeometricCenter);
    let found = all(std::slice::from_ref(&triangle), p(2.2, 0.8), &modes, None);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, SnapKind::GeometricCenter);
    assert!(near(found[0].point, p(2.0, 1.0)));
    assert_eq!(found[0].label(), "Geometric Center");
    // Off by default, and not for an open or a self-crossing outline.
    assert!(all(&[triangle], p(2.2, 0.8), &SnapModes::default(), None)
        .iter()
        .all(|s| s.kind != SnapKind::GeometricCenter));
    let open = polyline(&[(0.0, 0.0), (6.0, 0.0), (0.0, 3.0)], false);
    assert!(all(&[open], p(2.2, 0.8), &modes, None).is_empty());
    let bow = polyline(&[(0.0, 0.0), (4.0, 4.0), (4.0, 0.0), (0.0, 4.0)], true);
    assert!(all(&[bow], p(2.0, 2.0), &modes, None).is_empty());
}

#[test]
fn a_closed_spline_offers_the_centre_of_its_area() {
    // A closed degree 1 spline is a polygon: this one is a square.
    let ring = Spline::new_nurbs(
        1,
        vec![
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(0.0, 0.0),
        ],
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        None,
        true,
        false,
    )
    .unwrap();
    let found = all(
        &[Entity::Spline(Box::new(ring))],
        p(2.3, 1.8),
        &only(SnapKind::GeometricCenter),
        None,
    );
    assert_eq!(found.len(), 1);
    assert!(near(found[0].point, p(2.0, 2.0)), "{:?}", found[0].point);
}

#[test]
fn apparent_intersection_is_found_where_intersection_would_be() {
    let cross = [line(0.0, 0.0, 10.0, 10.0), line(0.0, 10.0, 10.0, 0.0)];
    let found = all(
        &cross,
        p(5.2, 4.9),
        &only(SnapKind::ApparentIntersection),
        None,
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, SnapKind::ApparentIntersection);
    assert_eq!(found[0].label(), "Apparent Intersection");
    assert!(near(found[0].point, p(5.0, 5.0)));
    // With both on the point is listed once, as the true intersection.
    let both = SnapModes {
        apparent_intersection: true,
        ..only(SnapKind::Intersection)
    };
    let found = all(&cross, p(5.2, 4.9), &both, None);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, SnapKind::Intersection);
}

#[test]
fn a_spline_crosses_a_line_and_another_spline() {
    let modes = only(SnapKind::Intersection);
    let level = line(-1.0, 3.0, 11.0, 3.0);
    let r = 10.0_f64.sqrt();
    let found = all(&[cap(), level], p(5.0 + r + 0.2, 3.1), &modes, None);
    assert_eq!(found.len(), 1);
    assert!(
        near(found[0].point, p(5.0 + r, 3.0)),
        "{:?}",
        found[0].point
    );
    let flat = Entity::Spline(Box::new(
        Spline::from_control_points(1, vec![p(0.0, 2.0), p(10.0, 2.0)]).unwrap(),
    ));
    let r = 15.0_f64.sqrt();
    let found = all(&[cap(), flat], p(5.0 - r + 0.2, 2.1), &modes, None);
    assert_eq!(found.len(), 1);
    assert!(
        near(found[0].point, p(5.0 - r, 2.0)),
        "{:?}",
        found[0].point
    );
}

#[test]
fn perpendicular_and_tangent_work_on_a_spline() {
    // The foot from above the top is the top.
    let found = all(
        &[cap()],
        p(5.3, 5.2),
        &only(SnapKind::Perpendicular),
        Some(p(5.0, 20.0)),
    );
    assert_eq!(found.len(), 1);
    assert!(near(found[0].point, p(5.0, 5.0)), "{:?}", found[0].point);
    // From (10, 10) the cap is touched at x = 10 - sqrt(50).
    let x = 10.0 - 50.0_f64.sqrt();
    let y = 2.0 * x - x * x / 5.0;
    let touch = all(
        &[cap()],
        p(x + 0.2, y + 0.1),
        &only(SnapKind::Tangent),
        Some(p(10.0, 10.0)),
    );
    assert_eq!(touch.len(), 1);
    assert!(near(touch[0].point, p(x, y)));
    // With no last point a spline offers the deferred snaps too.
    let deferred = all(&[cap()], p(5.0, 5.2), &only(SnapKind::Tangent), None);
    assert!(deferred.iter().all(|s| s.deferred));
    assert_eq!(deferred.len(), 1);
}

/// A block of one 10 unit line along the x axis, inserted at (0, 20).
fn block_with_a_line() -> Entity {
    let block = BlockDef {
        id: BlockId(1),
        name: "Bar".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        entities: vec![BlockEntity {
            entity: line(0.0, 0.0, 10.0, 0.0),
            attrs: Attributes::on_layer(LayerId::DEFAULT),
        }],
    };
    Entity::Insert(Box::new(Insert::new(Arc::new(block), p(0.0, 20.0))))
}

#[test]
fn what_a_block_draws_crosses_other_objects() {
    let upright = line(4.0, 15.0, 4.0, 25.0);
    let found = all(
        &[block_with_a_line(), upright],
        p(4.2, 20.1),
        &only(SnapKind::Intersection),
        None,
    );
    assert_eq!(found.len(), 1);
    assert!(near(found[0].point, p(4.0, 20.0)), "{:?}", found[0].point);
    // The block's own contents can be Nearest and Perpendicular targets too.
    let foot = all(
        &[block_with_a_line()],
        p(7.2, 20.1),
        &only(SnapKind::Perpendicular),
        Some(p(7.0, 30.0)),
    );
    assert_eq!(foot.len(), 1);
    assert!(near(foot[0].point, p(7.0, 20.0)));
}

#[test]
fn a_polyline_crosses_itself_but_not_at_its_corners() {
    // A bow tie left open: its second and fourth segments cross at (2, 2).
    let bow = polyline(&[(0.0, 0.0), (4.0, 4.0), (4.0, 0.0), (0.0, 4.0)], false);
    let modes = only(SnapKind::Intersection);
    let at_cross = all(std::slice::from_ref(&bow), p(2.1, 1.9), &modes, None);
    assert_eq!(at_cross.len(), 1);
    assert!(near(at_cross[0].point, p(2.0, 2.0)));
    // Its corners are its endpoints, not crossings.
    assert!(all(&[bow], p(4.1, 4.1), &modes, None).is_empty());
}

#[test]
fn the_extension_of_a_kept_end_meets_another_object() {
    // A line from (0, 0) to (10, 0) kept at its end; an upright at x = 25.
    let entities = [line(0.0, 0.0, 10.0, 0.0), line(25.0, -10.0, 25.0, 10.0)];
    let document = od_core::Document::from_entities(entities.iter().cloned());
    let key = Acquired::End {
        object: od_core::EntityId(0),
        last: true,
    };
    let track = resolve(&document, key).unwrap();
    let modes = only(SnapKind::ExtendedIntersection);
    let probe = Probe {
        cursor: p(25.3, 0.4),
        tolerance: 1.0,
        grid_step: None,
        modes: &modes,
        from: None,
        tracks: &[track],
        track: None,
    };
    let found = snap_all(document.objects(), &probe);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].kind, SnapKind::ExtendedIntersection);
    assert!(near(found[0].point, p(25.0, 0.0)));
    // Not a running mode: with it off, nothing, and a plain Intersection
    // (which wants the lines themselves to meet) finds none.
    let plain = SnapModes {
        extended_intersection: false,
        ..only(SnapKind::Intersection)
    };
    let probe = Probe {
        modes: &plain,
        ..probe
    };
    assert!(snap_all(document.objects(), &probe).is_empty());
}
