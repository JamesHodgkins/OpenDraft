//! Properties of a closed outline: whether it crosses itself, and where the
//! centre of the area it encloses is. Pure geometry for the Geometric Center
//! snap.

use crate::{bulge, Point};

/// Outlines with more flattened points than this are not tested for
/// crossing themselves (the test is quadratic); they count as not simple.
const SIMPLE_TEST_LIMIT: usize = 400;

/// The area `p` to `q` and a point `r` span, twice, signed.
fn cross(p: Point, q: Point, r: Point) -> f64 {
    (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x)
}

/// Whether the segments `a`-`b` and `c`-`d` meet (touching counts).
fn segments_meet(a: Point, b: Point, c: Point, d: Point) -> bool {
    let (d1, d2) = (cross(a, b, c), cross(a, b, d));
    let (d3, d4) = (cross(c, d, a), cross(c, d, b));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    let on = |p: Point, q: Point, r: Point, d: f64| {
        d == 0.0
            && r.x >= p.x.min(q.x)
            && r.x <= p.x.max(q.x)
            && r.y >= p.y.min(q.y)
            && r.y <= p.y.max(q.y)
    };
    on(a, b, c, d1) || on(a, b, d, d2) || on(c, d, a, d3) || on(c, d, b, d4)
}

/// Whether the closed polygon through `points` has no two edges that meet
/// except neighbours at their shared corner. Too large a polygon says no.
pub fn polygon_is_simple(points: &[Point]) -> bool {
    let n = points.len();
    if !(3..=SIMPLE_TEST_LIMIT).contains(&n) {
        return false;
    }
    for i in 0..n {
        let (a, b) = (points[i], points[(i + 1) % n]);
        for j in i + 1..n {
            // Neighbours share a corner, which is no crossing.
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            if segments_meet(a, b, points[j], points[(j + 1) % n]) {
                return false;
            }
        }
    }
    true
}

/// The centre of the area the closed path through `points` (with the
/// `bulges` of its arc segments) encloses. `None` when it encloses no area
/// or crosses itself, where the area has no single meaning.
pub fn path_centroid(points: &[Point], bulges: &[f64]) -> Option<Point> {
    let n = points.len();
    if n < 3 {
        return None;
    }
    let size = points
        .iter()
        .fold(0.0_f64, |m, p| m.max(p.x.abs()).max(p.y.abs()));
    let outline = bulge::flatten_path(points, bulges, true, (size * 1e-3).max(1e-9));
    // The flattening of a closed path ends where it began.
    let outline = outline.strip_suffix(&[outline[0]]).unwrap_or(&outline);
    if !polygon_is_simple(outline) {
        return None;
    }
    // The area of the chords' polygon, and its centre.
    let (mut twice, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let c = a.x * b.y - b.x * a.y;
        twice += c;
        cx += (a.x + b.x) * c;
        cy += (a.y + b.y) * c;
    }
    let area = twice / 2.0;
    if area.abs() < 1e-12 * (1.0 + size * size) {
        return None;
    }
    let (mut total, mut mx, mut my) = (area, cx / 6.0, cy / 6.0);
    // Each arc adds (or, bulging in, takes away) the sliver between its
    // chord and itself, which sits on the arc's side of its centre.
    for i in 0..n {
        let (a, b, bulge_i) = (points[i], points[(i + 1) % n], bulge::at(bulges, i));
        let Some(arc) = bulge::arc_of(a, b, bulge_i) else {
            continue;
        };
        let theta = 4.0 * bulge_i.abs().atan();
        let slack = theta - theta.sin();
        if slack < 1e-9 {
            continue;
        }
        let r = arc.radius;
        let sliver = r * r / 2.0 * slack;
        let mid = bulge::midpoint(a, b, bulge_i);
        let (dx, dy) = ((mid.x - arc.center.x) / r, (mid.y - arc.center.y) / r);
        let reach = 4.0 * r * (theta / 2.0).sin().powi(3) / (3.0 * slack);
        let signed = bulge_i.signum() * sliver;
        total += signed;
        mx += signed * (arc.center.x + dx * reach);
        my += signed * (arc.center.y + dy * reach);
    }
    (total.abs() > 1e-12).then(|| Point::new(mx / total, my / total))
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).hypot(a.y - b.y) < 1e-6
    }

    #[test]
    fn a_polygon_is_simple_unless_two_far_edges_meet() {
        let square = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
        assert!(polygon_is_simple(&square));
        // A bow tie crosses itself.
        let bow = [p(0.0, 0.0), p(4.0, 4.0), p(4.0, 0.0), p(0.0, 4.0)];
        assert!(!polygon_is_simple(&bow));
        // So does one that pinches back onto its own corner.
        let touch = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(2.0, 2.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(2.0, 2.0),
        ];
        assert!(!polygon_is_simple(&touch));
        assert!(!polygon_is_simple(&square[..2]));
    }

    #[test]
    fn the_centre_of_a_polygon_is_its_area_centre() {
        // A right triangle: a third of the way along from the right angle.
        let t = path_centroid(&[p(0.0, 0.0), p(6.0, 0.0), p(0.0, 3.0)], &[]).unwrap();
        assert!(near(t, p(2.0, 1.0)), "{t:?}");
        // Either way round gives the same place.
        let back = path_centroid(&[p(0.0, 3.0), p(6.0, 0.0), p(0.0, 0.0)], &[]).unwrap();
        assert!(near(back, p(2.0, 1.0)));
        // An L is not at the middle of its box.
        let l = path_centroid(
            &[
                p(0.0, 0.0),
                p(4.0, 0.0),
                p(4.0, 1.0),
                p(1.0, 1.0),
                p(1.0, 4.0),
                p(0.0, 4.0),
            ],
            &[],
        )
        .unwrap();
        assert!(near(l, p(1.357_142_857, 1.357_142_857)), "{l:?}");
    }

    #[test]
    fn an_arc_segment_moves_the_centre_toward_its_bulge() {
        // A 2 by 2 square with a half disc added below its bottom edge
        // (bulge 1 on the edge running east, bulging to the right of it).
        let c = path_centroid(
            &[p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(0.0, 2.0)],
            &[1.0, 0.0, 0.0, 0.0],
        )
        .unwrap();
        let area = 4.0 + PI / 2.0;
        let y = (4.0 * 1.0 - PI / 2.0 * (4.0 / (3.0 * PI))) / area;
        assert!(near(c, p(1.0, y)), "{c:?} want y {y}");
        // The same bulge the other way takes the half disc out of the square.
        let inward = path_centroid(
            &[p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(0.0, 2.0)],
            &[-1.0, 0.0, 0.0, 0.0],
        )
        .unwrap();
        let y = (4.0 * 1.0 - PI / 2.0 * (4.0 / (3.0 * PI))) / (4.0 - PI / 2.0);
        assert!(near(inward, p(1.0, y)), "{inward:?}");
    }

    #[test]
    fn no_area_and_a_path_that_crosses_itself_have_no_centre() {
        assert!(path_centroid(&[p(0.0, 0.0), p(5.0, 0.0), p(9.0, 0.0)], &[]).is_none());
        assert!(
            path_centroid(&[p(0.0, 0.0), p(4.0, 4.0), p(4.0, 0.0), p(0.0, 4.0)], &[]).is_none()
        );
        assert!(path_centroid(&[p(0.0, 0.0), p(1.0, 1.0)], &[]).is_none());
    }
}
