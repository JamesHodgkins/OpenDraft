//! Laying a linetype pattern along a path, by distance travelled.
//!
//! [`fit`] works in one dimension: given a path length it says which stretches
//! are drawn. [`dash_path`] maps that onto a flattened path. Both follow the
//! documented rules for AutoCAD's A-type patterns (guide, section 9):
//!
//! - an open path starts and ends with a dash (or a dot, for a pattern that
//!   starts with one), and the pattern is fitted between;
//! - a path too short to hold one repeat is drawn continuously;
//! - a closed path is fitted around its length so there is no odd seam.
//!
//! AutoCAD does not publish its exact fitting arithmetic. Ours is a choice
//! that meets those rules (the end dashes take up the slack; a dot-first
//! pattern stretches its gaps; a closed path stretches the whole pattern), to
//! be checked against AutoCAD-made fixtures before anyone relies on it.

use super::StrokeElement;
use crate::Point;

/// More marks than this on one path is not worth making; the path is drawn
/// continuously instead.
pub const MAX_MARKS: usize = 200_000;

/// One drawn stretch of a path, as distances from its start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mark {
    Dash { from: f64, to: f64 },
    Dot(f64),
}

/// Which stretches of a path `length` long are drawn, or `None` when the
/// whole path is drawn continuously (no pattern, too short, a bad scale, or
/// too many marks). `scale` multiplies every element length.
pub fn fit(length: f64, closed: bool, elements: &[StrokeElement], scale: f64) -> Option<Vec<Mark>> {
    let period: f64 = elements.iter().map(|e| e.length()).sum::<f64>() * scale;
    if elements.is_empty()
        || !(length.is_finite() && length > 0.0)
        || !(scale.is_finite() && scale > 0.0)
        || !(period.is_finite() && period > 1e-12)
        || length < period
        || length / period > MAX_MARKS as f64 / elements.len() as f64
    {
        return None;
    }
    let mut marks = Vec::new();
    if closed {
        let repeats = (length / period).round().max(1.0);
        let stretch = length / (repeats * period);
        lay_out(
            &mut marks,
            elements,
            scale * stretch,
            1.0,
            repeats as usize,
            0.0,
        );
    } else {
        match elements[0] {
            StrokeElement::Dash(first) => {
                let first = first * scale;
                let repeats = ((length - first) / period).floor();
                if repeats < 1.0 {
                    return None;
                }
                let repeats = repeats as usize;
                let slack = length - (repeats as f64 * period + first);
                let shift = slack / 2.0;
                lay_out(&mut marks, elements, scale, 1.0, repeats, shift);
                // The closing dash, then both ends reach the path's ends.
                marks.push(Mark::Dash {
                    from: shift + repeats as f64 * period,
                    to: length,
                });
                if let Some(Mark::Dash { from, .. }) = marks.first_mut() {
                    *from = 0.0;
                }
            }
            StrokeElement::Dot => {
                let gaps: f64 = elements
                    .iter()
                    .filter_map(|e| match e {
                        StrokeElement::Gap(g) => Some(*g),
                        _ => None,
                    })
                    .sum::<f64>()
                    * scale;
                let repeats = (length / period).floor();
                if gaps <= 0.0 || repeats < 1.0 {
                    return None;
                }
                let slack = length - repeats * period;
                let stretch = 1.0 + slack / (repeats * gaps);
                lay_out(&mut marks, elements, scale, stretch, repeats as usize, 0.0);
                marks.push(Mark::Dot(length));
            }
            // Not a fitted A pattern: lay it from the start and cut it off.
            StrokeElement::Gap(_) => {
                let repeats = (length / period).ceil() as usize;
                lay_out(&mut marks, elements, scale, 1.0, repeats, 0.0);
                marks.retain_mut(|m| match m {
                    Mark::Dash { from, to } => {
                        *to = to.min(length);
                        *from < length
                    }
                    Mark::Dot(at) => *at <= length,
                });
            }
        }
    }
    Some(merge(marks, length))
}

