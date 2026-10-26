//! Hit-testing: which entity is under a point.

use od_core::{arc, Bounds, Document, Entity, EntityId, HatchEntity, Object, Point};

fn distance(a: Point, b: Point) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

pub(crate) fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len_sq = dx * dx + dy * dy;
    if len_sq == 0.0 {
        return distance(p, a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0);
    distance(p, Point::new(a.x + t * dx, a.y + t * dy))
}

/// A thousandth of a hatch's size: how closely its curves are followed
/// when picking and selecting.
pub(crate) fn hatch_tolerance(h: &HatchEntity) -> f64 {
    h.bounds().map_or(1e-9, |b| {
        ((b.max.x - b.min.x).hypot(b.max.y - b.min.y) * 1e-3).max(1e-9)
    })
}

/// The loops of a hatch as polygons, or none if they cannot be followed.
pub(crate) fn hatch_contours(h: &HatchEntity) -> Vec<Vec<Point>> {
    h.contours(hatch_tolerance(h)).unwrap_or_default()
}

/// A click inside a hatch is this fraction of the pick radius away from it:
/// near enough to pick it, but anything nearer wins.
const INSIDE_HATCH: f64 = 0.999;

/// [`distance_to_entity`] for a stored object, which for a dimension uses the
/// picture the object remembers instead of building it again.
pub fn distance_to_object(object: &Object, p: Point) -> f64 {
    match object.dimension_picture() {
        Some(pieces) => pieces
            .iter()
            .map(|piece| distance_to_entity(piece, p))
            .fold(f64::INFINITY, f64::min),
        None => distance_to_entity(&object.entity, p),
    }
}

/// Distance from `p` to the entity's outline (not its interior).
pub fn distance_to_entity(entity: &Entity, p: Point) -> f64 {
    match entity {
        // Its boundary: the strokes of a pattern are not picked one by one.
        Entity::Hatch(h) => h
            .distance_to_boundary(p, hatch_tolerance(h))
            .unwrap_or(f64::INFINITY),
        // Its lines, arrows and text strokes are what can be picked.
        Entity::Dimension(d) => d
            .picture()
            .iter()
            .map(|piece| distance_to_entity(piece, p))
            .fold(f64::INFINITY, f64::min),
        Entity::Leader(l) => l
            .picture()
            .iter()
            .map(|piece| distance_to_entity(piece, p))
            .fold(f64::INFINITY, f64::min),
        Entity::MLeader(m) => m
            .picture()
            .iter()
            .map(|piece| distance_to_entity(piece, p))
            .fold(f64::INFINITY, f64::min),
        // A block is picked by what it draws.
        Entity::Insert(i) => i
            .picture()
            .iter()
            .map(|piece| distance_to_entity(piece, p))
            .fold(f64::INFINITY, f64::min),
        // The area the text fills stands for the glyphs; empty space in a
        // bigger box is not picked, so what is drawn there still can be.
        Entity::MText(m) if m.text_contains(p) => 0.0,
        Entity::MText(m) => {
            let corners = m.text_frame();
            (0..4)
                .map(|i| distance_to_segment(p, corners[i], corners[(i + 1) % 4]))
                .fold(f64::INFINITY, f64::min)
        }
        Entity::Point { at } => distance(p, *at),
        Entity::Xline { origin, direction } => {
            od_core::infinite::distance_to(*origin, *direction, false, p)
        }
        Entity::Ray { origin, direction } => {
            od_core::infinite::distance_to(*origin, *direction, true, p)
        }
        Entity::Line { start, end } => distance_to_segment(p, *start, *end),
        Entity::Circle { center, radius } => (distance(p, *center) - radius.abs()).abs(),
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => arc::distance_to(*center, *radius, *start, *end, p),
        Entity::Ellipse(e) => e.distance_to(p),
        Entity::Spline(s) => s.distance_to(p),
        Entity::Rectangle { min, max } => {
            let corners = [
                Point::new(min.x, min.y),
                Point::new(max.x, min.y),
                Point::new(max.x, max.y),
                Point::new(min.x, max.y),
            ];
            (0..4)
                .map(|i| distance_to_segment(p, corners[i], corners[(i + 1) % 4]))
                .fold(f64::INFINITY, f64::min)
        }
        // The exact distance, arcs included, only when there are arcs: the
        // straight case is the hot one when picking.
        Entity::Polyline { bulges, .. } if !bulges.is_empty() => entity.distance_to(p),
        Entity::Polyline { points, closed, .. } => segments(points, *closed)
            .map(|(a, b)| distance_to_segment(p, a, b))
            .fold(f64::INFINITY, f64::min),
    }
}

