//! Stroke glyphs as filled outlines.
//!
//! Painting and plotting fill closed contours with the non-zero rule, so a
//! stroke becomes one capsule (a rectangle with a half disc at each end) per
//! straight piece, every one wound counter-clockwise. Overlapping capsules
//! then fill as one shape, with the round joins and caps the design asks for,
//! and no polygon clipping is needed. A writer that filled even-odd would
//! punch holes where capsules overlap; both of ours say non-zero.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use super::font::{pieces, Contour};
use super::stroke_font::{Pt, Segment, Stroke, StrokeGlyph};

/// The pen width bounds, as a fraction of cap height. Thinner vanishes at
/// plot resolution; thicker fills the counters.
pub const MIN_WEIGHT: f64 = 0.01;
pub const MAX_WEIGHT: f64 = 0.40;

/// Most points one half disc is cut into.
const MAX_ARC_STEPS: usize = 32;

/// `glyph` as counter-clockwise contours in font units, for a pen of
/// `weight` (a fraction of `cap_height`, clamped to the allowed range) with
/// curves and caps kept within `tolerance` font units.
pub fn stroke_contours(
    glyph: &StrokeGlyph,
    weight: f64,
    cap_height: f64,
    tolerance: f64,
) -> Vec<Contour> {
    let width = weight.clamp(MIN_WEIGHT, MAX_WEIGHT) * cap_height;
    let tolerance = tolerance.max(1e-6);
    let mut out = Vec::new();
    for stroke in &glyph.strokes {
        let half = width * stroke.weight / 2.0;
        let line = flatten(stroke, tolerance);
        if let [only] = line.as_slice() {
            out.push(disc(*only, half, tolerance));
            continue;
        }
        for pair in line.windows(2) {
            if let [a, b] = pair {
                if let Some(c) = capsule(*a, *b, half, tolerance) {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// The points of a stroke with its curves cut into straight pieces.
fn flatten(stroke: &Stroke, tolerance: f64) -> Vec<Pt> {
    let mut points = vec![stroke.start];
    for segment in &stroke.segments {
        let from = points.last().copied().unwrap_or(stroke.start);
        match *segment {
            Segment::Line(to) => points.push(to),
            Segment::Cubic(c1, c2, to) => {
                let bend =
                    |a: Pt, b: Pt, c: Pt| (a.0 - 2.0 * b.0 + c.0).hypot(a.1 - 2.0 * b.1 + c.1);
                let n = pieces(bend(from, c1, c2).max(bend(c1, c2, to)), 0.75, tolerance);
                for i in 1..=n {
                    let t = i as f64 / n as f64;
                    let u = 1.0 - t;
                    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    points.push((
                        a * from.0 + b * c1.0 + c * c2.0 + d * to.0,
                        a * from.1 + b * c1.1 + c * c2.1 + d * to.1,
                    ));
                }
            }
        }
    }
    points
}

/// Points per half disc so a chord stays within `tolerance` of radius `r`.
fn arc_steps(r: f64, tolerance: f64) -> usize {
    if tolerance >= r {
        return 2;
    }
    let step = 2.0 * (1.0 - tolerance / r).acos();
    ((PI / step).ceil() as usize).clamp(2, MAX_ARC_STEPS)
}

fn disc(centre: Pt, r: f64, tolerance: f64) -> Contour {
    let n = 2 * arc_steps(r, tolerance);
    (0..n)
        .map(|i| {
            let a = TAU * i as f64 / n as f64;
            (centre.0 + r * a.cos(), centre.1 + r * a.sin())
        })
        .collect()
}

/// The capsule around the segment `a` to `b`, or `None` if they coincide.
fn capsule(a: Pt, b: Pt, r: f64, tolerance: f64) -> Option<Contour> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    if dx.hypot(dy) < 1e-9 {
        return None;
    }
    let heading = dy.atan2(dx);
    let n = arc_steps(r, tolerance);
    let mut contour = Vec::with_capacity(2 * n + 2);
    // The end cap runs from the right of the heading round to its left, the
    // start cap from the left round to the right: counter-clockwise.
    for (centre, from) in [(b, heading - FRAC_PI_2), (a, heading + FRAC_PI_2)] {
        for i in 0..=n {
            let angle = from + PI * i as f64 / n as f64;
            contour.push((centre.0 + r * angle.cos(), centre.1 + r * angle.sin()));
        }
    }
    Some(contour)
}

#[cfg(test)]
mod tests;
