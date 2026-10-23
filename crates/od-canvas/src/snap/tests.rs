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

#[test]
fn snaps_to_a_nearby_endpoint() {
    let entities = [line(0.0, 0.0, 100.0, 0.0)];
    let s = snap(
        &objs(&entities),
        p(101.0, 2.0),
        5.0,
        None,
        &SnapModes::default(),
    )
    .unwrap();
    assert_eq!(s.kind, SnapKind::Endpoint);
    assert_eq!(s.point, p(100.0, 0.0));
}

#[test]
fn snaps_to_midpoint_and_center() {
    let entities = [
        line(0.0, 0.0, 100.0, 0.0),
        Entity::Circle {
            center: p(50.0, 50.0),
            radius: 10.0,
        },
    ];
    let modes = SnapModes::default();
    let m = snap(&objs(&entities), p(49.0, 1.0), 5.0, None, &modes).unwrap();
    assert_eq!((m.kind, m.point), (SnapKind::Midpoint, p(50.0, 0.0)));
    let c = snap(&objs(&entities), p(52.0, 49.0), 5.0, None, &modes).unwrap();
    assert_eq!((c.kind, c.point), (SnapKind::Center, p(50.0, 50.0)));
}

#[test]
fn rectangle_offers_corners_edge_midpoints_and_center() {
    let r = Entity::Rectangle {
        min: p(0.0, 0.0),
        max: p(10.0, 6.0),
    };
    let kinds: Vec<_> = candidates(&r).iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds.iter().filter(|k| **k == SnapKind::Endpoint).count(),
        4
    );
    assert_eq!(
        kinds.iter().filter(|k| **k == SnapKind::Midpoint).count(),
        4
    );
    assert_eq!(kinds.iter().filter(|k| **k == SnapKind::Center).count(), 1);
}

#[test]
fn nearest_candidate_wins_and_ties_prefer_endpoint() {
    // Short line: midpoint 1 away from each end; cursor exactly at the
    // midpoint is nearer to it than to either end.
    let entities = [line(0.0, 0.0, 2.0, 0.0)];
    let s = snap(
        &objs(&entities),
        p(1.0, 0.0),
        5.0,
        None,
        &SnapModes::default(),
    )
    .unwrap();
    assert_eq!(s.kind, SnapKind::Midpoint);
    // Coincident endpoint and center at the same spot: endpoint wins.
    let entities = [
        line(5.0, 5.0, 9.0, 9.0),
        Entity::Circle {
            center: p(5.0, 5.0),
            radius: 1.0,
        },
    ];
    let s = snap(
        &objs(&entities),
        p(5.0, 5.0),
        3.0,
        None,
        &SnapModes::default(),
    )
    .unwrap();
    assert_eq!(s.kind, SnapKind::Endpoint);
}

#[test]
fn object_snaps_beat_the_grid_and_grid_is_the_fallback() {
    let entities = [line(3.0, 3.0, 50.0, 3.0)];
    let modes = SnapModes::default();
    let near = snap(&objs(&entities), p(3.4, 3.2), 2.0, Some(10.0), &modes).unwrap();
    assert_eq!(near.kind, SnapKind::Endpoint);
    let far = snap(&objs(&entities), p(27.0, 33.0), 2.0, Some(10.0), &modes).unwrap();
    assert_eq!((far.kind, far.point), (SnapKind::Grid, p(30.0, 30.0)));
}

#[test]
fn modes_can_be_switched_off_individually_or_together() {
    let entities = [line(0.0, 0.0, 100.0, 0.0)];
    let no_endpoint = SnapModes {
        endpoint: false,
        ..SnapModes::default()
    };
    let s = snap(&objs(&entities), p(1.0, 1.0), 5.0, Some(10.0), &no_endpoint).unwrap();
    assert_eq!(s.kind, SnapKind::Grid);

    let off = SnapModes {
        enabled: false,
        ..SnapModes::default()
    };
    assert_eq!(
        snap(&objs(&entities), p(1.0, 1.0), 5.0, Some(10.0), &off),
        None
    );

    let no_grid = SnapModes {
        grid: false,
        ..SnapModes::default()
    };
    assert_eq!(snap(&[], p(1.0, 1.0), 5.0, Some(10.0), &no_grid), None);
}

