//! Radius and diameter dimensions.

use std::f64::consts::FRAC_PI_2;

use super::font;
use super::sketch::{along, direction, dist, outside_box, readable, sub, unit, Sketch};
use super::{Arrow, Built, DimKind, Dimension};
use crate::Point;

pub(super) fn build(dim: &Dimension) -> Option<Built> {
    let DimKind::Radial {
        center,
        chord_point,
        diameter,
        leader_length,
    } = &dim.kind
    else {
        return None;
    };
    let (c, pt) = (*center, *chord_point);
    let r = dist(c, pt);
    let u = unit(sub(pt, c))?;
    let s = dim.style.resolved();
    let label = dim.label();
    let (h, g) = (s.text_height, s.text_gap);
    let size = if s.arrow == Arrow::None {
        0.0
    } else {
        s.arrow_size
    };
    let width = font::text_width(&label, h);
    let clearance = if label.is_empty() {
        0.0
    } else {
        width + 2.0 * g
    };
    let has_text = !label.is_empty();
    let far = Point::new(2.0 * c.x - pt.x, 2.0 * c.y - pt.y);

    // Room for the text on the line inside the circle.
    let room = if *diameter {
        2.0 * r - 2.0 * size
    } else {
        r - size
    };
    let text_in = match dim.text_pos {
        Some(pos) => dist(c, pos) <= r,
        None => s.text_inside || room >= clearance,
    };
    let horizontal = if text_in {
        s.inside_horizontal
    } else {
        s.outside_horizontal
    };
    let line_angle = u.1.atan2(u.0);
    let sx = |dx: f64| if dx < 0.0 { -1.0 } else { 1.0 };

    let (centre, text_angle) = match (dim.text_pos, text_in) {
        (Some(pos), _) => {
            let angle = if horizontal {
                0.0
            } else {
                readable(line_angle)
            };
            (pos, angle)
        }
        (None, true) => {
            let angle = if horizontal {
                0.0
            } else {
                readable(line_angle)
            };
            let on_line = if *diameter {
                c
            } else {
                along(c, u, (r - size) / 2.0)
            };
            let up = direction(angle + FRAC_PI_2);
            let lift = match s.text_above {
                0 => 0.0,
                4 => -(g + h / 2.0),
                _ => g + h / 2.0,
            };
            (along(on_line, up, lift), angle)
        }
        (None, false) => {
            let reach = r + leader_length.max(0.0);
            let end = along(c, u, reach);
            if horizontal {
                let dx = sx(u.0);
                let landing = Point::new(end.x + dx * size, end.y);
                (
                    Point::new(landing.x + dx * (g + width / 2.0), landing.y),
                    0.0,
                )
            } else {
                let angle = readable(line_angle);
                (along(end, u, g + width / 2.0), angle)
            }
        }
    };

    let mut out = Sketch::default();
    // Arrows touch the circle from inside.
    out.arrow(s.arrow, pt, (-u.0, -u.1), size);
    if *diameter {
        out.arrow(s.arrow, far, u, size);
    }
    // Centre mark.
    // Text centred on a diameter would sit on top of the mark.
    if s.centre_mark > 0.0 && !(*diameter && text_in && has_text && dim.text_pos.is_none()) {
        let m = s.centre_mark;
        out.line(Point::new(c.x - m, c.y), Point::new(c.x + m, c.y));
        out.line(Point::new(c.x, c.y - m), Point::new(c.x, c.y + m));
    }

    let breaks = text_in && has_text && s.text_above == 0 && dim.text_pos.is_none();
    let draw = |out: &mut Sketch, a: Point, b: Point| {
        if breaks {
            for (x, y) in outside_box(a, b, centre, text_angle, (width / 2.0 + g, h / 2.0 + g)) {
                out.line(x, y);
            }
        } else {
            out.line(a, b);
        }
    };
    let inner_start = if *diameter { far } else { c };
    if text_in || s.force_line {
        draw(&mut out, inner_start, pt);
    }
    if !text_in {
        // From the circle out to the text, with a landing for horizontal text.
        let stop = |toward: Point| {
            let dir = unit(sub(toward, pt)).unwrap_or(u);
            along(centre, dir, -(width / 2.0 + g))
        };
        if horizontal {
            let dx = sx(centre.x - pt.x);
            let edge = Point::new(centre.x - dx * (width / 2.0 + g), centre.y);
            if dim.text_pos.is_some() {
                out.line(pt, edge);
            } else {
                out.polyline(vec![pt, Point::new(edge.x - dx * size, edge.y), edge]);
            }
        } else {
            out.line(
                pt,
                if dim.text_pos.is_some() {
                    stop(centre)
                } else {
                    along(c, u, r + leader_length.max(0.0))
                },
            );
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

    fn has_line(d: &Dimension, f: impl Fn(Point, Point) -> bool) -> bool {
        d.picture().into_iter().any(|e| match e {
            Entity::Line { start, end } => f(start, end),
            _ => false,
        })
    }

    #[test]
    fn a_radius_dimension_has_a_line_to_the_centre() {
        let d = Dimension::radius(p(0.0, 0.0), p(50.0, 0.0), &DimStyle::default());
        assert!(has_line(&d, |a, b| {
            let (lo, hi) = (a.x.min(b.x), a.x.max(b.x));
            a.y.abs() < 1e-9 && b.y.abs() < 1e-9 && lo.abs() < 1e-9 && (hi - 50.0).abs() < 1e-9
        }));
        let c = d.text_centre().unwrap();
        assert!(c.x > 0.0 && c.x < 50.0, "text between centre and circle");
    }

    #[test]
    fn a_small_circle_puts_the_text_outside() {
        let d = Dimension::radius(p(0.0, 0.0), p(3.0, 0.0), &DimStyle::default());
        let c = d.text_centre().unwrap();
        assert!(c.x > 3.0, "outside the circle: {c:?}");
    }

    #[test]
    fn a_diameter_has_two_arrows_and_text_at_the_centre() {
        let style = DimStyle {
            text_above: 0,
            ..DimStyle::default()
        };
        let d = Dimension::diameter(p(10.0, 10.0), p(40.0, 10.0), &style);
        let arrows = d
            .picture()
            .iter()
            .filter(|e| matches!(e, Entity::Polyline { widths, .. } if widths.first().is_some_and(|w| w.0 > 0.0)))
            .count();
        assert_eq!(arrows, 2);
        let c = d.text_centre().unwrap();
        assert!((c.x - 10.0).abs() < 1e-9 && (c.y - 10.0).abs() < 1e-9);
    }

    #[test]
    fn the_centre_mark_can_be_turned_off() {
        let on = Dimension::radius(p(0.0, 0.0), p(50.0, 0.0), &DimStyle::default());
        let off = Dimension::radius(
            p(0.0, 0.0),
            p(50.0, 0.0),
            &DimStyle {
                centre_mark: 0.0,
                ..DimStyle::default()
            },
        );
        assert!(on.picture().len() > off.picture().len());
    }

    #[test]
    fn user_text_outside_gets_a_leader_to_it() {
        let mut d = Dimension::radius(p(0.0, 0.0), p(50.0, 0.0), &DimStyle::default());
        d.text_pos = Some(p(80.0, 30.0));
        assert_eq!(d.text_centre(), Some(p(80.0, 30.0)));
        assert!(has_line(&d, |a, _| a == p(50.0, 0.0)));
    }
}
