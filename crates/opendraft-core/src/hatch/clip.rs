//! Cutting pattern lines by the filled area.
//!
//! Each line family is worked in its own frame: `u` along the lines and `v`
//! across them, measured from the family's base point, so line `k` is
//! `v = k * spacing`. A boundary edge crosses line `k` when the line lies in
//! `[lowest v, highest v)` of the edge (half open, so a vertex on a line is
//! counted once, and a tangent touch twice or not at all). The crossings of a
//! line, sorted, pair up into the stretches inside the filled area.
//!
//! The boundary belongs to the area: a line that runs along an edge is drawn
//! along it, on both sides of a region, as AutoCAD does. Those stretches are
//! found apart from the crossings (an edge parallel to the lines has none)
//! and merged with them.

use super::dash::{self, Piece};
use super::eval::HatchLimits;
use super::{HatchError, PatternLine};
use crate::Point;

/// What the pattern draws.
#[derive(Default)]
pub(super) struct Strokes {
    pub segments: Vec<[Point; 2]>,
    pub dots: Vec<Point>,
}

/// Line `k` and a stretch of it, `(k, from, to)`, as distances `u` along it.
type Span = (i64, f64, f64);

/// `k` with `k * step >= x`, the smallest such, for a positive `step`.
fn first_at_or_after(x: f64, step: f64) -> i64 {
    let mut k = (x / step).ceil() as i64;
    while (k as f64) * step < x {
        k += 1;
    }
    while ((k - 1) as f64) * step >= x {
        k -= 1;
    }
    k
}

/// One family's frame: unit direction `d` along its lines, `m` across them
/// (turned so that the spacing is positive), the base point relative to the
/// middle of the hatch, and how far each line is from the last.
struct Frame {
    d: (f64, f64),
    m: (f64, f64),
    base: Point,
    center: Point,
    spacing: f64,
    shift: f64,
}

impl Frame {
    /// `p` as `(u, v)`.
    fn of(&self, p: Point) -> (f64, f64) {
        let (x, y) = (p.x - self.base.x, p.y - self.base.y);
        (x * self.d.0 + y * self.d.1, x * self.m.0 + y * self.m.1)
    }

    /// The point at distance `u` along line `k`, in drawing coordinates.
    fn at(&self, k: i64, u: f64) -> Point {
        let across = k as f64 * self.spacing;
        Point::new(
            self.center.x + self.base.x + u * self.d.0 + across * self.m.0,
            self.center.y + self.base.y + u * self.d.1 + across * self.m.1,
        )
    }
}

/// The strokes of `lines` over the area whose even-odd boundary is `contours`.
pub(super) fn strokes(
    contours: &[Vec<Point>],
    lines: &[PatternLine],
    limits: &HatchLimits,
) -> Result<Strokes, HatchError> {
    let mut out = Strokes::default();
    let Some((lo, hi)) = extent(contours) else {
        return Ok(out);
    };
    let center = Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
    let local: Vec<Vec<Point>> = contours
        .iter()
        .map(|c| {
            c.iter()
                .map(|p| Point::new(p.x - center.x, p.y - center.y))
                .collect()
        })
        .collect();
    let size = (hi.x - lo.x).hypot(hi.y - lo.y);
    let mut lines_used: u64 = 0;
    let mut spans: Vec<Span> = Vec::new();
    for line in lines {
        spans.clear();
        let frame = frame_of(line, center, size)?;
        let (v_lo, v_hi) = v_range(&local, &frame);
        count_lines(v_lo, v_hi, &frame, limits, &mut lines_used)?;
        collect(&local, &frame, size, limits, &mut spans)?;
        emit(&spans, &frame, line, size, limits, &mut out)?;
    }
    Ok(out)
}

fn extent(contours: &[Vec<Point>]) -> Option<(Point, Point)> {
    let mut points = contours.iter().flatten();
    let first = *points.next()?;
    Some(points.fold((first, first), |(lo, hi), p| {
        (
            Point::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    }))
}

fn frame_of(line: &PatternLine, center: Point, size: f64) -> Result<Frame, HatchError> {
    let finite = line.angle.is_finite()
        && line.base.x.is_finite()
        && line.base.y.is_finite()
        && line.offset.iter().all(|v| v.is_finite());
    if !finite {
        return Err(HatchError::NonFinite);
    }
    let (sin, cos) = line.angle.sin_cos();
    let d = (cos, sin);
    let mut m = (-sin, cos);
    let shift = line.offset[0] * d.0 + line.offset[1] * d.1;
    let mut spacing = line.offset[0] * m.0 + line.offset[1] * m.1;
    // The same set of lines, counted away from the base the other way.
    if spacing < 0.0 {
        m = (-m.0, -m.1);
        spacing = -spacing;
    }
    let reach = size.max(line.base.x.hypot(line.base.y)).max(1.0);
    if spacing <= 1e-12 * reach {
        return Err(HatchError::PatternSpacing);
    }
    Ok(Frame {
        d,
        m,
        base: Point::new(line.base.x - center.x, line.base.y - center.y),
        center,
        spacing,
        shift,
    })
}

fn v_range(local: &[Vec<Point>], frame: &Frame) -> (f64, f64) {
    local
        .iter()
        .flatten()
        .map(|p| frame.of(*p).1)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        })
}