#[test]
fn grid_rounds_negative_coordinates_and_ignores_bad_steps() {
    let modes = SnapModes::default();
    let s = snap(&[], p(-14.0, 26.0), 1.0, Some(10.0), &modes).unwrap();
    assert_eq!(s.point, p(-10.0, 30.0));
    assert_eq!(snap(&[], p(1.0, 1.0), 1.0, Some(0.0), &modes), None);
    assert_eq!(snap(&[], p(1.0, 1.0), 1.0, Some(f64::NAN), &modes), None);
}

#[test]
fn a_spline_offers_its_ends_and_the_point_halfway_along_it() {
    let open = od_core::Spline::from_control_points(1, vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)]);
    let found = candidates(&Entity::Spline(Box::new(open.unwrap())));
    let of = |kind| -> Vec<Point> {
        found
            .iter()
            .filter(|c| c.kind == kind)
            .map(|c| c.point)
            .collect()
    };
    assert_eq!(of(SnapKind::Endpoint), [p(0.0, 0.0), p(4.0, 3.0)]);
    // Half of 7 along it, not the middle of its parameters.
    let [mid] = of(SnapKind::Midpoint)[..] else {
        panic!("one midpoint")
    };
    assert!((mid.x - 3.5).abs() < 1e-9 && mid.y.abs() < 1e-9, "{mid:?}");
    assert!(of(SnapKind::Center).is_empty() && of(SnapKind::Quadrant).is_empty());
    let closed = od_core::Spline::new_nurbs(
        1,
        vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0), p(0.0, 0.0)],
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0],
        None,
        true,
        false,
    );
    let found = candidates(&Entity::Spline(Box::new(closed.unwrap())));
    assert!(
        found.iter().all(|c| c.kind == SnapKind::Midpoint),
        "no ends"
    );
}

#[test]
fn an_ellipse_offers_its_centre_and_an_arc_its_ends_and_middle() {
    use std::f64::consts::{PI, TAU};
    let ellipse = |end: f64| {
        Entity::Ellipse(od_core::Ellipse::new(p(0.0, 0.0), p(10.0, 0.0), 0.5, 0.0, end).unwrap())
    };
    let full = candidates(&ellipse(TAU));
    assert_eq!(full.len(), 5, "the centre and four quadrants");
    assert_eq!(
        (full[0].point, full[0].kind),
        (p(0.0, 0.0), SnapKind::Center)
    );
    let quadrants: Vec<Point> = full[1..].iter().map(|c| c.point).collect();
    for q in [p(10.0, 0.0), p(0.0, 5.0), p(-10.0, 0.0), p(0.0, -5.0)] {
        assert!(
            quadrants
                .iter()
                .any(|f| (f.x - q.x).hypot(f.y - q.y) < 1e-9),
            "{q:?}"
        );
    }
    let half = candidates(&ellipse(PI));
    let of = |kind| -> Vec<Point> {
        half.iter()
            .filter(|c| c.kind == kind)
            .map(|c| c.point)
            .collect()
    };
    let ends = of(SnapKind::Endpoint);
    assert!((ends[0].x - 10.0).abs() < 1e-9 && (ends[1].x + 10.0).abs() < 1e-9);
    let mid = of(SnapKind::Midpoint)[0];
    assert!(mid.x.abs() < 1e-9 && (mid.y - 5.0).abs() < 1e-9);
    assert_eq!(of(SnapKind::Center), [p(0.0, 0.0)]);
}

