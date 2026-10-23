//! The snap engine as a whole: ranking, intersections, the points that need
//! the command's last point, and Tab cycling.

use od_core::{Entity, Point};

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

fn all(entities: &[Entity], cursor: Point, tolerance: f64, modes: &SnapModes) -> Vec<SnapPoint> {
    all_from(entities, cursor, tolerance, modes, None)
}

fn all_from(
    entities: &[Entity],
    cursor: Point,
    tolerance: f64,
    modes: &SnapModes,
    from: Option<Point>,
) -> Vec<SnapPoint> {
    let probe = Probe {
        cursor,
        tolerance,
        grid_step: None,
        modes,
        from,
        tracks: &[],
        track: None,
    };
    snap_all(&objs(entities), &probe)
}

/// Only `kind` among the running snaps, and no grid.
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

fn circle(x: f64, y: f64, r: f64) -> Entity {
    Entity::Circle {
        center: p(x, y),
        radius: r,
    }
}

#[test]
fn a_crossing_is_an_intersection_of_the_two_objects() {
    let entities = [line(0.0, 0.0, 10.0, 10.0), line(0.0, 10.0, 10.0, 0.0)];
    let found = all(&entities, p(5.2, 4.9), 1.0, &only(SnapKind::Intersection));
    assert_eq!(found.len(), 1);
    assert_eq!(
        (found[0].kind, found[0].point),
        (SnapKind::Intersection, p(5.0, 5.0))
    );
    assert_eq!(
        found[0].source,
        [Some(od_core::EntityId(0)), Some(od_core::EntityId(1))]
    );
    // Out of the aperture, nothing.
    assert!(all(&entities, p(7.0, 5.0), 1.0, &only(SnapKind::Intersection)).is_empty());
    // Switched off, nothing.
    let off = SnapModes {
        intersection: false,
        ..only(SnapKind::Intersection)
    };
    assert!(all(&entities, p(5.0, 5.0), 1.0, &off).is_empty());
}

#[test]
fn lines_that_do_not_reach_each_other_have_no_intersection() {
    let entities = [line(0.0, 0.0, 4.0, 4.0), line(0.0, 10.0, 10.0, 0.0)];
    // They would cross at (5, 5) if the first were longer.
    assert!(all(&entities, p(5.0, 5.0), 1.0, &only(SnapKind::Intersection)).is_empty());
}

