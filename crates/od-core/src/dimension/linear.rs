//! Aligned and rotated dimensions.

use std::f64::consts::FRAC_PI_2;

use super::fit::fit;
use super::font;
use super::sketch::{
    along, cross, direction, dist, dot, left, outside_box, readable, sub, unit, Sketch,
};
use super::{Built, DimKind, Dimension};
use crate::Point;

/// The dimension line's direction (turned so the second foot lies ahead of
/// the first) and the points where the extension lines meet it.
struct Frame {
    dir: (f64, f64),
    q1: Point,
    q2: Point,
}

fn frame(kind: &DimKind) -> Option<Frame> {
    let DimKind::Linear {
        p1,
        p2,
        line_point,
        rotation,
        oblique,
    } = kind
    else {
        return None;
    };
    let angle = match rotation {
        Some(a) => *a,
        None => {
            let v = sub(*p2, *p1);
            if v.0.hypot(v.1) <= 1e-12 {
                return None;
            }
            v.1.atan2(v.0)
        }
    };
    let dir = direction(angle);
    let ext = oblique.map_or_else(|| left(dir), direction);
    let c = cross(ext, dir);
    if c.abs() < 1e-9 || !angle.is_finite() {
        return None;
    }
    let foot = |p: Point| {
        let t = cross(sub(*line_point, p), dir) / c;
        along(p, ext, t)
    };
    let (q1, q2) = (foot(*p1), foot(*p2));
    let dir = if dot(sub(q2, q1), dir) < 0.0 {
        (-dir.0, -dir.1)
    } else {
        dir
    };
    Some(Frame { dir, q1, q2 })
}

/// The direction the dimension line runs.
pub(super) fn line_direction(kind: &DimKind) -> Option<(f64, f64)> {
    frame(kind).map(|f| f.dir)
}

/// The middle of the dimension line, where its grip sits.
pub(super) fn line_middle(kind: &DimKind) -> Option<Point> {
    let Frame { q1, q2, .. } = frame(kind)?;
    Some(Point::new((q1.x + q2.x) / 2.0, (q1.y + q2.y) / 2.0))
}

/// Aligned: the distance between the points. Rotated: how far apart they
/// are along the dimension line.
pub(super) fn measurement(kind: &DimKind) -> Option<f64> {
    let DimKind::Linear {
        p1, p2, rotation, ..
    } = kind
    else {
        return None;
    };
    match rotation {
        None => {
            let d = dist(*p1, *p2);
            (d > 1e-12).then_some(d)
        }
        Some(a) => Some(dot(sub(*p2, *p1), direction(*a)).abs()),
    }
}

