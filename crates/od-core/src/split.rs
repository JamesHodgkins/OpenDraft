//! Cutting an entity into pieces at points on it. The base for Trim (cut at
//! the cutting edges, drop the piece that was clicked) and Break.

use std::f64::consts::TAU;

use crate::intersect::corners;
use crate::{arc, bulge, Entity, Point};

/// Positions closer than this (as a fraction of a segment or in radians) are
/// the same place.
const EPS: f64 = 1e-9;

impl Entity {
    /// The pieces left when the outline is cut at `points`, in order along
    /// it. A point not on the outline cuts where the outline is nearest to
    /// it. Cuts at the very ends, and repeated cuts, are ignored. With
    /// nothing to cut at, the entity comes back whole.
    ///
    /// - A line gives lines, an arc gives arcs.
    /// - A circle needs two cuts to give arcs (it comes back whole for fewer);
    ///   an ellipse likewise gives elliptical arcs, and an elliptical arc
    ///   gives elliptical arcs.
    /// - A rectangle or polyline gives open polylines. A closed one cut once
    ///   comes back as one open polyline that starts and ends at the cut.
    pub fn split_at(&self, points: &[Point]) -> Vec<Entity> {
        match self {
            // Splines are not cut yet (Trim and Break refuse them).
            Entity::Dimension(_)
            | Entity::MText(_)
            | Entity::MLeader(_)
            | Entity::Leader(_)
            | Entity::Insert(_)
            | Entity::Hatch(_)
            | Entity::Spline(_) => {
                vec![self.clone()]
            }
            Entity::Line { start, end } => {
                split_line(*start, *end, points).unwrap_or_else(|| vec![self.clone()])
            }
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => split_arc(*center, *radius, *start, *end, points)
                .unwrap_or_else(|| vec![self.clone()]),
            Entity::Circle { center, radius } => {
                split_circle(*center, radius.abs(), points).unwrap_or_else(|| vec![self.clone()])
            }
            Entity::Ellipse(e) => e.split_at(points).map_or_else(
                || vec![self.clone()],
                |pieces| pieces.into_iter().map(Entity::Ellipse).collect(),
            ),
            Entity::Rectangle { min, max } => split_path(&corners(*min, *max), &[], true, points)
                .unwrap_or_else(|| vec![self.clone()]),
            Entity::Polyline {
                points: vertices,
                closed,
                bulges,
                ..
            } => {
                split_path(vertices, bulges, *closed, points).unwrap_or_else(|| vec![self.clone()])
            }
        }
    }
}

/// Sorted values with the ones that repeat an earlier one removed.
fn sorted_unique(mut values: Vec<f64>) -> Vec<f64> {
    values.sort_by(f64::total_cmp);
    values.dedup_by(|a, b| (*a - *b).abs() < EPS);
    values
}

fn lerp(a: Point, b: Point, t: f64) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

fn split_line(a: Point, b: Point, cuts: &[Point]) -> Option<Vec<Entity>> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len_sq = dx * dx + dy * dy;
    if len_sq == 0.0 {
        return None;
    }
    let at: Vec<f64> = cuts
        .iter()
        .map(|c| (((c.x - a.x) * dx + (c.y - a.y) * dy) / len_sq).clamp(0.0, 1.0))
        .filter(|t| *t > EPS && *t < 1.0 - EPS)
        .collect();
    let at = sorted_unique(at);
    if at.is_empty() {
        return None;
    }
    let mut marks = vec![0.0];
    marks.extend(at);
    marks.push(1.0);
    Some(
        marks
            .windows(2)
            .map(|w| Entity::Line {
                start: lerp(a, b, w[0]),
                end: lerp(a, b, w[1]),
            })
            .collect(),
    )
}

