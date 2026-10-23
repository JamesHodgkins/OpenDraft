//! Which loops are filled: nesting depth and the island styles.
//!
//! A loop's depth is how many larger loops contain it. Winding direction
//! plays no part. The filled area is the even-odd area of the loops kept:
//! Normal keeps them all (so every other nesting level is filled), Outer
//! keeps depths 0 and 1, Ignore keeps depth 0.

use super::{HatchError, HatchStyle};
use crate::{Bounds, Point};

/// Most loops whose nesting is worked out (the work grows with the square).
pub(super) const MAX_NESTED_LOOPS: usize = 4096;

/// The area of a polygon, positive whichever way it runs.
pub fn contour_area(points: &[Point]) -> f64 {
    signed_area(points).abs()
}

pub(super) fn signed_area(points: &[Point]) -> f64 {
    let n = points.len();
    let twice: f64 = (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum();
    twice / 2.0
}

/// Whether `p` is inside the polygon (even-odd).
pub(super) fn contains(polygon: &[Point], p: Point) -> bool {
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

fn bounds_of(points: &[Point]) -> Bounds {
    let first = points[0];
    points
        .iter()
        .fold(Bounds::from_points(first, first), |b, p| {
            b.union(Bounds::from_points(*p, *p))
        })
}

fn bounds_contain(outer: &Bounds, inner: &Bounds) -> bool {
    outer.min.x <= inner.min.x
        && outer.min.y <= inner.min.y
        && outer.max.x >= inner.max.x
        && outer.max.y >= inner.max.y
}

/// A point strictly inside a simple polygon: the middle of the first filled
/// stretch of a horizontal line through the widest gap between vertex
/// heights, so it never lies on a vertex or along an edge.
pub(super) fn interior_point(polygon: &[Point]) -> Option<Point> {
    let mut ys: Vec<f64> = polygon.iter().map(|p| p.y).collect();
    ys.sort_by(f64::total_cmp);
    ys.dedup();
    let (lo, hi) = ys
        .windows(2)
        .map(|w| (w[0], w[1]))
        .max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)))?;
    let y = (lo + hi) / 2.0;
    let n = polygon.len();
    let mut xs: Vec<f64> = (0..n)
        .filter_map(|i| {
            let (a, b) = (polygon[i], polygon[(i + 1) % n]);
            ((a.y > y) != (b.y > y)).then(|| a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x))
        })
        .collect();
    xs.sort_by(f64::total_cmp);
    (xs.len() >= 2).then(|| Point::new((xs[0] + xs[1]) / 2.0, y))
}

/// The loops that are filled, and the signed area they add up to: each kept
/// loop adds its area at even depth and takes it away at odd depth.
pub(super) struct Selected {
    pub contours: Vec<Vec<Point>>,
    pub area: f64,
}

/// Applies `style` to `contours`. Loops with no area are left out.
pub(super) fn select(contours: Vec<Vec<Point>>, style: HatchStyle) -> Result<Selected, HatchError> {
    let contours: Vec<Vec<Point>> = contours
        .into_iter()
        .filter(|c| c.len() >= 3 && contour_area(c) > 0.0)
        .collect();
    if contours.len() > MAX_NESTED_LOOPS {
        return Err(HatchError::TooLarge("boundaries to nest"));
    }
    let areas: Vec<f64> = contours.iter().map(|c| contour_area(c)).collect();
    let boxes: Vec<Bounds> = contours.iter().map(|c| bounds_of(c)).collect();
    let samples: Vec<Option<Point>> = contours.iter().map(|c| interior_point(c)).collect();
    let depth = |i: usize| -> usize {
        let Some(p) = samples[i] else {
            return 0;
        };
        (0..contours.len())
            .filter(|&j| {
                j != i
                    && areas[j] > areas[i]
                    && bounds_contain(&boxes[j], &boxes[i])
                    && contains(&contours[j], p)
            })
            .count()
    };
    let depths: Vec<usize> = (0..contours.len()).map(depth).collect();
    let mut kept = Vec::new();
    let mut area = 0.0;
    for (i, mut contour) in contours.into_iter().enumerate() {
        let d = depths[i];
        let keep = match style {
            HatchStyle::Normal => true,
            HatchStyle::Outer => d <= 1,
            HatchStyle::Ignore => d == 0,
        };
        if keep {
            area += if d.is_multiple_of(2) {
                areas[i]
            } else {
                -areas[i]
            };
            // Outer loops run counter-clockwise and islands clockwise, so
            // the non-zero rule fills the same as even-odd.
            if (signed_area(&contour) > 0.0) != d.is_multiple_of(2) {
                contour.reverse();
            }
            kept.push(contour);
        }
    }
    Ok(Selected {
        contours: kept,
        area: area.max(0.0),
    })
}