pub(super) fn build(dim: &Dimension) -> Option<Built> {
    let DimKind::Linear {
        p1, p2, line_point, ..
    } = &dim.kind
    else {
        return None;
    };
    let Frame { dir, q1, q2 } = frame(&dim.kind)?;
    let s = dim.style.resolved();
    let label = dim.label();
    let (h, g) = (s.text_height, s.text_gap);
    let size = if s.arrow == super::Arrow::None {
        0.0
    } else {
        s.arrow_size
    };
    let span = dist(q1, q2);
    let width = font::text_width(&label, h);
    let clearance = if label.is_empty() {
        0.0
    } else {
        width + 2.0 * g
    };
    let line_angle = dir.1.atan2(dir.0);

    // Where the text sits relative to the first foot, along the line.
    let along_of = |pt: Point| dot(sub(pt, q1), dir);
    let (arrows_in, text_in) = match dim.text_pos {
        Some(pos) => {
            let inside = (0.0..=span).contains(&along_of(pos));
            let (a, t) = fit(span, size, clearance, s.fit, inside);
            (if inside { a } else { span >= 2.0 * size }, t && inside)
        }
        None => fit(span, size, clearance, s.fit, s.text_inside),
    };
    // DIMJUST 3 and 4 put the text over an extension line, along it.
    let over = s.text_just >= 3;
    let (arrows_in, text_in) = if over {
        (span >= 2.0 * size, false)
    } else {
        (arrows_in, text_in)
    };
    let over_foot = if s.text_just == 4 {
        (*p2, q2)
    } else {
        (*p1, q1)
    };
    let over_dir = unit(sub(over_foot.1, over_foot.0)).unwrap_or(dir);
    let horizontal = if text_in {
        s.inside_horizontal
    } else {
        s.outside_horizontal
    };
    let text_angle = if over {
        readable(over_dir.1.atan2(over_dir.0))
    } else if horizontal {
        0.0
    } else {
        readable(line_angle)
    };
    let text_angle = dim.text_angle.unwrap_or(text_angle);
    let up = direction(text_angle + FRAC_PI_2);
    let sloped = horizontal && dir.1.abs() > 1e-9;
    let mid_points = Point::new((p1.x + p2.x) / 2.0, (p1.y + p2.y) / 2.0);
    let outward = if dot(sub(*line_point, mid_points), left(dir)) < 0.0 {
        -1.0
    } else {
        1.0
    };
    let lift = g + h / 2.0;
    let offset = |above: u8| -> (f64, f64) {
        match above {
            1 | 3 => (up.0 * lift, up.1 * lift),
            4 => (-up.0 * lift, -up.1 * lift),
            2 => {
                let n = left(dir);
                (n.0 * outward * lift, n.1 * outward * lift)
            }
            _ => (0.0, 0.0),
        }
    };
    // Text sitting on the line (not lifted off it) breaks the line.
    let centred = s.text_above == 0 || (text_in && sloped);
    let tc_along = if text_in {
        let lead = if arrows_in { size } else { 0.0 } + g + width / 2.0;
        match s.text_just {
            1 => lead,
            2 => span - lead,
            _ => span / 2.0,
        }
        .clamp(0.0, span.max(0.0))
    } else if s.text_just == 1 {
        -(size + g + width / 2.0)
    } else {
        span + size + g + width / 2.0
    };
    let default_centre = if over {
        along(over_foot.1, over_dir, s.ext_extend + g + width / 2.0)
    } else {
        let on_line = along(q1, dir, tc_along);
        let (ox, oy) = if centred && text_in {
            (0.0, 0.0)
        } else {
            offset(s.text_above)
        };
        Point::new(on_line.x + ox, on_line.y + oy)
    };
    let centre = dim.text_pos.unwrap_or(default_centre);
    let tc = along_of(centre);

    let mut out = Sketch::default();
    // Extension lines.
    for (i, (p, q)) in [(*p1, q1), (*p2, q2)].into_iter().enumerate() {
        if s.suppress_ext[i] || dist(p, q) <= s.ext_offset {
            continue;
        }
        if let Some(u) = unit(sub(q, p)) {
            out.line(along(p, u, s.ext_offset), along(q, u, s.ext_extend));
        }
    }
    // Arrows.
    let flip = |v: (f64, f64)| (-v.0, -v.1);
    let (back1, back2) = if arrows_in {
        (dir, flip(dir))
    } else {
        (flip(dir), dir)
    };
    out.arrow(s.arrow, q1, back1, size);
    out.arrow(s.arrow, q2, back2, size);

    // Dimension line, in pieces to draw.
    let mut lines: Vec<(Point, Point)> = Vec::new();
    if arrows_in || text_in || s.force_line {
        let mid = along(
            q1,
            dir,
            if text_in {
                tc.clamp(0.0, span)
            } else {
                span / 2.0
            },
        );
        match s.suppress_line {
            [false, false] => lines.push((q1, q2)),
            [first, second] => {
                if !first {
                    lines.push((q1, mid));
                }
                if !second {
                    lines.push((mid, q2));
                }
            }
        }
    }
    let stub = 2.0 * size;
    if !arrows_in && stub > 0.0 {
        lines.push((q1, along(q1, dir, -stub)));
        lines.push((q2, along(q2, dir, stub)));
    }
    // A line that follows text moved off the end (DIMTMOVE 0) or outside.
    let follows = (dim.text_pos.is_none() || s.text_move == 0) && !over;
    if !text_in && !label.is_empty() && follows {
        if tc > span {
            let end = if centred {
                tc - width / 2.0 - g
            } else {
                tc + width / 2.0 + g
            };
            lines.push((q2, along(q1, dir, end.max(span))));
        } else if tc < 0.0 {
            let end = if centred {
                tc + width / 2.0 + g
            } else {
                tc - width / 2.0 - g
            };
            lines.push((q1, along(q1, dir, end.min(0.0))));
        }
    }
    let has_text = !label.is_empty();
    let breaks = has_text && text_in && centred && dim.text_pos.is_none();
    for (a, b) in lines {
        if breaks {
            for (x, y) in outside_box(a, b, centre, text_angle, (width / 2.0 + g, h / 2.0 + g)) {
                out.line(x, y);
            }
        } else {
            out.line(a, b);
        }
    }
    // A leader from the line to text left away from it (DIMTMOVE 1).
    if dim.text_pos.is_some() && s.text_move == 1 && has_text {
        let foot = along(q1, dir, tc.clamp(0.0, span));
        let gap = dist(foot, centre);
        if gap > h {
            if let Some(u) = unit(sub(centre, foot)) {
                out.line(foot, along(centre, u, -(h / 2.0 + g)));
            }
        }
    }
    if has_text {
        out.text(&label, h, centre, text_angle, s.text_style);
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

    fn style() -> DimStyle {
        DimStyle::default()
    }

    fn lines(d: &Dimension) -> Vec<(Point, Point)> {
        d.picture()
            .into_iter()
            .filter_map(|e| match e {
                Entity::Line { start, end } => Some((start, end)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn aligned_measures_the_straight_distance() {
        let d = Dimension::aligned(p(0.0, 0.0), p(3.0, 4.0), p(0.0, 5.0), &style());
        assert_eq!(d.measurement(), Some(5.0));
    }

    #[test]
    fn rotated_measures_along_its_own_direction() {
        let h = Dimension::linear(p(0.0, 0.0), p(3.0, 4.0), p(0.0, 9.0), 0.0, &style());
        assert!((h.measurement().unwrap() - 3.0).abs() < 1e-12);
        let v = Dimension::linear(p(0.0, 0.0), p(3.0, 4.0), p(9.0, 0.0), FRAC_PI_2, &style());
        assert!((v.measurement().unwrap() - 4.0).abs() < 1e-12);
    }

    #[test]
    fn the_feet_lie_on_the_dimension_line() {
        let d = Dimension::aligned(p(0.0, 0.0), p(10.0, 0.0), p(2.0, 6.0), &style());
        let f = frame(&d.kind).unwrap();
        assert!((f.q1.y - 6.0).abs() < 1e-9 && (f.q2.y - 6.0).abs() < 1e-9);
        assert!((f.q1.x - 0.0).abs() < 1e-9 && (f.q2.x - 10.0).abs() < 1e-9);
    }

    #[test]
    fn extension_lines_leave_a_gap_and_overshoot() {
        let d = Dimension::aligned(p(0.0, 0.0), p(100.0, 0.0), p(0.0, 10.0), &style());
        let ext: Vec<_> = lines(&d)
            .into_iter()
            .filter(|(a, b)| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() > 5.0)
            .collect();
        assert_eq!(ext.len(), 2);
        let first = ext.iter().find(|(a, _)| a.x.abs() < 1e-9).unwrap();
        assert!((first.0.y - 0.625).abs() < 1e-9);
        assert!((first.1.y - 11.25).abs() < 1e-9);
    }

    #[test]
    fn text_above_the_line_leaves_the_line_whole() {
        let d = Dimension::aligned(p(0.0, 0.0), p(100.0, 0.0), p(0.0, 10.0), &style());
        let on_line = lines(&d)
            .iter()
            .filter(|(a, b)| (a.y - 10.0).abs() < 1e-9 && (b.y - 10.0).abs() < 1e-9)
            .count();
        assert_eq!(on_line, 1);
        let c = d.text_centre().unwrap();
        assert!(c.y > 10.0, "above the line");
        assert!((c.x - 50.0).abs() < 1e-9);
    }

    #[test]
    fn centred_text_breaks_the_line() {
        let s = DimStyle {
            text_above: 0,
            ..style()
        };
        let d = Dimension::aligned(p(0.0, 0.0), p(100.0, 0.0), p(0.0, 10.0), &s);
        let on_line: Vec<_> = lines(&d)
            .into_iter()
            .filter(|(a, b)| (a.y - 10.0).abs() < 1e-9 && (b.y - 10.0).abs() < 1e-9)
            .collect();
        assert_eq!(on_line.len(), 2);
        let gap = on_line[1].0.x - on_line[0].1.x;
        let w = font::text_width("100.00", 2.5);
        assert!((gap - (w + 2.0 * 0.625)).abs() < 1e-9);
    }

    #[test]
    fn a_short_dimension_moves_its_text_outside() {
        let d = Dimension::aligned(p(0.0, 0.0), p(4.0, 0.0), p(0.0, 5.0), &style());
        let c = d.text_centre().unwrap();
        assert!(c.x > 4.0, "past the second extension line");
    }

    #[test]
    fn a_user_placed_text_stays_where_it_was_put() {
        let mut d = Dimension::aligned(p(0.0, 0.0), p(100.0, 0.0), p(0.0, 10.0), &style());
        d.text_pos = Some(p(30.0, 20.0));
        assert_eq!(d.text_centre(), Some(p(30.0, 20.0)));
    }

    #[test]
    fn coincident_points_make_no_aligned_dimension() {
        let d = Dimension::aligned(p(1.0, 1.0), p(1.0, 1.0), p(0.0, 5.0), &style());
        assert!(d.picture().is_empty());
    }

    #[test]
    fn text_over_an_extension_line_lies_along_it_beyond_its_end() {
        let mut d = Dimension::aligned(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 5.0), &style());
        d.style.text_just = 3;
        let centre = d.text_centre().unwrap();
        // The first extension line runs up from (0, 0): the text continues it.
        assert!(centre.x.abs() < 1e-9 && centre.y > 5.0, "{centre:?}");
        d.style.text_just = 4;
        let centre = d.text_centre().unwrap();
        assert!(
            (centre.x - 10.0).abs() < 1e-9 && centre.y > 5.0,
            "{centre:?}"
        );
    }

    #[test]
    fn oblique_extension_lines_are_slanted() {
        let mut d = Dimension::linear(p(0.0, 0.0), p(100.0, 0.0), p(0.0, 10.0), 0.0, &style());
        if let DimKind::Linear { oblique, .. } = &mut d.kind {
            *oblique = Some(std::f64::consts::FRAC_PI_4);
        }
        let slanted = lines(&d)
            .iter()
            .any(|(a, b)| ((b.y - a.y) - (b.x - a.x)).abs() < 1e-9 && (b.y - a.y).abs() > 5.0);
        assert!(slanted);
    }

    #[test]
    fn every_edge_case_value_is_finite() {
        for (a, b, c) in [
            (p(0.0, 0.0), p(1e-9, 0.0), p(0.0, 5.0)),
            (p(0.0, 0.0), p(10.0, 0.0), p(5.0, 0.0)),
            (p(-5.0, -5.0), p(5.0, 5.0), p(0.0, 0.0)),
        ] {
            let d = Dimension::aligned(a, b, c, &style());
            for e in d.picture() {
                let bb = e.bounds();
                assert!(bb.min.x.is_finite() && bb.max.y.is_finite());
            }
        }
    }
}
