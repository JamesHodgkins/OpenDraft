//! Polyline widths: the `(start, end)` width of each segment. Like bulges,
//! the list is empty or has one entry per point. Painting a wide polyline
//! needs its area, which [`triangles`] gives.

use crate::{bulge, Point};

/// The widths of segment `i`; a hairline when the polyline has no widths.
pub fn at(widths: &[(f64, f64)], i: usize) -> (f64, f64) {
    widths.get(i).copied().unwrap_or((0.0, 0.0))
}

/// Whether any segment is wide.
pub fn has_widths(widths: &[(f64, f64)]) -> bool {
    widths.iter().any(|&(a, b)| a != 0.0 || b != 0.0)
}

/// Where a segment's edge starts or ends: the point, the unit normal on its
/// left, and half the width there.
#[derive(Clone, Copy)]
struct Edge {
    at: Point,
    normal: (f64, f64),
    half: f64,
}

fn offset(edge: Edge, side: f64) -> Point {
    Point::new(
        edge.at.x + side * edge.normal.0 * edge.half,
        edge.at.y + side * edge.normal.1 * edge.half,
    )
}

/// The left-hand unit normal of the way through `before` and `after`.
fn normal_of(before: Point, after: Point) -> (f64, f64) {
    let (dx, dy) = (after.x - before.x, after.y - before.y);
    let len = dx.hypot(dy);
    if len == 0.0 {
        (0.0, 0.0)
    } else {
        (-dy / len, dx / len)
    }
}

/// The ribbon of one segment as triangles, and the edges at its two ends.
/// Arcs are cut into pieces within `tolerance` of the curve; the width
/// tapers along the length.
fn ribbon(
    a: Point,
    b: Point,
    curve: f64,
    (w0, w1): (f64, f64),
    tolerance: f64,
    out: &mut Vec<[Point; 3]>,
) -> Option<(Edge, Edge)> {
    let flat = bulge::flatten(a, b, curve, tolerance);
    let total: f64 = flat.windows(2).map(|w| distance(w[0], w[1])).sum();
    if flat.len() < 2 || total == 0.0 {
        return None;
    }
    let mut run = 0.0;
    let edges: Vec<Edge> = (0..flat.len())
        .map(|k| {
            if k > 0 {
                run += distance(flat[k - 1], flat[k]);
            }
            let before = flat[k.saturating_sub(1)];
            let after = flat[(k + 1).min(flat.len() - 1)];
            Edge {
                at: flat[k],
                normal: normal_of(before, after),
                half: (w0 + (w1 - w0) * run / total) / 2.0,
            }
        })
        .collect();
    for pair in edges.windows(2) {
        let (l0, r0) = (offset(pair[0], 1.0), offset(pair[0], -1.0));
        let (l1, r1) = (offset(pair[1], 1.0), offset(pair[1], -1.0));
        out.push([l0, r0, l1]);
        out.push([r0, r1, l1]);
    }
    Some((edges[0], edges[edges.len() - 1]))
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// The area a wide polyline covers, as triangles, so a painter can fill it.
/// Each segment is a ribbon that tapers from its start width to its end
/// width; at a corner the gap on the outside is filled with a bevel. Empty
/// when no segment is wide.
pub fn triangles(
    points: &[Point],
    bulges: &[f64],
    widths: &[(f64, f64)],
    closed: bool,
    tolerance: f64,
) -> Vec<[Point; 3]> {
    let mut out = Vec::new();
    if !has_widths(widths) {
        return out;
    }
    let n = points.len();
    let count = bulge::segment_count(n, closed);
    let ends: Vec<Option<(Edge, Edge)>> = (0..count)
        .map(|i| {
            ribbon(
                points[i],
                points[(i + 1) % n],
                bulge::at(bulges, i),
                at(widths, i),
                tolerance,
                &mut out,
            )
        })
        .collect();
    // Corners: where one segment ends and the next starts.
    let joins = if closed {
        count
    } else {
        count.saturating_sub(1)
    };
    for i in 0..joins {
        let (Some((_, incoming)), Some((outgoing, _))) = (ends[i], ends[(i + 1) % count]) else {
            continue;
        };
        for side in [1.0, -1.0] {
            out.push([incoming.at, offset(incoming, side), offset(outgoing, side)]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn area(triangles: &[[Point; 3]]) -> f64 {
        triangles
            .iter()
            .map(|t| {
                ((t[1].x - t[0].x) * (t[2].y - t[0].y) - (t[2].x - t[0].x) * (t[1].y - t[0].y))
                    .abs()
                    / 2.0
            })
            .sum()
    }

    #[test]
    fn a_hairline_polyline_covers_no_area() {
        let none = triangles(&[p(0.0, 0.0), p(5.0, 0.0)], &[], &[], false, 0.1);
        assert!(none.is_empty());
        let zero = triangles(
            &[p(0.0, 0.0), p(5.0, 0.0)],
            &[],
            &[(0.0, 0.0); 2],
            false,
            0.1,
        );
        assert!(zero.is_empty());
    }

    #[test]
    fn a_wide_segment_covers_length_times_width() {
        let t = triangles(
            &[p(0.0, 0.0), p(5.0, 0.0)],
            &[],
            &[(2.0, 2.0), (0.0, 0.0)],
            false,
            0.1,
        );
        assert!((area(&t) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn a_tapered_segment_covers_the_average_width() {
        // 0 at the start to 4 at the end: a triangle, 5 long, 4 across.
        let t = triangles(
            &[p(0.0, 0.0), p(5.0, 0.0)],
            &[],
            &[(0.0, 4.0), (0.0, 0.0)],
            false,
            0.1,
        );
        assert!((area(&t) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn a_corner_is_filled_on_the_outside() {
        let points = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0)];
        let t = triangles(
            &points,
            &[],
            &[(2.0, 2.0), (2.0, 2.0), (0.0, 0.0)],
            false,
            0.1,
        );
        // Two ribbons of 4 x 2, plus the bevel on the outside (half of 1 x 1)
        // and the same on the inside, which lies within the ribbons' overlap.
        assert!((area(&t) - 17.0).abs() < 1e-9, "{}", area(&t));
    }

    #[test]
    fn an_arc_segment_follows_the_curve() {
        // A half circle of radius 2 (bulge 1), 0.5 wide: area is about the
        // length of the arc times the width.
        let t = triangles(
            &[p(0.0, 0.0), p(4.0, 0.0)],
            &[1.0, 0.0],
            &[(0.5, 0.5), (0.0, 0.0)],
            false,
            0.001,
        );
        let want = std::f64::consts::PI * 2.0 * 0.5;
        assert!((area(&t) - want).abs() < 0.01, "{} vs {want}", area(&t));
    }

    #[test]
    fn a_closed_polyline_is_joined_at_every_corner() {
        let square = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
        let open = triangles(&square, &[], &[(1.0, 1.0); 4], false, 0.1);
        let closed = triangles(&square, &[], &[(1.0, 1.0); 4], true, 0.1);
        assert!(area(&closed) > area(&open) + 4.0 - 1e-9, "a fourth ribbon");
    }
}
