//! Region selection in the AutoCAD style: window, crossing and fence, with
//! rectangles or polygons. Pure geometry in world space.
//!
//! - **Window**: entities entirely inside the region.
//! - **Crossing**: entities inside the region or touching its boundary.
//! - **Fence**: entities touched by an open polyline.
//!
//! A drag rectangle is just a four-corner polygon.

use od_core::{arc, bulge, Bounds, Document, Entity, EntityId, Object, Point};

use crate::hit::{distance_to_segment, hatch_contours, segments};

#[derive(Clone, Debug, PartialEq)]
pub enum Region {
    /// Closed polygon (vertices only; the last edge closes it).
    Window(Vec<Point>),
    Crossing(Vec<Point>),
    /// Open polyline.
    Fence(Vec<Point>),
}

impl Region {
    /// A rectangle between two opposite corners.
    pub fn rect_corners(a: Point, b: Point) -> Vec<Point> {
        vec![
            Point::new(a.x, a.y),
            Point::new(b.x, a.y),
            Point::new(b.x, b.y),
            Point::new(a.x, b.y),
        ]
    }

    /// Whether a point is inside a polygon region (never, for a fence).
    pub fn contains(&self, p: Point) -> bool {
        match self {
            Region::Window(points) | Region::Crossing(points) => {
                points.len() >= 3 && point_in_polygon(p, points)
            }
            Region::Fence(_) => false,
        }
    }

    /// The box around the region's points, which holds everything the region
    /// can select; `None` for a region with no points.
    pub fn envelope(&self) -> Option<Bounds> {
        let (Region::Window(points) | Region::Crossing(points) | Region::Fence(points)) = self;
        let first = points.first()?;
        let start = Bounds::from_points(*first, *first);
        Some(
            points
                .iter()
                .fold(start, |b, p| b.union(Bounds::from_points(*p, *p))),
        )
    }

    /// Fewest vertices that make a usable region.
    pub fn min_points(&self) -> usize {
        match self {
            Region::Fence(_) => 2,
            _ => 3,
        }
    }
}

type Edge = (Point, Point);

fn closed_edges(points: &[Point]) -> Vec<Edge> {
    (0..points.len())
        .map(|i| (points[i], points[(i + 1) % points.len()]))
        .collect()
}

fn open_edges(points: &[Point]) -> Vec<Edge> {
    points.windows(2).map(|w| (w[0], w[1])).collect()
}

fn cross(o: Point, a: Point, b: Point) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

fn on_segment(a: Point, b: Point, p: Point) -> bool {
    p.x >= a.x.min(b.x) && p.x <= a.x.max(b.x) && p.y >= a.y.min(b.y) && p.y <= a.y.max(b.y)
}

