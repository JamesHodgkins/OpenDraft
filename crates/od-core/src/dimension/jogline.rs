//! The jog line of a linear dimension: a zigzag in the dimension line where
//! it is drawn out of scale, and the spacing of dimension lines (DIMSPACE).

use super::sketch::{along, dot, left, sub};
use super::{linear::line_direction, DimKind, Dimension};
use crate::Point;

/// `lines` with a zigzag of height `height` put in where the line through
/// `q1` along `dir` passes `at`. Segments that do not reach it are left as
/// they are.
pub(super) fn zigzag(
    lines: Vec<(Point, Point)>,
    at: Point,
    q1: Point,
    dir: (f64, f64),
    height: f64,
) -> Vec<(Point, Point)> {
    let m = along(q1, dir, dot(sub(at, q1), dir));
    let n = left(dir);
    let w = height / 2.0;
    let points = [
        along(m, dir, -w),
        along(along(m, dir, -w / 4.0), n, height / 2.0),
        along(along(m, dir, w / 4.0), n, -height / 2.0),
        along(m, dir, w),
    ];
    let mut out = Vec::new();
    for (a, b) in lines {
        let (ta, tb) = (dot(sub(a, m), dir), dot(sub(b, m), dir));
        if ta.min(tb) < -w && ta.max(tb) > w {
            let (first, last) = if ta < tb { (a, b) } else { (b, a) };
            out.push((first, points[0]));
            out.extend(points.windows(2).map(|p| (p[0], p[1])));
            out.push((points[3], last));
        } else {
            out.push((a, b));
        }
    }
    out
}

impl Dimension {
    /// With a jog line put in at `at` (`None` takes it out). Only linear and
    /// aligned dimensions have one.
    pub fn with_jog_line(&self, at: Option<Point>) -> Option<Dimension> {
        if !matches!(self.kind, DimKind::Linear { .. }) {
            return None;
        }
        Some(Dimension {
            jog_line: at,
            ..self.clone()
        })
    }

    /// Where a jog line goes when none is given: a quarter of the way along.
    pub fn default_jog_point(&self) -> Option<Point> {
        let DimKind::Linear { p1, p2, .. } = &self.kind else {
            return None;
        };
        Some(Point::new(
            p1.x + (p2.x - p1.x) / 4.0,
            p1.y + (p2.y - p1.y) / 4.0,
        ))
    }

    /// How far this dimension's line lies from `base`'s, measured across
    /// `base`'s line (positive on its left), when the two run the same way.
    pub fn line_offset_from(&self, base: &Dimension) -> Option<f64> {
        let (DimKind::Linear { line_point: a, .. }, DimKind::Linear { line_point: b, .. }) =
            (&self.kind, &base.kind)
        else {
            return None;
        };
        let (d, e) = (line_direction(&self.kind)?, line_direction(&base.kind)?);
        if (d.0 * e.1 - d.1 * e.0).abs() > 1e-6 {
            return None;
        }
        Some(dot(sub(*a, *b), left(e)))
    }

    /// With its line moved to lie `offset` from `base`'s (see
    /// [`Dimension::line_offset_from`]).
    pub fn with_line_offset(&self, base: &Dimension, offset: f64) -> Option<Dimension> {
        let now = self.line_offset_from(base)?;
        let n = left(line_direction(&base.kind)?);
        let mut out = self.clone();
        if let DimKind::Linear { line_point, .. } = &mut out.kind {
            *line_point = along(*line_point, n, offset - now);
        }
        out.text_pos = out.text_pos.map(|p| along(p, n, offset - now));
        Some(out)
    }
}

/// The offsets from the base line that give `now` (each dimension's offset
/// from the base) an even `spacing`, keeping which side and the order they
/// are in. A spacing of zero lines them all up with the base.
pub fn spaced_offsets(now: &[f64], spacing: f64) -> Vec<f64> {
    let mut order: Vec<usize> = (0..now.len()).collect();
    order.sort_by(|a, b| now[*a].abs().total_cmp(&now[*b].abs()));
    let mut out = vec![0.0; now.len()];
    let (mut above, mut below) = (0.0, 0.0);
    for i in order {
        out[i] = if now[i] >= 0.0 {
            above += spacing;
            above
        } else {
            below -= spacing;
            below
        };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DimStyle;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn at_height(y: f64) -> Dimension {
        Dimension::linear(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(5.0, y),
            0.0,
            &DimStyle::default(),
        )
    }

    #[test]
    fn a_zigzag_replaces_a_line_that_passes_it() {
        let whole = vec![(p(0.0, 5.0), p(10.0, 5.0))];
        let lines = zigzag(whole, p(5.0, 7.0), p(0.0, 5.0), (1.0, 0.0), 3.0);
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0].1, p(3.5, 5.0));
        assert_eq!(lines[4].0, p(6.5, 5.0));
        let short = vec![(p(0.0, 5.0), p(2.0, 5.0))];
        let untouched = zigzag(short, p(5.0, 7.0), p(0.0, 5.0), (1.0, 0.0), 3.0);
        assert_eq!(untouched.len(), 1);
    }

    #[test]
    fn a_jog_line_is_drawn_only_on_linear_dimensions() {
        let plain = at_height(5.0);
        let jogged = plain.with_jog_line(Some(p(3.0, 5.0))).unwrap();
        assert!(jogged.picture().len() > plain.picture().len());
        let r = Dimension::radius(p(0.0, 0.0), p(5.0, 0.0), &DimStyle::default());
        assert!(r.with_jog_line(Some(p(1.0, 1.0))).is_none());
        let back = jogged.with_jog_line(None).unwrap();
        assert_eq!(back.picture(), plain.picture());
    }

    #[test]
    fn lines_are_spaced_from_the_base() {
        let base = at_height(5.0);
        let a = at_height(11.0);
        assert_eq!(a.line_offset_from(&base), Some(6.0));
        let moved = a.with_line_offset(&base, 10.0).unwrap();
        assert_eq!(moved.points()[2], p(5.0, 15.0));
        let across = Dimension::linear(
            p(0.0, 0.0),
            p(0.0, 10.0),
            p(5.0, 5.0),
            std::f64::consts::FRAC_PI_2,
            &DimStyle::default(),
        );
        assert_eq!(across.line_offset_from(&base), None);
    }

    #[test]
    fn spacing_keeps_the_side_and_the_order() {
        assert_eq!(spaced_offsets(&[9.0, 3.0, -4.0], 2.0), [4.0, 2.0, -2.0]);
        assert_eq!(spaced_offsets(&[9.0, -3.0], 0.0), [0.0, 0.0]);
    }
}