#[test]
fn a_tangential_intersection_is_listed_once() {
    // The line touches the circle at (0, 5), where the two solutions meet.
    let entities = [line(-10.0, 5.0, 10.0, 5.0), circle(0.0, 0.0, 5.0)];
    let found = all(&entities, p(0.0, 5.0), 1.0, &only(SnapKind::Intersection));
    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn a_polylines_own_corners_are_not_intersections() {
    let corner = Entity::Polyline {
        points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
        closed: false,
        bulges: Vec::new(),
        widths: Vec::new(),
    };
    assert!(all(&[corner], p(10.0, 0.0), 1.0, &only(SnapKind::Intersection)).is_empty());
}

#[test]
fn coincident_snaps_are_listed_once_as_the_most_specific() {
    // Two lines meeting at their ends: Endpoint twice and Intersection.
    let entities = [line(0.0, 0.0, 10.0, 0.0), line(10.0, 0.0, 10.0, 10.0)];
    let found = all(&entities, p(10.0, 0.0), 1.0, &SnapModes::default());
    let here: Vec<_> = found.iter().filter(|f| f.point == p(10.0, 0.0)).collect();
    assert_eq!(here.len(), 1);
    assert_eq!(here[0].kind, SnapKind::Endpoint);
}

#[test]
fn points_in_the_aperture_come_first_then_nearest_then_the_grid() {
    let entities = [line(0.0, 0.0, 10.0, 0.0)];
    let modes = SnapModes {
        nearest: true,
        grid: true,
        ..SnapModes::default()
    };
    let probe = Probe {
        cursor: p(1.0, 0.5),
        tolerance: 3.0,
        grid_step: Some(2.0),
        modes: &modes,
        from: None,
        tracks: &[],
        track: None,
    };
    let found = snap_all(&objs(&entities), &probe);
    let kinds: Vec<_> = found.iter().map(|f| f.kind).collect();
    assert_eq!(
        kinds,
        [SnapKind::Endpoint, SnapKind::Nearest, SnapKind::Grid],
        "{found:?}"
    );
}

#[test]
fn nearest_never_hides_an_endpoint_even_when_it_is_closer() {
    // The pointer is nearer the line than its end, but the end is in reach.
    let entities = [line(0.0, 0.0, 10.0, 0.0)];
    let modes = SnapModes {
        nearest: true,
        midpoint: false,
        ..SnapModes::default()
    };
    let found = all(&entities, p(9.0, 0.1), 2.0, &modes);
    assert_eq!(found[0].kind, SnapKind::Endpoint);
    assert_eq!(found[1].kind, SnapKind::Nearest);
}

#[test]
fn perpendicular_with_a_last_point_needs_the_pointer_near_the_foot() {
    let entities = [line(0.0, 0.0, 100.0, 0.0)];
    let modes = only(SnapKind::Perpendicular);
    let from = Some(p(5.0, 30.0));
    // No last point: the snap is deferred, at the nearest point of the line.
    let deferred = all(&entities, p(5.2, 0.2), 1.0, &modes);
    assert_eq!(deferred.len(), 1);
    assert!(deferred[0].deferred);
    assert_eq!(deferred[0].point, p(5.2, 0.0));
    assert_eq!(deferred[0].label(), "Deferred Perpendicular");
    assert!(
        all(&entities, p(5.2, 5.0), 1.0, &modes).is_empty(),
        "off the line"
    );
    // Near the foot (5% along the line), it snaps there.
    let found = all_from(&entities, p(5.4, 0.3), 1.0, &modes, from);
    assert_eq!(found.len(), 1);
    assert_eq!(
        (found[0].kind, found[0].point),
        (SnapKind::Perpendicular, p(5.0, 0.0))
    );
    // Over the same line at 95%: no snap, though the foot exists.
    assert!(all_from(&entities, p(95.0, 0.2), 1.0, &modes, from).is_empty());
    // A foot beyond the line's end is no solution.
    let off = Some(p(120.0, 30.0));
    assert!(all_from(&entities, p(100.0, 0.2), 1.0, &modes, off).is_empty());
}

#[test]
fn tangent_has_zero_one_or_two_solutions() {
    let entities = [circle(0.0, 0.0, 5.0)];
    let modes = only(SnapKind::Tangent);
    // From (13, 0) the circle is touched at x = 25/13, y = ±(5 * 12 / 13).
    let (x, y) = (25.0 / 13.0, 60.0 / 13.0);
    let from = Some(p(13.0, 0.0));
    let upper = all_from(&entities, p(x, y), 1.0, &modes, from);
    let lower = all_from(&entities, p(x, -y), 1.0, &modes, from);
    assert_eq!((upper.len(), lower.len()), (1, 1), "one at each solution");
    assert!(upper[0].point.y > 0.0 && lower[0].point.y < 0.0);
    // Between the two, over the circle but at neither: none.
    assert!(all_from(&entities, p(5.0, 0.0), 1.0, &modes, from).is_empty());
    // From a point on the circle it is its own tangent point.
    let on = all_from(&entities, p(5.0, 0.0), 1.0, &modes, Some(p(5.0, 0.0)));
    assert_eq!(on.len(), 1);
    // From inside there is no solution.
    assert!(all_from(&entities, p(5.0, 0.0), 1.0, &modes, Some(p(1.0, 0.0))).is_empty());
}

#[test]
fn a_perpendicular_on_an_arc_is_kept_only_inside_its_sweep() {
    let arc = Entity::Arc {
        center: p(0.0, 0.0),
        radius: 5.0,
        start: -0.5,
        end: 0.5,
    };
    let modes = only(SnapKind::Perpendicular);
    let from = Some(p(10.0, 0.0));
    let found = all_from(std::slice::from_ref(&arc), p(5.0, 0.0), 1.0, &modes, from);
    assert_eq!(found.len(), 1);
    assert!(found[0].point.x > 0.0);
    // The far foot is off the arc: the pointer there finds nothing.
    assert!(all_from(std::slice::from_ref(&arc), p(-5.0, 0.0), 1.0, &modes, from).is_empty());
}

#[test]
fn dimensions_offer_nodes_not_insertions() {
    let style = od_core::DimStyle::default();
    let dim = od_core::Dimension::aligned(p(0.0, 0.0), p(20.0, 0.0), p(0.0, 6.0), &style);
    let entity = Entity::Dimension(Box::new(dim));
    let found = candidates(&entity);
    assert!(found.iter().any(|c| c.kind == SnapKind::Node));
    assert!(found.iter().all(|c| c.kind != SnapKind::Insertion));
    let nodes = all(&[entity], p(0.0, 0.0), 1.0, &only(SnapKind::Node));
    assert_eq!(nodes.len(), 1);
}

#[test]
fn a_ray_offers_its_base_as_an_endpoint_and_a_construction_line_offers_none() {
    let origin = p(2.0, 3.0);
    let direction = p(1.0, 0.0);
    let ray = Entity::Ray { origin, direction };
    let found = all(&[ray], p(2.1, 3.0), 1.0, &only(SnapKind::Endpoint));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].point, origin);
    let xline = Entity::Xline { origin, direction };
    assert!(all(&[xline], p(2.1, 3.0), 1.0, &only(SnapKind::Endpoint)).is_empty());
}

#[test]
fn a_point_object_offers_a_node() {
    let point = Entity::Point { at: p(3.0, 4.0) };
    let found = all(&[point], p(3.1, 4.0), 1.0, &only(SnapKind::Node));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].point, p(3.0, 4.0));
}