/// The segments of a polyline, including the closing one when `closed`.
pub(crate) fn segments(
    points: &[Point],
    closed: bool,
) -> impl Iterator<Item = (Point, Point)> + '_ {
    let wrap = (closed && points.len() > 2).then(|| (points[points.len() - 1], points[0]));
    points.windows(2).map(|w| (w[0], w[1])).chain(wrap)
}

/// The square `tolerance` around `p`: the part of the drawing that can hold
/// an answer for [`pick`] or `snap`, so the index need only name its objects.
pub(crate) fn probe(p: Point, tolerance: f64) -> Bounds {
    Bounds::from_points(
        Point::new(p.x - tolerance, p.y - tolerance),
        Point::new(p.x + tolerance, p.y + tolerance),
    )
}

/// [`pick`] among the selectable objects of the space being edited, asking
/// the document's index which are near enough to be worth testing.
pub(crate) fn pick_here(document: &Document, p: Point, tolerance: f64) -> Option<EntityId> {
    pick(
        document.selectable_here_in_rect(probe(p, tolerance)),
        p,
        tolerance,
    )
}

/// [`pick_here`] among every visible object, those on locked layers too.
pub(crate) fn pick_visible_here(document: &Document, p: Point, tolerance: f64) -> Option<EntityId> {
    pick(
        document.visible_here_in_rect(probe(p, tolerance)),
        p,
        tolerance,
    )
}

/// The entity nearest `p` within `tolerance` drawing units. When two are
/// equally near, the later (topmost) one wins.
pub fn pick<'a>(
    objects: impl IntoIterator<Item = &'a Object>,
    p: Point,
    tolerance: f64,
) -> Option<EntityId> {
    let probe = probe(p, tolerance);
    let mut best: Option<(EntityId, f64)> = None;
    for object in objects {
        if !object.bounds().intersects(&probe) {
            continue;
        }
        let Object { id, entity, .. } = object;
        let d = match entity {
            Entity::Hatch(h) if h.contains(p, hatch_tolerance(h)) => tolerance * INSIDE_HATCH,
            _ => distance_to_object(object, p),
        };
        if d <= tolerance && best.is_none_or(|(_, best_d)| d <= best_d) {
            best = Some((*id, d));
        }
    }
    best.map(|(id, _)| id)
}

