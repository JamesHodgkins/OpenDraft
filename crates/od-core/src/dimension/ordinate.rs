//! Ordinate dimensions: the X or Y coordinate of a feature, on a leader that
//! runs along the other axis to the text. Coordinates are those of the
//! drawing's origin (there is no user coordinate system).

use std::f64::consts::FRAC_PI_2;

use super::font;
use super::sketch::{readable, Sketch};
use super::{Built, DimKind, Dimension};
use crate::Point;

/// The coordinate measured: X for an X datum dimension (a leader running
/// up or down), Y otherwise.
pub(super) fn measurement(kind: &DimKind) -> Option<f64> {
    let DimKind::Ordinate {
        feature, x_datum, ..
    } = kind
    else {
        return None;
    };
    Some(if *x_datum { feature.x } else { feature.y })
}

/// Which datum a leader to `leader_end` suggests: the leader runs up or
/// down (X datum) when `leader_end` is further from the feature in Y than
/// in X.
pub fn suggested_x_datum(feature: Point, leader_end: Point) -> bool {
    (leader_end.y - feature.y).abs() >= (leader_end.x - feature.x).abs()
}

pub(super) fn build(dim: &Dimension) -> Option<Built> {
    let DimKind::Ordinate {
        feature,
        leader_end,
        x_datum,
    } = &dim.kind
    else {
        return None;
    };
    let s = dim.style.resolved();
    let label = dim.label();
    let (h, g) = (s.text_height, s.text_gap);
    let width = font::text_width(&label, h);
    // The leader runs along `axis` (the unit step of its main direction)
    // from the feature, standing off by the extension offset, to the end.
    let (fx, fy) = (feature.x, feature.y);
    let (ex, ey) = (leader_end.x, leader_end.y);
    let (axis, along_len, across) = if *x_datum {
        let s = if ey >= fy { 1.0 } else { -1.0 };
        ((0.0, s), (ey - fy).abs(), ex - fx)
    } else {
        let s = if ex >= fx { 1.0 } else { -1.0 };
        ((s, 0.0), (ex - fx).abs(), ey - fy)
    };
    let at = |t: f64, side: f64| {
        let n = (-axis.1, axis.0);
        Point::new(fx + axis.0 * t + n.0 * side, fy + axis.1 * t + n.1 * side)
    };
    let start = s.ext_offset.min(along_len);
    let mut out = Sketch::default();
    if across.abs() <= 1e-9 || along_len <= start {
        out.line(at(start, 0.0), at(along_len.max(start), across));
    } else {
        // A jog halfway sideways to the text's line.
        let jog = ((along_len + start) / 2.0).max(start);
        let half = (s.text_height / 2.0).min((along_len - start) / 2.0);
        out.polyline(vec![
            at(start, 0.0),
            at(jog - half, 0.0),
            at(jog + half, across),
            at(along_len, across),
        ]);
    }
    let end = at(along_len, across);
    let run = if *x_datum { FRAC_PI_2 } else { 0.0 };
    let angle = dim.text_angle.unwrap_or_else(|| readable(run));
    let beyond = g + width / 2.0;
    let centre = dim
        .text_pos
        .unwrap_or_else(|| Point::new(end.x + axis.0 * beyond, end.y + axis.1 * beyond));
    if !label.is_empty() {
        out.text(&label, h, centre, angle, s.text_style);
    }
    Some(Built {
        pieces: out.pieces,
        text_centre: centre,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DimStyle, Entity};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn it_measures_the_coordinate_of_the_datum() {
        let style = DimStyle::default();
        let x = Dimension::ordinate(p(12.5, 3.0), p(12.5, 20.0), true, &style);
        assert_eq!(x.measurement(), Some(12.5));
        assert_eq!(x.label(), "12.50");
        let y = Dimension::ordinate(p(12.5, 3.0), p(30.0, 3.0), false, &style);
        assert_eq!(y.measurement(), Some(3.0));
    }

    #[test]
    fn the_datum_follows_the_leader_unless_forced() {
        assert!(suggested_x_datum(p(0.0, 0.0), p(1.0, 5.0)));
        assert!(!suggested_x_datum(p(0.0, 0.0), p(5.0, 1.0)));
    }

    #[test]
    fn a_straight_leader_is_one_line_and_an_offset_one_jogs() {
        let style = DimStyle::default();
        let lines = |d: &Dimension| {
            d.picture()
                .iter()
                .filter(|e| matches!(e, Entity::Line { .. } | Entity::Polyline { .. }))
                .count()
        };
        let straight = Dimension::ordinate(p(5.0, 5.0), p(5.0, 25.0), true, &style);
        assert!(straight
            .picture()
            .iter()
            .any(|e| matches!(e, Entity::Line { start, end } if start.x == 5.0 && end.x == 5.0)));
        let jogged = Dimension::ordinate(p(5.0, 5.0), p(9.0, 25.0), true, &style);
        assert!(jogged
            .picture()
            .iter()
            .any(|e| matches!(e, Entity::Polyline { points, .. } if points.len() == 4)));
        assert!(lines(&jogged) >= 1);
    }

    #[test]
    fn the_text_sits_beyond_the_end_of_the_leader() {
        let style = DimStyle::default();
        let d = Dimension::ordinate(p(5.0, 5.0), p(5.0, 25.0), true, &style);
        assert!(d.text_centre().unwrap().y > 25.0);
        let d = Dimension::ordinate(p(5.0, 5.0), p(25.0, 5.0), false, &style);
        assert!(d.text_centre().unwrap().x > 25.0);
    }

    #[test]
    fn a_leader_shorter_than_its_offset_still_draws() {
        let style = DimStyle::default();
        let d = Dimension::ordinate(p(5.0, 5.0), p(5.0, 5.1), true, &style);
        assert!(!d.picture().is_empty());
    }
}