fn split_arc(
    center: Point,
    radius: f64,
    start: f64,
    end: f64,
    cuts: &[Point],
) -> Option<Vec<Entity>> {
    let sweep = arc::sweep(start, end);
    // How far round from the start each cut is, nearest the arc itself.
    let at: Vec<f64> = cuts
        .iter()
        .map(|c| (arc::angle_of(center, *c) - start).rem_euclid(TAU))
        .filter(|d| *d > EPS && *d < sweep - EPS)
        .collect();
    let at = sorted_unique(at);
    if at.is_empty() {
        return None;
    }
    let mut marks = vec![0.0];
    marks.extend(at);
    marks.push(sweep);
    Some(
        marks
            .windows(2)
            .map(|w| Entity::Arc {
                center,
                radius,
                start: arc::normalize(start + w[0]),
                end: arc::normalize(start + w[1]),
            })
            .collect(),
    )
}

fn split_circle(center: Point, radius: f64, cuts: &[Point]) -> Option<Vec<Entity>> {
    let angles = sorted_unique(cuts.iter().map(|c| arc::angle_of(center, *c)).collect());
    // The first and last can be the same place seen from either side of zero.
    let angles = if angles.len() > 1 && angles[0] + TAU - angles[angles.len() - 1] < EPS {
        angles[..angles.len() - 1].to_vec()
    } else {
        angles
    };
    if angles.len() < 2 {
        return None;
    }
    Some(
        (0..angles.len())
            .map(|i| Entity::Arc {
                center,
                radius,
                start: angles[i],
                end: angles[(i + 1) % angles.len()],
            })
            .collect(),
    )
}

/// One segment of a path: its two ends and its bulge.
type Seg = (Point, Point, f64);

/// Where along a path a point is: the segment number plus how far through
/// it (a fraction of the length of a line, of the angle of an arc), nearest
/// the point.
fn path_position(segment: &dyn Fn(usize) -> Seg, count: usize, p: Point) -> f64 {
    let mut best = (f64::INFINITY, 0.0);
    for i in 0..count {
        let (a, b, bulge) = segment(i);
        let (q, t) = bulge::nearest(a, b, bulge, p);
        let d = (p.x - q.x).hypot(p.y - q.y);
        if d < best.0 - 1e-12 {
            best = (d, i as f64 + t);
        }
    }
    best.1
}