#[test]
fn the_same_pixels_reach_less_of_the_drawing_when_zoomed_in() {
    // The aperture arrives as a radius in drawing units: 8 px at zoom 2 is
    // 4 units, at zoom 4 it is 2.
    let entities = [line(0.0, 0.0, 10.0, 0.0)];
    let near = p(10.0, 3.0);
    assert!(!all(&entities, near, 8.0 / 2.0, &SnapModes::default()).is_empty());
    assert!(all(&entities, near, 8.0 / 4.0, &SnapModes::default()).is_empty());
}

#[test]
fn an_id_survives_small_pointer_moves_but_not_a_change_of_point() {
    let entities = [line(0.0, 0.0, 10.0, 0.0)];
    let modes = SnapModes::default();
    let a = all(&entities, p(9.8, 0.1), 2.0, &modes);
    let b = all(&entities, p(10.1, -0.1), 2.0, &modes);
    assert_eq!(a[0].id(), b[0].id());
    assert_ne!(a[0].id(), all(&entities, p(0.1, 0.0), 2.0, &modes)[0].id());
}

#[test]
fn tab_walks_the_candidates_and_wraps() {
    let entities = [line(0.0, 0.0, 2.0, 0.0), circle(1.0, 0.0, 1.0)];
    let modes = SnapModes {
        quadrant: true,
        ..SnapModes::default()
    };
    let found = all(&entities, p(1.0, 0.0), 3.0, &modes);
    assert!(found.len() >= 3, "{found:?}");
    let ids: Vec<_> = found.iter().map(SnapPoint::id).collect();
    // Nothing chosen: the first is shown, so Tab moves to the second.
    assert_eq!(cycled(&found, None, 1), Some(ids[1]));
    let last = ids.len() - 1;
    assert_eq!(cycled(&found, Some(ids[last]), 1), Some(ids[0]), "wraps");
    assert_eq!(cycled(&found, Some(ids[0]), -1), Some(ids[last]), "back");
    // No press keeps the choice while it is listed, and drops it when not.
    assert_eq!(cycled(&found, Some(ids[1]), 0), Some(ids[1]));
    let gone = all(&entities, p(50.0, 50.0), 3.0, &modes);
    assert_eq!(cycled(&gone, Some(ids[1]), 0), None);
    assert_eq!(cycled(&gone, Some(ids[1]), 1), None);
    assert_eq!(cycled(&found, None, 0), None);
}

#[test]
fn a_dense_patch_stays_bounded() {
    // Hundreds of crossing lines under one aperture still answer.
    let entities: Vec<Entity> = (0..300)
        .map(|i| {
            let a = f64::from(i) * 0.01;
            line(-10.0, -10.0 * a.tan(), 10.0, 10.0 * a.tan())
        })
        .collect();
    let found = all(&entities, p(0.0, 0.0), 1.0, &SnapModes::default());
    assert!(!found.is_empty());
}

#[test]
fn nearest_works_on_every_kind_of_curve() {
    let entities = [
        line(0.0, 0.0, 10.0, 0.0),
        circle(0.0, 0.0, 5.0),
        Entity::Arc {
            center: p(0.0, 0.0),
            radius: 5.0,
            start: 0.0,
            end: 1.5,
        },
        Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(10.0, 6.0),
        },
        Entity::Polyline {
            points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        },
        Entity::Ellipse(od_core::Ellipse::full(p(0.0, 0.0), p(10.0, 0.0), 0.5).unwrap()),
    ];
    let modes = only(SnapKind::Nearest);
    for e in entities {
        let on = e.nearest_point(p(3.3, 1.7)).unwrap();
        let cursor = p(on.x + 0.1, on.y + 0.1);
        let found = all(std::slice::from_ref(&e), cursor, 1.0, &modes);
        assert!(!found.is_empty(), "{e:?}");
        assert_eq!(found[0].kind, SnapKind::Nearest);
    }
}

#[test]
fn tangent_is_deferred_only_on_curves() {
    let modes = only(SnapKind::Tangent);
    let found = all(&[circle(0.0, 0.0, 5.0)], p(5.0, 0.3), 1.0, &modes);
    assert_eq!(found.len(), 1);
    assert!(found[0].deferred && found[0].kind == SnapKind::Tangent);
    assert_eq!(found[0].label(), "Deferred Tangent");
    // A straight line has no tangent to a point.
    assert!(all(&[line(0.0, 0.0, 10.0, 0.0)], p(5.0, 0.2), 1.0, &modes).is_empty());
}

#[test]
fn a_snap_with_a_last_point_is_never_deferred() {
    let entities = [line(0.0, 0.0, 100.0, 0.0)];
    let modes = only(SnapKind::Perpendicular);
    let found = all_from(&entities, p(5.4, 0.3), 1.0, &modes, Some(p(5.0, 30.0)));
    assert!(!found[0].deferred);
    assert_eq!(found[0].label(), "Perpendicular");
}
