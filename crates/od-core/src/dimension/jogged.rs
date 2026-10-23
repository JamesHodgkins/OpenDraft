//! Jogged radius dimensions: the line heads for a centre shown elsewhere,
//! with a zigzag where the real centre is too far off the drawing to reach.

use super::font;
use super::sketch::{along, dist, dot, left, readable, sub, unit, Sketch};
use super::{Arrow, Built, DimKind, Dimension};
use crate::Point;

pub(super) fn build(dim: &Dimension) -> Option<Built> {
    let DimKind::Radial {
        chord_point,
        jog: Some((shown, at)),
        ..
    } = &dim.kind
    else {
        return None;
    };
    let s = dim.style.resolved();
    let label = dim.label();
    let (h, g) = (s.text_height, s.text_gap);
    let size = if s.arrow == Arrow::None {
        0.0
    } else {
        s.arrow_size
    };
    let width = font::text_width(&label, h);
    let pt = *chord_point;
    let dir = unit(sub(*shown, pt))?;
    let normal = left(dir);
    // The line runs to the shown centre, or on to the text put elsewhere.
    let reach = dist(pt, *shown);
    let jog_half = size.max(h / 2.0);
    let t_jog =
        dot(sub(*at, pt), dir).clamp(size + jog_half, (reach - jog_half).max(size + jog_half));
    let a = along(pt, dir, t_jog - jog_half);
    let b = along(pt, dir, t_jog + jog_half);
    let peak = |t: f64, side: f64| {
        let on = along(pt, dir, t);
        along(on, normal, side * jog_half)
    };
    let end = along(pt, dir, reach.max(t_jog + jog_half));

    let mut out = Sketch::default();
    out.arrow(s.arrow, pt, dir, size);
    let sx = if dir.0 < 0.0 { -1.0 } else { 1.0 };
    let (centre, angle, stop) = match dim.text_pos {
        Some(pos) => {
            let toward = unit(sub(pos, b)).unwrap_or(dir);
            (pos, 0.0, along(pos, toward, -(width / 2.0 + g)))
        }
        None if s.outside_horizontal => {
            let landing = Point::new(end.x + sx * size, end.y);
            (
                Point::new(landing.x + sx * (g + width / 2.0), landing.y),
                0.0,
                landing,
            )
        }
        None => (
            along(end, dir, g + width / 2.0),
            readable(dir.1.atan2(dir.0)),
            end,
        ),
    };
    let mut path = vec![
        pt,
        a,
        peak(t_jog - jog_half / 2.0, 1.0),
        peak(t_jog + jog_half / 2.0, -1.0),
        b,
    ];
    if dim.text_pos.is_none() {
        path.push(end);
    }
    path.push(stop);
    path.dedup();
    out.polyline(path);
    if !label.is_empty() {
        out.text(
            &label,
            h,
            centre,
            dim.text_angle.unwrap_or(angle),
            s.text_style,
        );
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

    fn jogged() -> Dimension {
        Dimension::jogged(
            p(0.0, 0.0),
            p(50.0, 0.0),
            p(30.0, 4.0),
            p(40.0, 2.0),
            &DimStyle::default(),
        )
    }

    #[test]
    fn it_still_measures_the_real_radius() {
        assert_eq!(jogged().measurement(), Some(50.0));
        assert_eq!(jogged().label(), "R50.00");
    }

    #[test]
    fn the_line_zigzags_toward_the_shown_centre() {
        let pieces = jogged().picture();
        let points = pieces
            .iter()
            .find_map(|e| match e {
                Entity::Polyline { points, .. } if points.len() >= 6 => Some(points.clone()),
                _ => None,
            })
            .expect("a jogged polyline");
        assert_eq!(points[0], p(50.0, 0.0));
        // The first leg heads toward (30, 4): up and to the left.
        assert!(points[1].x < 50.0 && points[1].y > 0.0);
        assert!(pieces.iter().any(|e| matches!(e, Entity::MText(_))));
    }

    #[test]
    fn it_moves_with_its_points_and_shows_grips_for_the_jog() {
        let moved = jogged().translated(10.0, 0.0);
        assert_eq!(moved.points()[2], p(40.0, 4.0));
        assert_eq!(jogged().grips().len(), 5, "four points and the text");
        let dragged = jogged().with_grip_moved(3, p(41.0, 2.0)).unwrap();
        assert_eq!(dragged.points()[3], p(41.0, 2.0));
    }

    #[test]
    fn a_shown_centre_on_the_circle_point_draws_nothing() {
        let d = Dimension::jogged(
            p(0.0, 0.0),
            p(50.0, 0.0),
            p(50.0, 0.0),
            p(40.0, 2.0),
            &DimStyle::default(),
        );
        assert!(d.picture().is_empty());
    }
}