/// Refuses a family that would take more lines than are left of the limit.
fn count_lines(
    v_lo: f64,
    v_hi: f64,
    frame: &Frame,
    limits: &HatchLimits,
    used: &mut u64,
) -> Result<(), HatchError> {
    let estimate = ((v_hi - v_lo) / frame.spacing).ceil() + 1.0;
    let left = limits.max_pattern_lines.saturating_sub(*used);
    // Line numbers past 2^50 have lost the precision to tell lines apart.
    let farthest = v_lo.abs().max(v_hi.abs()) / frame.spacing;
    if estimate.is_nan() || estimate > left as f64 || farthest >= 1e15 {
        return Err(HatchError::TooDense {
            estimated: used.saturating_add(if estimate.is_finite() {
                estimate as u64
            } else {
                u64::MAX
            }),
            limit: limits.max_pattern_lines,
        });
    }
    *used += estimate as u64;
    Ok(())
}

/// The stretches of every line of the family that lie in the area.
fn collect(
    local: &[Vec<Point>],
    frame: &Frame,
    size: f64,
    limits: &HatchLimits,
    spans: &mut Vec<Span>,
) -> Result<(), HatchError> {
    let near = (size * 1e-9).max(1e-12);
    let mut crossings: Vec<(i64, f64)> = Vec::new();
    for contour in local {
        for i in 0..contour.len() {
            let (ua, va) = frame.of(contour[i]);
            let (ub, vb) = frame.of(contour[(i + 1) % contour.len()]);
            let (lo, hi) = (va.min(vb), va.max(vb));
            let (k0, k1) = (
                first_at_or_after(lo, frame.spacing),
                first_at_or_after(hi, frame.spacing),
            );
            for k in k0..k1 {
                let level = k as f64 * frame.spacing;
                crossings.push((k, ua + (level - va) * (ub - ua) / (vb - va)));
            }
            // An edge along a line: that line is drawn along the edge.
            if hi - lo <= near {
                let k = (((va + vb) / 2.0) / frame.spacing).round() as i64;
                if (k as f64 * frame.spacing - (va + vb) / 2.0).abs() <= near && ua != ub {
                    spans.push((k, ua.min(ub), ua.max(ub)));
                }
            }
            if crossings.len() + spans.len() > limits.max_crossings {
                return Err(HatchError::TooDense {
                    estimated: (crossings.len() + spans.len()) as u64,
                    limit: limits.max_crossings as u64,
                });
            }
        }
    }
    crossings.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut i = 0;
    while i < crossings.len() {
        let k = crossings[i].0;
        let mut j = i;
        while j < crossings.len() && crossings[j].0 == k {
            j += 1;
        }
        // Crossings pair up: in, out, in, out. A last odd one is dropped.
        for first in (i..j.saturating_sub(1)).step_by(2) {
            spans.push((k, crossings[first].1, crossings[first + 1].1));
        }
        i = j;
    }
    Ok(())
}

/// Dashes the stretches (merging those that overlap or touch) and adds what
/// they draw to `out`.
fn emit(
    spans: &[Span],
    frame: &Frame,
    line: &PatternLine,
    size: f64,
    limits: &HatchLimits,
    out: &mut Strokes,
) -> Result<(), HatchError> {
    let period = if line.dashes.is_empty() {
        None
    } else {
        Some(dash::period(&line.dashes)?)
    };
    // A stretch shorter than rounding noise is a line touching a corner.
    let tiny = (size * 1e-9).max(1e-12);
    let mut sorted: Vec<Span> = spans
        .iter()
        .copied()
        .filter(|s| !s.1.is_nan() && !s.2.is_nan() && s.2 - s.1 > tiny)
        .collect();
    sorted.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut i = 0;
    while i < sorted.len() {
        let (k, from, mut to) = sorted[i];
        i += 1;
        while i < sorted.len() && sorted[i].0 == k && sorted[i].1 <= to {
            to = to.max(sorted[i].2);
            i += 1;
        }
        let used = out.segments.len() + out.dots.len();
        let room = limits.max_segments.saturating_sub(used);
        match period {
            None => {
                if room == 0 {
                    return Err(HatchError::TooDense {
                        estimated: used as u64 + 1,
                        limit: limits.max_segments as u64,
                    });
                }
                out.segments.push([frame.at(k, from), frame.at(k, to)]);
            }
            Some(p) => {
                let origin = k as f64 * frame.shift;
                dash::walk(
                    &line.dashes,
                    p,
                    origin,
                    from,
                    to,
                    room,
                    &mut |piece| match piece {
                        Piece::Dash(a, b) => out.segments.push([frame.at(k, a), frame.at(k, b)]),
                        Piece::Dot(u) => out.dots.push(frame.at(k, u)),
                    },
                )?;
            }
        }
    }
    Ok(())
}