/// Place `repeats` copies of the pattern from `start`, each element scaled by
/// `scale` and each gap by a further `gap_stretch`.
fn lay_out(
    marks: &mut Vec<Mark>,
    elements: &[StrokeElement],
    scale: f64,
    gap_stretch: f64,
    repeats: usize,
    start: f64,
) {
    let mut at = start;
    for _ in 0..repeats {
        for element in elements {
            match *element {
                StrokeElement::Dash(d) => {
                    marks.push(Mark::Dash {
                        from: at,
                        to: at + d * scale,
                    });
                    at += d * scale;
                }
                StrokeElement::Gap(g) => at += g * scale * gap_stretch,
                StrokeElement::Dot => marks.push(Mark::Dot(at)),
            }
        }
    }
}

/// Join dashes that touch (a pattern that ends and starts with a dash).
fn merge(marks: Vec<Mark>, length: f64) -> Vec<Mark> {
    let eps = length * 1e-9;
    let mut out: Vec<Mark> = Vec::with_capacity(marks.len());
    for mark in marks {
        if let (Some(Mark::Dash { to: last, .. }), Mark::Dash { from, to }) = (out.last_mut(), mark)
        {
            if from - *last <= eps {
                *last = to;
                continue;
            }
        }
        out.push(mark);
    }
    out
}

/// A drawn piece of a flattened path.
#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    /// A run of points to stroke, at least two.
    Dash(Vec<Point>),
    /// A dot: where it falls, and the path's direction there.
    Dot { at: Point, along: Point },
}

/// The pieces of the flattened path `points` (a closed one does not repeat
/// its first point) drawn with `elements` at `scale`, or `None` for a path
/// drawn continuously.
pub fn dash_path(
    points: &[Point],
    closed: bool,
    elements: &[StrokeElement],
    scale: f64,
) -> Option<Vec<Piece>> {
    let path = Path::new(points, closed)?;
    let marks = fit(path.length(), closed, elements, scale)?;
    let mut pieces = Vec::with_capacity(marks.len());
    for mark in marks {
        match mark {
            Mark::Dash { from, to } => {
                let run = path.between(from, to);
                if run.len() >= 2 {
                    pieces.push(Piece::Dash(run));
                }
            }
            Mark::Dot(at) => {
                let (point, along) = path.at(at);
                pieces.push(Piece::Dot { at: point, along });
            }
        }
    }
    Some(pieces)
}

/// A flattened path with its running distances.
struct Path {
    points: Vec<Point>,
    /// `cum[i]` is the distance from the start to `points[i]`.
    cum: Vec<f64>,
}

impl Path {
    fn new(points: &[Point], closed: bool) -> Option<Self> {
        if points.len() < 2 {
            return None;
        }
        let mut pts = points.to_vec();
        if closed {
            pts.push(points[0]);
        }
        let mut cum = vec![0.0];
        for w in pts.windows(2) {
            let d = (w[1].x - w[0].x).hypot(w[1].y - w[0].y);
            cum.push(cum[cum.len() - 1] + d);
        }
        Some(Self { points: pts, cum })
    }

    fn length(&self) -> f64 {
        self.cum[self.cum.len() - 1]
    }

    /// The segment holding distance `s` (the last one for the very end).
    fn segment(&self, s: f64) -> usize {
        let i = self.cum.partition_point(|c| *c <= s);
        i.saturating_sub(1).min(self.points.len() - 2)
    }

    /// The point at distance `s` and the unit direction of the path there.
    fn at(&self, s: f64) -> (Point, Point) {
        let i = self.segment(s);
        let (a, b) = (self.points[i], self.points[i + 1]);
        let len = self.cum[i + 1] - self.cum[i];
        if len <= 0.0 {
            return (a, Point::new(1.0, 0.0));
        }
        let t = ((s - self.cum[i]) / len).clamp(0.0, 1.0);
        let at = Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        (at, Point::new((b.x - a.x) / len, (b.y - a.y) / len))
    }

    /// The path from distance `from` to `to`, keeping the corners between.
    fn between(&self, from: f64, to: f64) -> Vec<Point> {
        let mut run = vec![self.at(from).0];
        let first = self.segment(from);
        for i in first + 1..self.points.len() {
            if self.cum[i] >= to {
                break;
            }
            if self.cum[i] > from {
                run.push(self.points[i]);
            }
        }
        run.push(self.at(to).0);
        run
    }
}
