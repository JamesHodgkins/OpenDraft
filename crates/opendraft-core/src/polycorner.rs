//! Rounding or cutting the vertices of a polyline (or a rectangle). The base
//! for the Polyline option of FILLET and CHAMFER and for joining two
//! segments of one polyline.
//!
//! The polyline stays one object. A chamfer replaces a corner vertex with two
//! points joined by a straight segment, and a fillet with two points joined by
//! an arc segment (a bulge). Only a vertex between two straight segments can
//! be rounded; one next to an arc segment is left as it is.

use std::f64::consts::PI;

use crate::corner::{Chamfer, CornerError};
use crate::intersect::corners;
use crate::{bulge, Entity, Point};

const EPS: f64 = 1e-9;

/// How each vertex is treated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Round {
    /// An arc of this radius.
    Fillet(f64),
    Chamfer(Chamfer),
}

/// The result of rounding a polyline.
#[derive(Clone, Debug, PartialEq)]
pub struct Rounded {
    /// What replaces the polyline: one polyline.
    pub pieces: Vec<Entity>,
    /// Vertices that were rounded or cut.
    pub done: usize,
    /// Corner vertices left sharp because the neighbouring segments were too
    /// short to give up what the corner needed.
    pub skipped: usize,
}

/// A polyline or rectangle as its vertices, their bulges and whether it
/// closes.
pub struct Path {
    pub points: Vec<Point>,
    pub bulges: Vec<f64>,
    pub closed: bool,
}

impl Entity {
    /// The vertices of a polyline or a rectangle.
    pub fn polyline_path(&self) -> Option<Path> {
        match self {
            Entity::Polyline {
                points,
                closed,
                bulges,
                ..
            } => Some(Path {
                points: points.clone(),
                bulges: bulges.clone(),
                closed: *closed,
            }),
            Entity::Rectangle { min, max } => Some(Path {
                points: corners(*min, *max).to_vec(),
                bulges: Vec::new(),
                closed: true,
            }),
            _ => None,
        }
    }