#[test]
fn quadrants_are_where_the_curve_reaches_them() {
    use std::f64::consts::FRAC_PI_2;
    let quadrants = |e: &Entity| -> Vec<Point> {
        candidates(e)
            .into_iter()
            .filter(|c| c.kind == SnapKind::Quadrant)
            .map(|c| c.point)
            .collect()
    };
    let circle = Entity::Circle {
        center: p(1.0, 1.0),
        radius: 2.0,
    };
    let found = quadrants(&circle);
    let want = [p(3.0, 1.0), p(1.0, 3.0), p(-1.0, 1.0), p(1.0, -1.0)];
    assert_eq!(found.len(), 4);
    for (f, w) in found.iter().zip(want) {
        assert!((f.x - w.x).hypot(f.y - w.y) < 1e-9, "{f:?} {w:?}");
    }
    // A quarter arc from north-east round to north-west passes north only.
    let arc = Entity::Arc {
        center: p(0.0, 0.0),
        radius: 2.0,
        start: FRAC_PI_2 / 2.0,
        end: 3.0 * FRAC_PI_2 / 2.0,
    };
    let found = quadrants(&arc);
    assert_eq!(found.len(), 1);
    assert!(found[0].x.abs() < 1e-9 && (found[0].y - 2.0).abs() < 1e-9);
}

#[test]
fn nearest_snaps_onto_a_curve_only_when_nothing_else_is_near() {
    let circle = Entity::Circle {
        center: p(0.0, 0.0),
        radius: 10.0,
    };
    let entities = [circle];
    let off = SnapModes::default();
    let on = SnapModes {
        nearest: true,
        grid: false,
        ..SnapModes::default()
    };
    // Off by default: nothing near the outline away from snap points.
    let cursor = p(6.0, 8.5);
    assert_eq!(snap(&objs(&entities), cursor, 1.0, None, &off), None);
    let s = snap(&objs(&entities), cursor, 1.0, None, &on).unwrap();
    assert_eq!(s.kind, SnapKind::Nearest);
    assert!((s.point.x.hypot(s.point.y) - 10.0).abs() < 1e-9);
    // A quadrant in reach wins over the nearest point.
    let quadrant = SnapModes {
        quadrant: true,
        ..on
    };
    let s = snap(&objs(&entities), p(0.5, 10.3), 1.0, None, &quadrant).unwrap();
    assert_eq!(s.kind, SnapKind::Quadrant);
    assert!(s.point.x.abs() < 1e-9 && (s.point.y - 10.0).abs() < 1e-9);
}

#[test]
fn an_arc_offers_its_ends_its_middle_and_its_centre() {
    let quarter = Entity::Arc {
        center: p(0.0, 0.0),
        radius: 10.0,
        start: 0.0,
        end: std::f64::consts::FRAC_PI_2,
    };
    let found = candidates(&quarter);
    let at =
        |kind| -> Vec<SnapPoint> { found.iter().filter(|c| c.kind == kind).copied().collect() };
    let ends = at(SnapKind::Endpoint);
    assert_eq!(ends.len(), 2);
    assert!((ends[0].point.x - 10.0).abs() < 1e-9 && ends[0].point.y.abs() < 1e-9);
    assert!(ends[1].point.x.abs() < 1e-9 && (ends[1].point.y - 10.0).abs() < 1e-9);
    let mid = at(SnapKind::Midpoint)[0].point;
    let s = 10.0 / 2.0_f64.sqrt();
    assert!((mid.x - s).abs() < 1e-9 && (mid.y - s).abs() < 1e-9);
    assert_eq!(at(SnapKind::Center)[0].point, p(0.0, 0.0));
}

#[test]
fn polyline_offers_vertices_and_segment_midpoints() {
    let open = Entity::Polyline {
        points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
        closed: false,
        bulges: Vec::new(),
        widths: Vec::new(),
    };
    let found = candidates(&open);
    let count = |kind| found.iter().filter(|c| c.kind == kind).count();
    assert_eq!(
        (count(SnapKind::Endpoint), count(SnapKind::Midpoint)),
        (3, 2)
    );
    let closed = Entity::Polyline {
        points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
        closed: true,
        bulges: Vec::new(),
        widths: Vec::new(),
    };
    let found = candidates(&closed);
    assert_eq!(
        found
            .iter()
            .filter(|c| c.kind == SnapKind::Midpoint)
            .count(),
        3
    );
}