fn split_path(
    vertices: &[Point],
    bulges: &[f64],
    closed: bool,
    cuts: &[Point],
) -> Option<Vec<Entity>> {
    let n = vertices.len();
    let count = bulge::segment_count(n, closed);
    if count == 0 {
        return None;
    }
    let segment = |i: usize| -> Seg {
        (
            vertices[i % n],
            vertices[(i + 1) % n],
            bulge::at(bulges, i % count),
        )
    };
    let total = count as f64;
    let mut at: Vec<f64> = cuts
        .iter()
        .map(|c| path_position(&segment, count, *c))
        .collect();
    if closed {
        // The end of the last segment is the start of the first.
        for position in &mut at {
            if *position > total - EPS {
                *position = 0.0;
            }
        }
    } else {
        at.retain(|p| *p > EPS && *p < total - EPS);
    }
    let at = sorted_unique(at);
    if at.is_empty() {
        return None;
    }

    // The path from one position to a later one, a stretch at a time (one per
    // segment it passes through) so arcs keep their curve: a piece of an arc
    // is an arc through the same circle, with its own bulge.
    let piece = |from: f64, to: f64| {
        let mut points: Vec<Point> = Vec::new();
        let mut curves: Vec<f64> = Vec::new();
        let mut position = from;
        while position < to - EPS {
            let i = position.floor() as usize;
            let end = to.min((i + 1) as f64);
            let (a, b, bulge) = segment(i);
            let (t0, t1) = (position - i as f64, end - i as f64);
            points.push(bulge::point_at(a, b, bulge, t0));
            curves.push(bulge::part(bulge, t0, t1));
            position = end;
            if position >= to - EPS {
                points.push(bulge::point_at(a, b, bulge, t1));
                curves.push(0.0);
            }
        }
        // A repeated point leaves a zero-length stretch: keep the point once,
        // with the bulge of the stretch that goes on from it.
        let mut merged: Vec<(Point, f64)> = Vec::new();
        for (p, k) in points.into_iter().zip(curves) {
            match merged.last_mut() {
                Some(last) if (p.x - last.0.x).hypot(p.y - last.0.y) < EPS => last.1 = k,
                _ => merged.push((p, k)),
            }
        }
        let (points, curves): (Vec<Point>, Vec<f64>) = merged.into_iter().unzip();
        (points.len() >= 2).then(|| Entity::Polyline {
            points,
            closed: false,
            bulges: if bulge::has_arcs(&curves) {
                curves
            } else {
                Vec::new()
            },
            widths: Vec::new(),
        })
    };

    let mut marks = Vec::new();
    if closed {
        marks.extend(at.iter().copied());
        marks.push(at[0] + total);
    } else {
        marks.push(0.0);
        marks.extend(at.iter().copied());
        marks.push(total);
    }
    Some(marks.windows(2).filter_map(|w| piece(w[0], w[1])).collect())
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).hypot(a.y - b.y) < 1e-9
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn polyline(points: &[Point], closed: bool) -> Entity {
        Entity::Polyline {
            points: points.to_vec(),
            closed,
            bulges: Vec::new(),
            widths: Vec::new(),
        }
    }

    fn points_of(e: &Entity) -> Vec<Point> {
        match e {
            Entity::Polyline { points, .. } => points.clone(),
            other => panic!("expected a polyline, got {other:?}"),
        }
    }

    #[test]
    fn a_line_is_cut_into_lines_in_order() {
        let l = line(p(0.0, 0.0), p(10.0, 0.0));
        let pieces = l.split_at(&[p(7.0, 0.0), p(3.0, 0.0)]);
        assert_eq!(
            pieces,
            [
                line(p(0.0, 0.0), p(3.0, 0.0)),
                line(p(3.0, 0.0), p(7.0, 0.0)),
                line(p(7.0, 0.0), p(10.0, 0.0)),
            ]
        );
    }

    #[test]
    fn cuts_at_the_ends_repeats_and_nothing_leave_it_whole() {
        let l = line(p(0.0, 0.0), p(10.0, 0.0));
        for cuts in [
            vec![],
            vec![p(0.0, 0.0)],
            vec![p(10.0, 0.0), p(0.0, 0.0)],
            vec![p(-5.0, 0.0), p(20.0, 3.0)],
        ] {
            assert_eq!(l.split_at(&cuts), std::slice::from_ref(&l), "{cuts:?}");
        }
        assert_eq!(
            l.split_at(&[p(4.0, 0.0), p(4.0, 0.0)]).len(),
            2,
            "a repeat is one cut"
        );
        // A point beside the line cuts where the line is nearest it.
        let pieces = l.split_at(&[p(4.0, 9.0)]);
        assert_eq!(pieces[0], line(p(0.0, 0.0), p(4.0, 0.0)));
    }

    #[test]
    fn an_arc_is_cut_into_arcs_that_join_up() {
        let a = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 5.0,
            start: 0.0,
            end: PI,
        };
        let pieces = a.split_at(&[p(0.0, 5.0)]);
        assert_eq!(pieces.len(), 2);
        let Entity::Arc { start, end, .. } = pieces[0] else {
            panic!()
        };
        assert!(start.abs() < 1e-9 && (end - FRAC_PI_2).abs() < 1e-9);
        let Entity::Arc { start, end, .. } = pieces[1] else {
            panic!()
        };
        assert!((start - FRAC_PI_2).abs() < 1e-9 && (end - PI).abs() < 1e-9);
        // A cut on the missing half is not on the arc: it is ignored.
        assert_eq!(a.split_at(&[p(0.0, -5.0)]), std::slice::from_ref(&a));
    }

    #[test]
    fn an_arc_across_zero_is_cut_in_sweep_order() {
        // From 270 degrees round through 0 to 90.
        let a = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 1.0,
            start: 3.0 * FRAC_PI_2,
            end: FRAC_PI_2,
        };
        let pieces = a.split_at(&[p(1.0, 0.0)]);
        let ends: Vec<(f64, f64)> = pieces
            .iter()
            .map(|e| match e {
                Entity::Arc { start, end, .. } => (*start, *end),
                _ => panic!(),
            })
            .collect();
        assert!((ends[0].0 - 3.0 * FRAC_PI_2).abs() < 1e-9 && ends[0].1.abs() < 1e-9);
        assert!(ends[1].0.abs() < 1e-9 && (ends[1].1 - FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn a_circle_needs_two_cuts_to_become_arcs() {
        let c = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 2.0,
        };
        assert_eq!(c.split_at(&[p(2.0, 0.0)]), std::slice::from_ref(&c));
        let pieces = c.split_at(&[p(0.0, 2.0), p(0.0, -2.0)]);
        assert_eq!(pieces.len(), 2);
        let sweeps: Vec<f64> = pieces
            .iter()
            .map(|e| match e {
                Entity::Arc { start, end, .. } => arc::sweep(*start, *end),
                _ => panic!(),
            })
            .collect();
        assert!((sweeps[0] - PI).abs() < 1e-9 && (sweeps[1] - PI).abs() < 1e-9);
        // Three cuts, three arcs that add up to the whole circle.
        let pieces = c.split_at(&[p(2.0, 0.0), p(0.0, 2.0), p(-2.0, 0.0)]);
        assert_eq!(pieces.len(), 3);
    }

    #[test]
    fn an_open_polyline_is_cut_through_its_vertices() {
        let l = polyline(
            &[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)],
            false,
        );
        // One cut on the first segment and one on the last.
        let pieces = l.split_at(&[p(4.0, 0.0), p(5.0, 10.0)]);
        assert_eq!(pieces.len(), 3);
        assert_eq!(points_of(&pieces[0]), [p(0.0, 0.0), p(4.0, 0.0)]);
        assert_eq!(
            points_of(&pieces[1]),
            [p(4.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(5.0, 10.0)]
        );
        assert_eq!(points_of(&pieces[2]), [p(5.0, 10.0), p(0.0, 10.0)]);
    }

    #[test]
    fn a_cut_at_a_vertex_does_not_repeat_it() {
        let l = polyline(&[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)], false);
        let pieces = l.split_at(&[p(10.0, 0.0)]);
        assert_eq!(pieces.len(), 2);
        assert_eq!(points_of(&pieces[0]), [p(0.0, 0.0), p(10.0, 0.0)]);
        assert_eq!(points_of(&pieces[1]), [p(10.0, 0.0), p(10.0, 10.0)]);
    }

    #[test]
    fn a_rectangle_cut_twice_gives_the_two_open_paths_between() {
        let r = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(10.0, 10.0),
        };
        let pieces = r.split_at(&[p(5.0, 0.0), p(5.0, 10.0)]);
        assert_eq!(pieces.len(), 2);
        // From the bottom cut round the right-hand side to the top cut.
        assert_eq!(
            points_of(&pieces[0]),
            [p(5.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(5.0, 10.0)]
        );
        // And on round the left-hand side back to the bottom cut.
        assert_eq!(
            points_of(&pieces[1]),
            [p(5.0, 10.0), p(0.0, 10.0), p(0.0, 0.0), p(5.0, 0.0)]
        );
    }

    #[test]
    fn a_polyline_arc_segment_is_cut_into_arcs_with_their_own_bulges() {
        // A half circle below the chord from (0, 0) to (4, 0), then a straight leg up.
        let l = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 5.0)],
            closed: false,
            bulges: vec![1.0, 0.0, 0.0],
            widths: Vec::new(),
        };
        // Cut at the bottom of the half circle.
        let pieces = l.split_at(&[p(2.0, -2.0)]);
        assert_eq!(pieces.len(), 2);
        let Entity::Polyline { points, bulges, .. } = &pieces[0] else {
            panic!()
        };
        assert!(near(points[0], p(0.0, 0.0)) && near(points[1], p(2.0, -2.0)));
        // A quarter circle: bulge tan(90 degrees / 4).
        assert!((bulges[0] - std::f64::consts::FRAC_PI_8.tan()).abs() < 1e-9);
        let Entity::Polyline { points, bulges, .. } = &pieces[1] else {
            panic!()
        };
        assert_eq!(points.len(), 3);
        assert!(near(points[0], p(2.0, -2.0)) && near(points[1], p(4.0, 0.0)));
        assert!((bulges[0] - std::f64::consts::FRAC_PI_8.tan()).abs() < 1e-9);
        assert!(bulges[1].abs() < 1e-12, "the straight leg stays straight");
        // Cutting through the arc and the leg gives three pieces.
        assert_eq!(l.split_at(&[p(2.0, -2.0), p(4.0, 3.0)]).len(), 3);
    }

    #[test]
    fn a_closed_path_cut_once_opens_at_the_cut() {
        let tri = polyline(&[p(0.0, 0.0), p(6.0, 0.0), p(0.0, 6.0)], true);
        let pieces = tri.split_at(&[p(3.0, 0.0)]);
        assert_eq!(pieces.len(), 1);
        let pts = points_of(&pieces[0]);
        assert!(near(pts[0], p(3.0, 0.0)) && near(*pts.last().unwrap(), p(3.0, 0.0)));
        assert_eq!(pts.len(), 5, "the cut, three corners and the cut again");
        // A cut at the first corner opens it there without a zero-length piece.
        let pieces = tri.split_at(&[p(0.0, 0.0)]);
        assert_eq!(pieces.len(), 1);
        assert_eq!(points_of(&pieces[0]).len(), 4);
    }

    /// The length of a line, arc or open polyline.
    fn length(e: &Entity) -> f64 {
        match e {
            Entity::Line { start, end } => (end.x - start.x).hypot(end.y - start.y),
            Entity::Arc {
                radius, start, end, ..
            } => radius * arc::sweep(*start, *end),
            Entity::Polyline {
                points,
                closed,
                bulges,
                ..
            } => {
                let n = points.len();
                (0..bulge::segment_count(n, *closed))
                    .map(|i| bulge::length(points[i], points[(i + 1) % n], bulge::at(bulges, i)))
                    .sum()
            }
            other => panic!("no length for {other:?}"),
        }
    }

    #[test]
    fn cutting_never_changes_the_total_length() {
        let mut state = 99_u64;
        let mut next = move || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        for round in 0..1500 {
            let mut point = || p(next() * 100.0, next() * 100.0);
            let entity = match round % 4 {
                0 => line(point(), point()),
                1 => Entity::Arc {
                    center: point(),
                    radius: 5.0 + point().x,
                    start: point().x / 16.0,
                    end: point().y / 16.0,
                },
                2 => polyline(&[point(), point(), point(), point()], round % 2 == 0),
                // Some segments are arcs, of different turns and directions.
                _ => Entity::Polyline {
                    points: vec![point(), point(), point(), point()],
                    closed: round % 2 == 0,
                    bulges: vec![0.7, -0.3, 1.2, -0.9],
                    widths: Vec::new(),
                },
            };
            let cuts: Vec<Point> = (0..1 + round % 3)
                .map(|_| entity.nearest_point(point()).unwrap())
                .collect();
            let pieces = entity.split_at(&cuts);
            let total: f64 = pieces.iter().map(length).sum();
            assert!(
                (total - length(&entity)).abs() < 1e-6 * (1.0 + length(&entity)),
                "{entity:?} cut at {cuts:?} -> {pieces:?}"
            );
        }
    }
}
