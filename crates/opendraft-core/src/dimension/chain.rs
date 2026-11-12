//! Baseline and continued dimensions: a new linear dimension that shares an
//! extension line origin with one that exists.

use super::linear::line_direction;
use super::sketch::{along, dot, left, sub};
use super::{DimKind, Dimension};
use crate::Point;

impl Dimension {
    /// The origin of extension line `second` (false: the first) of a linear
    /// or aligned dimension.
    pub fn origin(&self, second: bool) -> Option<Point> {
        match &self.kind {
            DimKind::Linear { p1, p2, .. } => Some(if second { *p2 } else { *p1 }),
            _ => None,
        }
    }

    /// Which extension line origin of this linear dimension is nearer `at`
    /// (true for the second).
    pub fn nearer_origin_is_second(&self, at: Point) -> bool {
        match (self.origin(false), self.origin(true)) {
            (Some(a), Some(b)) => {
                let d = |p: Point| (p.x - at.x).hypot(p.y - at.y);
                d(b) < d(a)
            }
            _ => false,
        }
    }

    /// A dimension from the origin `from_second` of this one to `to`, with
    /// its line a baseline spacing beyond this one's (DIMBASELINE). Linear
    /// and aligned dimensions only.
    pub fn baseline(&self, from_second: bool, to: Point) -> Option<Dimension> {
        let DimKind::Linear { line_point, .. } = &self.kind else {
            return None;
        };
        let (a, b) = (self.origin(false)?, self.origin(true)?);
        let dir = line_direction(&self.kind)?;
        let normal = left(dir);
        let middle = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        let side = if dot(sub(*line_point, middle), normal) < 0.0 {
            -1.0
        } else {
            1.0
        };
        let spacing = self.style.baseline_spacing * self.style.scale;
        let moved = along(*line_point, normal, side * spacing);
        self.chained(from_second, to, moved)
    }

    /// A dimension from the origin `from_second` of this one to `to`, on the
    /// same dimension line (DIMCONTINUE).
    pub fn continued(&self, from_second: bool, to: Point) -> Option<Dimension> {
        let DimKind::Linear { line_point, .. } = &self.kind else {
            return None;
        };
        self.chained(from_second, to, *line_point)
    }

    fn chained(&self, from_second: bool, to: Point, line_at: Point) -> Option<Dimension> {
        let origin = self.origin(from_second)?;
        if (origin.x - to.x).hypot(origin.y - to.y) <= 1e-9 {
            return None;
        }
        let mut out = self.clone();
        out.text = super::DimText::Measured;
        out.text_pos = None;
        out.text_angle = None;
        if let DimKind::Linear {
            p1, p2, line_point, ..
        } = &mut out.kind
        {
            *p1 = origin;
            *p2 = to;
            *line_point = line_at;
        }
        out.measurement()?;
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DimStyle;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn base() -> Dimension {
        Dimension::linear(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(5.0, 5.0),
            0.0,
            &DimStyle::default(),
        )
    }

    #[test]
    fn a_baseline_shares_the_first_origin_and_stacks_the_line_outward() {
        let next = base().baseline(false, p(25.0, 0.0)).unwrap();
        assert_eq!(next.points()[..2], [p(0.0, 0.0), p(25.0, 0.0)]);
        assert_eq!(next.points()[2], p(5.0, 5.0 + 3.75));
        assert_eq!(next.measurement(), Some(25.0));
        let below = Dimension::linear(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(5.0, -5.0),
            0.0,
            &DimStyle::default(),
        );
        assert_eq!(
            below.baseline(false, p(25.0, 0.0)).unwrap().points()[2],
            p(5.0, -8.75)
        );
    }

    #[test]
    fn a_continued_dimension_starts_where_the_last_ended_on_the_same_line() {
        let next = base().continued(true, p(25.0, 0.0)).unwrap();
        assert_eq!(next.points()[..2], [p(10.0, 0.0), p(25.0, 0.0)]);
        assert_eq!(next.points()[2], p(5.0, 5.0));
        assert_eq!(next.measurement(), Some(15.0));
    }

    #[test]
    fn typed_text_is_not_carried_and_other_kinds_refuse() {
        let mut d = base();
        d.text = super::super::DimText::Hidden;
        assert_eq!(
            d.baseline(false, p(20.0, 0.0)).unwrap().text,
            super::super::DimText::Measured
        );
        let r = Dimension::radius(p(0.0, 0.0), p(5.0, 0.0), &DimStyle::default());
        assert!(r.baseline(false, p(1.0, 1.0)).is_none());
        assert!(
            base().continued(true, p(10.0, 0.0)).is_none(),
            "nothing to measure"
        );
    }

    #[test]
    fn the_nearer_origin_is_found() {
        assert!(!base().nearer_origin_is_second(p(1.0, 3.0)));
        assert!(base().nearer_origin_is_second(p(9.0, 3.0)));
    }
}