/// Segments `ab` and `cd` share at least one point (touching counts).
fn segments_intersect(a: Point, b: Point, c: Point, d: Point) -> bool {
    let (d1, d2) = (cross(c, d, a), cross(c, d, b));
    let (d3, d4) = (cross(a, b, c), cross(a, b, d));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    (d1 == 0.0 && on_segment(c, d, a))
        || (d2 == 0.0 && on_segment(c, d, b))
        || (d3 == 0.0 && on_segment(a, b, c))
        || (d4 == 0.0 && on_segment(a, b, d))
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// Segment `ab` meets the circle outline (center `c`, radius `r`).
fn segment_meets_circle(a: Point, b: Point, c: Point, r: f64) -> bool {
    let nearest = distance_to_segment(c, a, b);
    let farthest = distance(c, a).max(distance(c, b));
    nearest <= r && r <= farthest
}

/// Ray-casting point-in-polygon test.
fn point_in_polygon(p: Point, polygon: &[Point]) -> bool {
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Outline segments of line-like entities (circles have none).
fn outline(entity: &Entity) -> Vec<Edge> {
    match entity {
        Entity::Dimension(d) => d.picture().iter().flat_map(outline).collect(),
        Entity::Insert(i) => i.picture().iter().flat_map(outline).collect(),
        Entity::MLeader(m) => m.picture().iter().flat_map(outline).collect(),
        Entity::Leader(l) => l.picture().iter().flat_map(outline).collect(),
        Entity::MText(m) => closed_edges(&m.frame()),
        Entity::Hatch(h) => hatch_contours(h)
            .iter()
            .flat_map(|c| closed_edges(c))
            .collect(),
        Entity::Line { start, end } => vec![(*start, *end)],
        Entity::Point { .. } | Entity::Circle { .. } => Vec::new(),
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => {
            let tolerance = (radius.abs() * 1e-3).max(1e-9);
            let points = arc::flatten(*center, *radius, *start, *end, tolerance);
            points.windows(2).map(|w| (w[0], w[1])).collect()
        }
        Entity::Ellipse(e) => {
            let tolerance = (e.major_radius() * 1e-3).max(1e-9);
            let points = e.outline(tolerance);
            points.windows(2).map(|w| (w[0], w[1])).collect()
        }
        Entity::Spline(s) => {
            let points = spline_outline(s);
            points.windows(2).map(|w| (w[0], w[1])).collect()
        }
        Entity::Rectangle { min, max } => closed_edges(&Region::rect_corners(*min, *max)),
        Entity::Polyline {
            points,
            closed,
            bulges,
            ..
        } => {
            let flat = flattened(entity, points, bulges, *closed, 1e-4);
            segments(&flat, *closed).collect()
        }
    }
}

/// A spline followed within a thousandth of the size of its control points.
fn spline_outline(s: &od_core::Spline) -> Vec<Point> {
    let b = s.bounds();
    let size = (b.max.x - b.min.x).hypot(b.max.y - b.min.y);
    s.outline((size * 1e-3).max(1e-9))
}

/// The path of a polyline with its arcs approximated, within `precision` of
/// its size.
fn flattened(
    entity: &Entity,
    points: &[Point],
    bulges: &[f64],
    closed: bool,
    precision: f64,
) -> Vec<Point> {
    if bulges.is_empty() {
        return points.to_vec();
    }
    let b = entity.bounds();
    let tolerance = ((b.max.x - b.min.x).hypot(b.max.y - b.min.y) * precision).max(1e-9);
    bulge::flatten_path(points, bulges, closed, tolerance)
}

/// Points that stand for the entity when testing "is it inside".
fn sample_points(entity: &Entity) -> Vec<Point> {
    match entity {
        // A window must hold the whole dimension, text included.
        Entity::Dimension(d) => {
            let b = d.bounds();
            Region::rect_corners(b.min, b.max)
        }
        Entity::MText(m) => m.frame().to_vec(),
        Entity::Leader(l) => {
            let b = l.bounds();
            Region::rect_corners(b.min, b.max)
        }
        // A window must hold the whole multileader, text included.
        Entity::MLeader(m) => {
            let b = m.bounds();
            Region::rect_corners(b.min, b.max)
        }
        // A window must hold the whole boundary.
        Entity::Hatch(h) => hatch_contours(h).into_iter().flatten().collect(),
        // A window must hold everything the block draws.
        Entity::Insert(i) => i.picture().iter().flat_map(sample_points).collect(),
        Entity::Line { start, end } => vec![*start, *end],
        Entity::Point { at } => vec![*at],
        Entity::Circle { center, radius } => {
            let r = radius.abs();
            vec![
                Point::new(center.x + r, center.y),
                Point::new(center.x - r, center.y),
                Point::new(center.x, center.y + r),
                Point::new(center.x, center.y - r),
            ]
        }
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => {
            // Both ends and any extreme point the arc passes, so a window
            // must hold the whole curve and not just its chord.
            let r = radius.abs();
            let mut points = vec![
                arc::point_at(*center, r, *start),
                arc::point_at(*center, r, *end),
            ];
            let quarters = (0..4).map(|q| f64::from(q) * std::f64::consts::FRAC_PI_2);
            points.extend(
                quarters
                    .filter(|a| arc::contains(*start, *end, *a))
                    .map(|a| arc::point_at(*center, r, a)),
            );
            points
        }
        // Its ends and the points where it runs level or upright: a window
        // that holds those (and that no edge of which cuts the outline)
        // holds it all.
        Entity::Ellipse(e) => e.extreme_points(),
        // Points along it, within a thousandth of its size.
        Entity::Spline(s) => spline_outline(s),
        Entity::Rectangle { min, max } => Region::rect_corners(*min, *max),
        Entity::Polyline {
            points,
            closed,
            bulges,
            ..
        } => flattened(entity, points, bulges, *closed, 1e-2),
    }
}

fn touches(entity: &Entity, edges: &[Edge]) -> bool {
    match entity {
        Entity::Circle { center, radius } => edges
            .iter()
            .any(|(a, b)| segment_meets_circle(*a, *b, *center, radius.abs())),
        Entity::Insert(i) => i.picture().iter().any(|piece| touches(piece, edges)),
        Entity::MLeader(m) => m.picture().iter().any(|piece| touches(piece, edges)),
        Entity::Leader(l) => l.picture().iter().any(|piece| touches(piece, edges)),
        _ => {
            let own = outline(entity);
            edges
                .iter()
                .any(|(a, b)| own.iter().any(|(c, d)| segments_intersect(*a, *b, *c, *d)))
        }
    }
}

pub(crate) fn selected(entity: &Entity, region: &Region) -> bool {
    selected_by(region, &sample_points(entity), |edges| {
        touches(entity, edges)
    })
}

/// Whether `region` selects an object whose stand-in points are `samples`
/// and which touches `edges` as `touches` says.
fn selected_by(region: &Region, samples: &[Point], touches: impl Fn(&[Edge]) -> bool) -> bool {
    match region {
        Region::Fence(points) => points.len() >= 2 && touches(&open_edges(points)),
        Region::Window(points) | Region::Crossing(points) if points.len() < 3 => false,
        Region::Window(points) => {
            samples.iter().all(|p| point_in_polygon(*p, points)) && !touches(&closed_edges(points))
        }
        Region::Crossing(points) => {
            touches(&closed_edges(points)) || samples.iter().any(|p| point_in_polygon(*p, points))
        }
    }
}

/// [`selected`] for a stored object. A dimension is tested by the picture the
/// object remembers and its remembered box.
pub(crate) fn selected_object(object: &Object, region: &Region) -> bool {
    let Some(picture) = object.dimension_picture() else {
        return selected(&object.entity, region);
    };
    let b = object.bounds();
    let own: Vec<Edge> = picture.iter().flat_map(outline).collect();
    selected_by(region, &Region::rect_corners(b.min, b.max), |edges| {
        edges
            .iter()
            .any(|(a, b)| own.iter().any(|(c, d)| segments_intersect(*a, *b, *c, *d)))
    })
}

/// Ids of the objects `region` selects.
pub fn select<'a>(objects: impl IntoIterator<Item = &'a Object>, region: &Region) -> Vec<EntityId> {
    objects
        .into_iter()
        .filter(|o| selected_object(o, region))
        .map(|o| o.id)
        .collect()
}

/// [`select`] among the selectable objects of the space being edited, asking
/// the document's index which lie within reach of the region first.
pub fn select_in(document: &Document, region: &Region) -> Vec<EntityId> {
    match region.envelope() {
        Some(envelope) => select(document.selectable_here_in_rect(envelope), region),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{ids, objs};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> Entity {
        Entity::Line {
            start: p(x1, y1),
            end: p(x2, y2),
        }
    }

    fn circle(x: f64, y: f64, r: f64) -> Entity {
        Entity::Circle {
            center: p(x, y),
            radius: r,
        }
    }

    fn rect(x1: f64, y1: f64, x2: f64, y2: f64) -> Entity {
        Entity::Rectangle {
            min: p(x1, y1),
            max: p(x2, y2),
        }
    }

    fn square() -> Vec<Point> {
        Region::rect_corners(p(0.0, 0.0), p(100.0, 100.0))
    }

    fn arc(x: f64, y: f64, r: f64, start: f64, end: f64) -> Entity {
        Entity::Arc {
            center: p(x, y),
            radius: r,
            start,
            end,
        }
    }

    #[test]
    fn arcs_are_selected_by_the_curve_not_the_circle() {
        use std::f64::consts::PI;
        let entities = [
            // Upper half of a circle whose lower half would stick out.
            arc(50.0, 10.0, 30.0, 0.0, PI),
            // The same circle, lower half: sticks out of the window below.
            arc(50.0, 10.0, 30.0, PI, 2.0 * PI),
            // Bulges past the right edge only.
            arc(90.0, 50.0, 30.0, -1.0, 1.0),
        ];
        assert_eq!(
            select(&objs(&entities), &Region::Window(square())),
            ids(&[0])
        );
        let fence = Region::Fence(vec![p(0.0, 90.0), p(100.0, 90.0)]);
        assert_eq!(select(&objs(&entities), &fence), ids(&[]));
        let fence = Region::Fence(vec![p(0.0, 30.0), p(130.0, 30.0)]);
        assert_eq!(select(&objs(&entities), &fence), ids(&[0, 2]));
    }

    #[test]
    fn splines_are_selected_by_their_curve_not_their_control_frame() {
        // Its middle control point is far above the window; its curve peaks
        // at y = 80, inside.
        let frame = vec![p(10.0, 10.0), p(50.0, 150.0), p(90.0, 10.0)];
        let s = od_core::Spline::from_control_points(2, frame).unwrap();
        let entities = [Entity::Spline(Box::new(s))];
        assert_eq!(
            select(&objs(&entities), &Region::Window(square())),
            ids(&[0])
        );
        let peak = Region::rect_corners(p(45.0, 75.0), p(55.0, 85.0));
        assert_eq!(select(&objs(&entities), &Region::Crossing(peak)), ids(&[0]));
        let frame_only = Region::rect_corners(p(45.0, 145.0), p(55.0, 155.0));
        assert!(select(&objs(&entities), &Region::Crossing(frame_only)).is_empty());
        let fence = Region::Fence(vec![p(0.0, 50.0), p(100.0, 50.0)]);
        assert_eq!(select(&objs(&entities), &fence), ids(&[0]));
        // A window that holds the ends but not the peak does not take it.
        let low = Region::rect_corners(p(0.0, 0.0), p(100.0, 70.0));
        assert!(select(&objs(&entities), &Region::Window(low)).is_empty());
    }

    #[test]
    fn ellipses_are_selected_by_their_curve() {
        use std::f64::consts::{PI, TAU};
        let ellipse = |x: f64, y: f64, a: f64, ratio: f64, end: f64| {
            Entity::Ellipse(od_core::Ellipse::new(p(x, y), p(a, 0.0), ratio, 0.0, end).unwrap())
        };
        let entities = [
            // Flat and inside the window.
            ellipse(50.0, 50.0, 40.0, 0.25, TAU),
            // Wider than the window: its ends stick out left and right.
            ellipse(50.0, 50.0, 60.0, 0.25, TAU),
            // The upper half of one whose lower half would stick out below.
            ellipse(50.0, 10.0, 30.0, 0.5, PI),
        ];
        assert_eq!(
            select(&objs(&entities), &Region::Window(square())),
            ids(&[0, 2])
        );
        let corner = Region::rect_corners(p(105.0, 45.0), p(115.0, 55.0));
        assert_eq!(
            select(&objs(&entities), &Region::Crossing(corner)),
            ids(&[1])
        );
        // Across both upper sides; the wide one is crossed just outside x 0..100.
        let fence = Region::Fence(vec![p(-20.0, 58.0), p(120.0, 58.0)]);
        assert_eq!(select(&objs(&entities), &fence), ids(&[0, 1]));
    }

    #[test]
    fn window_takes_only_fully_enclosed_entities() {
        let entities = [
            line(10.0, 10.0, 90.0, 90.0),     // inside
            line(50.0, 50.0, 150.0, 50.0),    // sticks out
            circle(50.0, 50.0, 30.0),         // inside
            circle(50.0, 50.0, 70.0),         // surrounds the window, outline outside
            circle(90.0, 50.0, 30.0),         // crosses the edge
            rect(20.0, 20.0, 80.0, 80.0),     // inside
            rect(80.0, 80.0, 120.0, 120.0),   // overlaps
            line(200.0, 200.0, 300.0, 300.0), // far away
        ];
        assert_eq!(
            select(&objs(&entities), &Region::Window(square())),
            ids(&[0, 2, 5])
        );
    }

    #[test]
    fn crossing_takes_enclosed_and_touching_entities() {
        let entities = [
            line(10.0, 10.0, 90.0, 90.0),
            line(50.0, 50.0, 150.0, 50.0),
            circle(50.0, 50.0, 30.0),
            circle(90.0, 50.0, 30.0),
            rect(80.0, 80.0, 120.0, 120.0),
            line(200.0, 200.0, 300.0, 300.0),
            // Spans the region without any endpoint inside.
            line(-50.0, 50.0, 150.0, 50.0),
        ];
        assert_eq!(
            select(&objs(&entities), &Region::Crossing(square())),
            ids(&[0, 1, 2, 3, 4, 6])
        );
    }

    #[test]
    fn circle_surrounding_the_region_is_not_touched() {
        let entities = [circle(50.0, 50.0, 500.0)];
        assert!(select(&objs(&entities), &Region::Crossing(square())).is_empty());
    }

    #[test]
    fn fence_selects_what_the_polyline_crosses() {
        let entities = [
            line(0.0, 0.0, 100.0, 0.0),
            line(0.0, 50.0, 100.0, 50.0),
            circle(50.0, 100.0, 20.0),
            rect(200.0, 200.0, 300.0, 300.0),
        ];
        let fence = Region::Fence(vec![p(50.0, -10.0), p(50.0, 60.0), p(50.0, 90.0)]);
        assert_eq!(select(&objs(&entities), &fence), ids(&[0, 1, 2]));
    }

    #[test]
    fn fence_is_open_so_it_does_not_close_back_to_the_start() {
        let entities = [line(0.0, 5.0, 10.0, 5.0)];
        // Path from (0,0) to (10,0) to (10,10); a closed loop would cross
        // the line again via (10,10)->(0,0), the open fence only at (10,5).
        let fence = Region::Fence(vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)]);
        assert_eq!(select(&objs(&entities), &fence), ids(&[0]));
        let miss = Region::Fence(vec![p(0.0, 0.0), p(5.0, 0.0), p(5.0, 3.0)]);
        assert!(select(&objs(&entities), &miss).is_empty());
    }

    #[test]
    fn concave_window_rejects_a_line_that_leaves_the_polygon() {
        // An L shape; the diagonal has both endpoints inside but cuts the
        // notch.
        let l_shape = vec![
            p(0.0, 0.0),
            p(100.0, 0.0),
            p(100.0, 40.0),
            p(40.0, 40.0),
            p(40.0, 100.0),
            p(0.0, 100.0),
        ];
        let entities = [line(80.0, 20.0, 20.0, 80.0), line(10.0, 10.0, 90.0, 20.0)];
        assert_eq!(
            select(&objs(&entities), &Region::Window(l_shape)),
            ids(&[1])
        );
    }

    #[test]
    fn degenerate_regions_select_nothing() {
        let entities = [line(0.0, 0.0, 10.0, 10.0)];
        assert!(select(
            &objs(&entities),
            &Region::Window(vec![p(0.0, 0.0), p(5.0, 5.0)])
        )
        .is_empty());
        assert!(select(&objs(&entities), &Region::Crossing(vec![])).is_empty());
        assert!(select(&objs(&entities), &Region::Fence(vec![p(0.0, 0.0)])).is_empty());
    }

    #[test]
    fn segment_intersection_cases() {
        assert!(segments_intersect(
            p(0.0, 0.0),
            p(10.0, 10.0),
            p(0.0, 10.0),
            p(10.0, 0.0)
        ));
        // Touching at an endpoint counts.
        assert!(segments_intersect(
            p(0.0, 0.0),
            p(5.0, 5.0),
            p(5.0, 5.0),
            p(9.0, 0.0)
        ));
        // Collinear overlap and collinear gap.
        assert!(segments_intersect(
            p(0.0, 0.0),
            p(5.0, 0.0),
            p(3.0, 0.0),
            p(9.0, 0.0)
        ));
        assert!(!segments_intersect(
            p(0.0, 0.0),
            p(2.0, 0.0),
            p(3.0, 0.0),
            p(9.0, 0.0)
        ));
        assert!(!segments_intersect(
            p(0.0, 0.0),
            p(1.0, 1.0),
            p(5.0, 0.0),
            p(6.0, 1.0)
        ));
    }

    #[test]
    fn polyline_is_selected_by_window_only_when_all_vertices_are_inside() {
        let inside = Entity::Polyline {
            points: vec![p(10.0, 10.0), p(50.0, 20.0), p(40.0, 80.0)],
            closed: true,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        let sticking_out = Entity::Polyline {
            points: vec![p(10.0, 10.0), p(500.0, 20.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        let entities = [inside, sticking_out];
        assert_eq!(
            select(&objs(&entities), &Region::Window(square())),
            ids(&[0])
        );
        assert_eq!(
            select(&objs(&entities), &Region::Crossing(square())),
            ids(&[0, 1])
        );
    }

    #[test]
    fn a_hatch_is_selected_by_its_boundary() {
        use od_core::hatch::{
            HatchBoundary, HatchEntity, HatchFill, HatchLoop, HatchLoopFlags, HatchPolyline,
        };
        let hatch = Entity::Hatch(Box::new(HatchEntity::new(
            vec![HatchLoop {
                flags: HatchLoopFlags::default(),
                boundary: HatchBoundary::Polyline(HatchPolyline {
                    vertices: vec![p(20.0, 20.0), p(60.0, 20.0), p(60.0, 60.0), p(20.0, 60.0)],
                    bulges: Vec::new(),
                    closed: true,
                }),
                sources: Vec::new(),
            }],
            HatchFill::Solid,
        )));
        let entities = [hatch];
        assert_eq!(
            select(&objs(&entities), &Region::Window(square())),
            ids(&[0])
        );
        // A window that cuts it does not hold it, a crossing one takes it.
        let part = Region::rect_corners(p(0.0, 0.0), p(40.0, 40.0));
        assert!(select(&objs(&entities), &Region::Window(part.clone())).is_empty());
        assert_eq!(select(&objs(&entities), &Region::Crossing(part)), ids(&[0]));
        // A fence across it touches its boundary.
        let fence = Region::Fence(vec![p(0.0, 40.0), p(100.0, 40.0)]);
        assert_eq!(select(&objs(&entities), &fence), ids(&[0]));
    }
}