    /// Which segment of a polyline or rectangle is nearest `p`. Segment `i`
    /// runs from vertex `i` to the next, so it is the one before vertex
    /// `i + 1`.
    pub fn segment_at(&self, p: Point) -> Option<usize> {
        let path = self.polyline_path()?;
        let n = path.points.len();
        (0..bulge::segment_count(n, path.closed))
            .map(|i| {
                let (a, b) = (path.points[i], path.points[(i + 1) % n]);
                let (q, _) = bulge::nearest(a, b, bulge::at(&path.bulges, i), p);
                (i, dist(q, p))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// The vertex two segments of a polyline meet at, when they are
    /// neighbours.
    pub fn shared_vertex(&self, first: usize, second: usize) -> Option<usize> {
        let path = self.polyline_path()?;
        let n = path.points.len();
        let segments = bulge::segment_count(n, path.closed);
        if first >= segments || second >= segments || first == second {
            return None;
        }
        // Segment i ends at vertex i + 1 (wrapping).
        if (first + 1) % n == second {
            Some(second)
        } else if (second + 1) % n == first {
            Some(first)
        } else {
            None
        }
    }
}

fn dist(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn unit(from: Point, to: Point) -> Option<Point> {
    let len = dist(from, to);
    (len > EPS).then(|| Point::new((to.x - from.x) / len, (to.y - from.y) / len))
}

fn along(from: Point, direction: Point, distance: f64) -> Point {
    Point::new(
        from.x + direction.x * distance,
        from.y + direction.y * distance,
    )
}

/// What one vertex becomes: the point where the incoming segment now ends,
/// the point where the outgoing one now starts, and the bulge of the step
/// between them (0 for a chamfer).
struct Cut {
    ta: Point,
    tb: Point,
    step: f64,
    /// How far the cut reaches along the segment before and after.
    before: f64,
    after: f64,
}

enum Item {
    /// A vertex that stays, with the bulge of the segment leaving it.
    Sharp(Point, f64),
    Cut(Cut),
}

/// The cut at vertex `at` between `prev` and `next`, or `None` when the
/// vertex is straight or turns straight back.
fn cut_at(prev: Point, at: Point, next: Point, how: Round) -> Result<Option<Cut>, CornerError> {
    let (Some(u_in), Some(u_out)) = (unit(at, prev), unit(at, next)) else {
        return Ok(None);
    };
    let angle = (u_in.x * u_out.x + u_in.y * u_out.y)
        .clamp(-1.0, 1.0)
        .acos();
    if !(1e-6..=PI - 1e-6).contains(&angle) {
        return Ok(None);
    }
    match how {
        Round::Fillet(radius) => {
            if !(radius.is_finite() && radius > EPS) {
                return Ok(None);
            }
            let reach = radius / (angle / 2.0).tan();
            // Travelling in along `-u_in` and out along `u_out`: a left turn
            // makes the arc counter-clockwise.
            let turn = -u_in.x * u_out.y + u_in.y * u_out.x;
            Ok(Some(Cut {
                ta: along(at, u_in, reach),
                tb: along(at, u_out, reach),
                step: bulge::from_sweep(PI - angle, turn > 0.0),
                before: reach,
                after: reach,
            }))
        }
        Round::Chamfer(chamfer) => {
            let (before, after) = chamfer.distances(angle).ok_or(CornerError::TooLong)?;
            if before <= EPS && after <= EPS {
                return Ok(None);
            }
            Ok(Some(Cut {
                ta: along(at, u_in, before),
                tb: along(at, u_out, after),
                step: 0.0,
                before,
                after,
            }))
        }
    }
}

/// Round or cut the corners of `path`. `only` limits it to one vertex, which
/// must be a corner that fits; otherwise every corner that fits is done and
/// the rest are counted in `skipped`.
pub fn round_polyline(
    path: &Path,
    how: Round,
    only: Option<usize>,
) -> Result<Rounded, CornerError> {
    // Repeated points have no direction.
    let mut kept: Vec<(Point, f64)> = Vec::new();
    for (i, p) in path.points.iter().enumerate() {
        if kept.last().is_none_or(|last| dist(last.0, *p) > EPS) {
            kept.push((*p, bulge::at(&path.bulges, i)));
        }
    }
    if path.closed && kept.len() > 1 && dist(kept[0].0, kept[kept.len() - 1].0) <= EPS {
        kept.pop();
    }
    let n = kept.len();
    let closed = path.closed && n > 2;
    if n < 3 {
        return Err(CornerError::NotAdjacent);
    }
    let point = |i: usize| kept[i % n].0;
    let curve = |i: usize| kept[i % n].1;
    let straight = |i: usize| curve(i).abs() <= bulge::STRAIGHT;
    let len_of = |segment: usize| dist(point(segment), point(segment + 1));
    // Length of each segment already given up to a corner.
    let mut used = vec![0.0_f64; n];
    let mut items: Vec<Item> = Vec::with_capacity(n);
    let (mut done, mut skipped) = (0, 0);
    for v in 0..n {
        let interior = closed || (v > 0 && v < n - 1);
        let (prev, next) = ((v + n - 1) % n, (v + 1) % n);
        // Only a vertex between two straight segments can be rounded.
        let wanted = interior && straight(prev) && straight(v) && only.is_none_or(|o| o == v);
        if !wanted {
            if only == Some(v) {
                return Err(CornerError::NotAdjacent);
            }
            items.push(Item::Sharp(point(v), curve(v)));
            continue;
        }
        let Some(cut) = cut_at(point(prev), point(v), point(next), how)? else {
            if only.is_some() {
                return Err(CornerError::NotAdjacent);
            }
            items.push(Item::Sharp(point(v), curve(v)));
            continue;
        };
        // The segment before vertex v is `prev`; the one after is `v`.
        let fits =
            used[prev] + cut.before <= len_of(prev) + EPS && used[v] + cut.after <= len_of(v) + EPS;
        if !fits {
            if only.is_some() {
                return Err(CornerError::TooLong);
            }
            skipped += 1;
            items.push(Item::Sharp(point(v), curve(v)));
            continue;
        }
        used[prev] += cut.before;
        used[v] += cut.after;
        done += 1;
        items.push(Item::Cut(cut));
    }
    Ok(Rounded {
        pieces: vec![build(items, closed)],
        done,
        skipped,
    })
}

/// The polyline the items make: a cut vertex becomes two points, the first
/// carrying the bulge of the step between them.
fn build(items: Vec<Item>, closed: bool) -> Entity {
    let mut vertices: Vec<(Point, f64)> = Vec::new();
    for item in items {
        match item {
            Item::Sharp(p, k) => vertices.push((p, k)),
            Item::Cut(c) => {
                vertices.push((c.ta, c.step));
                vertices.push((c.tb, 0.0));
            }
        }
    }
    // A cut that uses a whole segment puts two points on one spot: keep one,
    // with the bulge of the segment that leaves it.
    let mut merged: Vec<(Point, f64)> = Vec::with_capacity(vertices.len());
    for (p, k) in vertices {
        match merged.last_mut() {
            Some(last) if dist(last.0, p) <= EPS => last.1 = k,
            _ => merged.push((p, k)),
        }
    }
    if closed && merged.len() > 1 && dist(merged[0].0, merged[merged.len() - 1].0) <= EPS {
        merged.pop();
    }
    let (points, curves): (Vec<Point>, Vec<f64>) = merged.into_iter().unzip();
    Entity::Polyline {
        points,
        closed,
        bulges: if bulge::has_arcs(&curves) {
            curves
        } else {
            Vec::new()
        },
        widths: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn near(a: Point, b: Point) -> bool {
        dist(a, b) < 1e-9
    }

    fn path(points: &[Point], closed: bool) -> Path {
        Path {
            points: points.to_vec(),
            bulges: Vec::new(),
            closed,
        }
    }

    fn square(side: f64) -> Vec<Point> {
        vec![p(0.0, 0.0), p(side, 0.0), p(side, side), p(0.0, side)]
    }

    fn parts(e: &Entity) -> (&Vec<Point>, &Vec<f64>, bool) {
        match e {
            Entity::Polyline {
                points,
                bulges,
                closed,
                ..
            } => (points, bulges, *closed),
            other => panic!("expected a polyline, got {other:?}"),
        }
    }

    fn length(e: &Entity) -> f64 {
        let (points, bulges, closed) = parts(e);
        let n = points.len();
        (0..bulge::segment_count(n, closed))
            .map(|i| bulge::length(points[i], points[(i + 1) % n], bulge::at(bulges, i)))
            .sum()
    }

    #[test]
    fn a_chamfered_vertex_keeps_the_polyline_whole() {
        let l = path(&[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)], false);
        let how = Round::Chamfer(Chamfer::Distances(2.0, 3.0));
        let done = round_polyline(&l, how, Some(1)).unwrap();
        assert_eq!(done.done, 1);
        let (points, bulges, closed) = parts(&done.pieces[0]);
        assert!(!closed && points.len() == 4 && bulges.is_empty());
        // Two along the first segment back from the corner, three along the second.
        assert!(near(points[1], p(8.0, 0.0)) && near(points[2], p(10.0, 3.0)));
    }

    #[test]
    fn chamfering_every_corner_of_a_square_gives_a_closed_octagon() {
        let how = Round::Chamfer(Chamfer::Distances(1.0, 1.0));
        let done = round_polyline(&path(&square(10.0), true), how, None).unwrap();
        assert_eq!((done.done, done.skipped), (4, 0));
        let (points, _, closed) = parts(&done.pieces[0]);
        assert!(closed && points.len() == 8);
        let perimeter = 4.0 * 8.0 + 4.0 * 2.0_f64.sqrt();
        assert!((length(&done.pieces[0]) - perimeter).abs() < 1e-9);
    }

    #[test]
    fn filleting_every_corner_of_a_square_gives_one_polyline_with_four_arc_segments() {
        let done = round_polyline(&path(&square(10.0), true), Round::Fillet(2.0), None).unwrap();
        assert_eq!((done.done, done.skipped), (4, 0));
        assert_eq!(done.pieces.len(), 1, "the polyline stays whole");
        let (points, bulges, closed) = parts(&done.pieces[0]);
        assert!(closed && points.len() == 8);
        let arcs: Vec<f64> = bulges.iter().copied().filter(|b| b.abs() > 1e-9).collect();
        assert_eq!(arcs.len(), 4);
        // Each is a quarter turn, counter-clockwise for a counter-clockwise square.
        let quarter = std::f64::consts::FRAC_PI_8.tan();
        assert!(arcs.iter().all(|b| (b - quarter).abs() < 1e-9), "{arcs:?}");
        // The perimeter loses 2r - (pi/2) r at each corner.
        let expected = 40.0 - 4.0 * (4.0 - std::f64::consts::PI);
        assert!((length(&done.pieces[0]) - expected).abs() < 1e-9);
    }

    #[test]
    fn the_arc_turns_the_right_way_for_a_clockwise_polyline() {
        let clockwise = vec![p(0.0, 0.0), p(0.0, 10.0), p(10.0, 10.0), p(10.0, 0.0)];
        let done = round_polyline(&path(&clockwise, true), Round::Fillet(2.0), None).unwrap();
        let (_, bulges, _) = parts(&done.pieces[0]);
        let quarter = std::f64::consts::FRAC_PI_8.tan();
        assert!(bulges
            .iter()
            .filter(|b| b.abs() > 1e-9)
            .all(|b| (b + quarter).abs() < 1e-9));
    }

    #[test]
    fn one_filleted_vertex_of_an_open_polyline_adds_an_arc_segment() {
        let l = path(
            &[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(20.0, 10.0)],
            false,
        );
        let done = round_polyline(&l, Round::Fillet(2.0), Some(1)).unwrap();
        let (points, bulges, closed) = parts(&done.pieces[0]);
        assert!(!closed);
        assert_eq!(points.len(), 5);
        assert!(near(points[1], p(8.0, 0.0)) && near(points[2], p(10.0, 2.0)));
        assert!(near(points[0], p(0.0, 0.0)) && near(points[4], p(20.0, 10.0)));
        assert!((bulges[1] - std::f64::consts::FRAC_PI_8.tan()).abs() < 1e-9);
        assert!(bulges[0].abs() < 1e-12 && bulges[2].abs() < 1e-12);
        // The arc segment really is a quarter circle of radius 2 about (8, 2).
        let mid = bulge::midpoint(points[1], points[2], bulges[1]);
        assert!(
            (dist(mid, p(8.0, 2.0)) - 2.0).abs() < 1e-9,
            "{mid:?} is not 2 from (8, 2)"
        );
    }

    #[test]
    fn a_straight_vertex_stays_and_the_corners_round() {
        // A square with an extra vertex halfway along the bottom.
        let shape = [
            p(0.0, 0.0),
            p(5.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 10.0),
            p(0.0, 10.0),
        ];
        let done = round_polyline(&path(&shape, true), Round::Fillet(1.0), None).unwrap();
        assert_eq!((done.done, done.skipped), (4, 0));
        let expected = 40.0 - 4.0 * (2.0 - std::f64::consts::FRAC_PI_2);
        assert!((length(&done.pieces[0]) - expected).abs() < 1e-9);
    }

    #[test]
    fn corners_whose_segments_are_too_short_are_skipped_and_counted() {
        // A 3 by 3 square cannot give 2 to every corner: neighbours share a side.
        let done = round_polyline(&path(&square(3.0), true), Round::Fillet(2.0), None).unwrap();
        assert_eq!(done.done + done.skipped, 4);
        assert!(done.skipped > 0 && done.done > 0);
        // Exactly half a side each fits, leaving no straight part.
        let exact = round_polyline(&path(&square(3.0), true), Round::Fillet(1.5), None).unwrap();
        assert_eq!((exact.done, exact.skipped), (4, 0));
        let (points, _, _) = parts(&exact.pieces[0]);
        assert_eq!(
            points.len(),
            4,
            "four arcs, each ending where the next begins"
        );
    }

    #[test]
    fn picking_one_vertex_that_cannot_be_rounded_says_why() {
        let l = path(&[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 1.0)], false);
        assert_eq!(
            round_polyline(&l, Round::Fillet(2.0), Some(1)),
            Err(CornerError::TooLong)
        );
        assert_eq!(
            round_polyline(&l, Round::Fillet(0.5), Some(0)),
            Err(CornerError::NotAdjacent),
            "an end of an open polyline is not a corner"
        );
        // A straight vertex is not a corner either.
        let straight = path(
            &[p(0.0, 0.0), p(5.0, 0.0), p(10.0, 0.0), p(10.0, 5.0)],
            false,
        );
        assert_eq!(
            round_polyline(&straight, Round::Fillet(1.0), Some(1)),
            Err(CornerError::NotAdjacent)
        );
    }

    #[test]
    fn a_vertex_next_to_an_arc_segment_is_left_alone() {
        // The first segment is a half circle: its end vertex has an arc on one side.
        let shape = Path {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 5.0), p(0.0, 5.0)],
            bulges: vec![1.0, 0.0, 0.0, 0.0],
            closed: false,
        };
        assert_eq!(
            round_polyline(&shape, Round::Fillet(0.5), Some(1)),
            Err(CornerError::NotAdjacent)
        );
        // Vertex 2 is between two straight segments and rounds; the arc stays.
        let done = round_polyline(&shape, Round::Fillet(0.5), None).unwrap();
        assert_eq!(done.done, 1);
        let (_, bulges, _) = parts(&done.pieces[0]);
        assert_eq!(bulges[0], 1.0, "the original arc is kept");
        assert!(bulges.iter().filter(|b| b.abs() > 1e-9).count() == 2);
    }

    #[test]
    fn segments_are_found_by_a_click_and_neighbours_share_a_vertex() {
        let rect = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(10.0, 6.0),
        };
        assert_eq!(rect.segment_at(p(5.0, -1.0)), Some(0), "the bottom");
        assert_eq!(rect.segment_at(p(11.0, 3.0)), Some(1), "the right side");
        assert_eq!(rect.segment_at(p(-1.0, 3.0)), Some(3), "the left side");
        assert_eq!(rect.shared_vertex(0, 1), Some(1));
        assert_eq!(rect.shared_vertex(1, 0), Some(1));
        assert_eq!(rect.shared_vertex(3, 0), Some(0), "the closing corner");
        assert_eq!(rect.shared_vertex(0, 2), None, "opposite sides do not meet");
        let open = Entity::Polyline {
            points: square(4.0),
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        assert_eq!(open.shared_vertex(0, 2), None);
        assert_eq!(open.shared_vertex(1, 2), Some(2));
        // An arc segment is found by its curve, not its chord.
        let bowed = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0)],
            closed: false,
            bulges: vec![1.0, 0.0, 0.0],
            widths: Vec::new(),
        };
        assert_eq!(bowed.segment_at(p(2.0, -2.0)), Some(0));
    }
}