#[cfg(test)]
mod tests {
    /// Picking asks every object for its box. A dimension's box used to mean
    /// building everything it draws, 3.8 us each in a release build, so 200
    /// hovers over 3,000 of them took over two seconds even optimised and many
    /// times that in a debug build. Remembered, the same hovers take a few
    /// milliseconds, so the limit is loose.
    #[test]
    fn picking_over_a_great_many_dimensions_scales() {
        let style = od_core::DimStyle::default();
        let doc = od_core::Document::from_entities((0..3_000).map(|i| {
            let x = f64::from(i) * 50.0;
            Entity::Dimension(Box::new(od_core::Dimension::aligned(
                Point::new(x, 0.0),
                Point::new(x + 40.0, 0.0),
                Point::new(x, 10.0),
                &style,
            )))
        }));
        doc.bounds(); // the first look at every object
        let started = std::time::Instant::now();
        let mut hits = 0;
        for i in 0..200 {
            // Just beside an extension line, so each pick finds something.
            let at = Point::new(1.0 + f64::from(i) * 700.0, 3.0);
            hits += usize::from(pick(doc.objects(), at, 6.0).is_some());
        }
        assert!(hits > 0);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "took {:?}",
            started.elapsed()
        );
    }

    use super::*;
    use crate::testing::objs;

    fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> Entity {
        Entity::Line {
            start: Point::new(x1, y1),
            end: Point::new(x2, y2),
        }
    }

    #[test]
    fn line_distance_is_to_the_segment_not_the_infinite_line() {
        let l = line(0.0, 0.0, 10.0, 0.0);
        assert_eq!(distance_to_entity(&l, Point::new(5.0, 3.0)), 3.0);
        assert_eq!(distance_to_entity(&l, Point::new(13.0, 4.0)), 5.0);
    }

    #[test]
    fn degenerate_line_is_a_point() {
        let l = line(2.0, 2.0, 2.0, 2.0);
        assert_eq!(distance_to_entity(&l, Point::new(5.0, 6.0)), 5.0);
    }

    #[test]
    fn circle_distance_is_to_the_outline() {
        let c = Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 10.0,
        };
        assert!((distance_to_entity(&c, Point::new(13.0, 0.0)) - 3.0).abs() < 1e-9);
        assert!((distance_to_entity(&c, Point::new(0.0, 0.0)) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn an_arc_is_hit_along_its_curve_only() {
        // The upper half of a circle of radius 10.
        let a = Entity::Arc {
            center: Point::new(0.0, 0.0),
            radius: 10.0,
            start: 0.0,
            end: std::f64::consts::PI,
        };
        assert!((distance_to_entity(&a, Point::new(0.0, 12.0)) - 2.0).abs() < 1e-9);
        // On the missing half the circle is not there to be hit.
        assert!(distance_to_entity(&a, Point::new(0.0, -10.0)) > 10.0);
    }

    #[test]
    fn a_spline_is_hit_along_its_curve_not_its_control_frame() {
        let p = Point::new;
        let points = vec![p(0.0, 0.0), p(5.0, 5.0), p(10.0, 0.0)];
        let s = od_core::Spline::from_fit_points(points, od_core::KnotParameterization::Chord);
        let s = s.unwrap();
        let frame = s.control_points.clone();
        let e = Entity::Spline(Box::new(s));
        // It passes through its fit points, and is level at the top.
        assert!(distance_to_entity(&e, p(5.0, 5.0)) < 1e-9);
        assert!((distance_to_entity(&e, p(5.0, 7.0)) - 2.0).abs() < 1e-6);
        // A control point off the curve is not on it.
        assert!(frame.iter().any(|q| distance_to_entity(&e, *q) > 0.5));
    }

    #[test]
    fn an_ellipse_is_hit_along_its_curve_only() {
        // The upper half of a 20-by-10 ellipse.
        let e = od_core::Ellipse::new(
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            0.5,
            0.0,
            std::f64::consts::PI,
        );
        let e = Entity::Ellipse(e.unwrap());
        assert!((distance_to_entity(&e, Point::new(0.0, 7.0)) - 2.0).abs() < 1e-9);
        assert!((distance_to_entity(&e, Point::new(12.0, 0.0)) - 2.0).abs() < 1e-9);
        // Below, the ends (12.8 away) are nearer than the top (13 away); the
        // missing lower half (3 away) is not there to be hit.
        let below = distance_to_entity(&e, Point::new(0.0, -8.0));
        assert!((below - 164f64.sqrt()).abs() < 1e-9, "{below}");
    }

    #[test]
    fn rectangle_distance_is_to_the_edges() {
        let r = Entity::Rectangle {
            min: Point::new(0.0, 0.0),
            max: Point::new(10.0, 10.0),
        };
        assert_eq!(distance_to_entity(&r, Point::new(5.0, 2.0)), 2.0);
        assert_eq!(distance_to_entity(&r, Point::new(5.0, 12.0)), 2.0);
    }

    #[test]
    fn a_point_is_hit_only_near_the_point_itself() {
        let entities = [Entity::Point {
            at: Point::new(2.0, 2.0),
        }];
        assert_eq!(distance_to_entity(&entities[0], Point::new(5.0, 6.0)), 5.0);
        assert_eq!(
            pick(&objs(&entities), Point::new(2.2, 2.0), 0.5),
            Some(EntityId(0))
        );
        assert_eq!(pick(&objs(&entities), Point::new(3.0, 2.0), 0.5), None);
    }

    #[test]
    fn a_construction_line_is_hit_far_from_its_base_and_a_ray_only_ahead() {
        let xline = Entity::Xline {
            origin: Point::new(0.0, 0.0),
            direction: Point::new(1.0, 0.0),
        };
        let ray = Entity::Ray {
            origin: Point::new(0.0, 0.0),
            direction: Point::new(1.0, 0.0),
        };
        let far_behind = Point::new(-1.0e5, 0.2);
        assert!((distance_to_entity(&xline, far_behind) - 0.2).abs() < 1e-9);
        assert!(distance_to_entity(&ray, far_behind) > 1.0e4);
        let entities = [xline];
        assert_eq!(
            pick(&objs(&entities), Point::new(5.0e6, 0.1), 0.5),
            Some(EntityId(0))
        );
    }

    #[test]
    fn pick_returns_none_outside_tolerance() {
        let entities = [line(0.0, 0.0, 10.0, 0.0)];
        assert_eq!(pick(&objs(&entities), Point::new(5.0, 2.0), 1.0), None);
        assert_eq!(
            pick(&objs(&entities), Point::new(5.0, 0.5), 1.0),
            Some(EntityId(0))
        );
    }

    #[test]
    fn pick_prefers_nearest_then_topmost() {
        let entities = [line(0.0, 0.0, 10.0, 0.0), line(0.0, 1.0, 10.0, 1.0)];
        // Nearer to the first line.
        assert_eq!(
            pick(&objs(&entities), Point::new(5.0, 0.2), 2.0),
            Some(EntityId(0))
        );
        // Equidistant: later entity wins.
        assert_eq!(
            pick(&objs(&entities), Point::new(5.0, 0.5), 2.0),
            Some(EntityId(1))
        );
    }

    fn polyline(closed: bool) -> Entity {
        Entity::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(10.0, 10.0),
            ],
            closed,
            bulges: Vec::new(),
            widths: Vec::new(),
        }
    }

    #[test]
    fn polyline_is_hit_along_its_segments() {
        let open = polyline(false);
        assert!(distance_to_entity(&open, Point::new(5.0, 0.5)) < 1e-9 + 0.5);
        assert!(distance_to_entity(&open, Point::new(10.0, 5.0)) < 1e-9);
        // Only a closed polyline has the segment from the last point home.
        let across = Point::new(5.0, 5.0);
        assert!(distance_to_entity(&open, across) > 3.0);
        assert!(distance_to_entity(&polyline(true), across) < 1e-9);
    }

    fn square_hatch(min: f64, max: f64) -> Entity {
        use od_core::hatch::{
            HatchBoundary, HatchEntity, HatchFill, HatchLoop, HatchLoopFlags, HatchPolyline,
        };
        let p = Point::new;
        Entity::Hatch(Box::new(HatchEntity::new(
            vec![HatchLoop {
                flags: HatchLoopFlags::default(),
                boundary: HatchBoundary::Polyline(HatchPolyline {
                    vertices: vec![p(min, min), p(max, min), p(max, max), p(min, max)],
                    bulges: Vec::new(),
                    closed: true,
                }),
                sources: Vec::new(),
            }],
            HatchFill::Solid,
        )))
    }

    #[test]
    fn a_hatch_is_picked_anywhere_inside_but_anything_nearer_wins() {
        let h = square_hatch(0.0, 100.0);
        // The distance is to the boundary...
        assert!((distance_to_entity(&h, Point::new(50.0, 50.0)) - 50.0).abs() < 1e-6);
        // ...but a click inside picks it, as long as nothing is nearer.
        let alone = [h.clone()];
        assert_eq!(
            pick(&objs(&alone), Point::new(50.0, 50.0), 3.0),
            Some(EntityId(0))
        );
        assert_eq!(pick(&objs(&alone), Point::new(150.0, 50.0), 3.0), None);
        assert_eq!(
            pick(&objs(&alone), Point::new(101.0, 50.0), 3.0),
            Some(EntityId(0))
        );
        // A line inside the hatch beats it, whichever is on top.
        for entities in [
            [h.clone(), line(20.0, 50.0, 80.0, 50.0)],
            [line(20.0, 50.0, 80.0, 50.0), h.clone()],
        ] {
            let at_line = pick(&objs(&entities), Point::new(50.0, 51.0), 3.0);
            let line_id = entities
                .iter()
                .position(|e| matches!(e, Entity::Line { .. }));
            assert_eq!(at_line, line_id.map(|i| EntityId(i as u64)));
        }
    }

    #[test]
    fn a_click_in_an_island_is_not_inside_the_hatch() {
        use od_core::hatch::HatchEntity;
        let (Entity::Hatch(outer), Entity::Hatch(inner)) =
            (square_hatch(0.0, 100.0), square_hatch(40.0, 60.0))
        else {
            panic!("hatches");
        };
        let mut both: HatchEntity = (*outer).clone();
        both.loops.extend(inner.loops.clone());
        let entities = [Entity::Hatch(Box::new(both))];
        assert_eq!(
            pick(&objs(&entities), Point::new(10.0, 50.0), 3.0),
            Some(EntityId(0))
        );
        // In the island, 10 from its edge: out of reach.
        assert_eq!(pick(&objs(&entities), Point::new(50.0, 50.0), 3.0), None);
    }
}
