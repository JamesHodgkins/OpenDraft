//! The sample multileader the style dialogs preview: one leader with its
//! arrow, landing and text, built from nothing but the style. Pure geometry
//! as ordinary entities; the dialog only draws it.

use std::f64::consts::FRAC_PI_4;

use super::place::place_text;
use super::style::{LeaderAttachment, MLeaderContentType, MLeaderLineType, MLeaderStyle};
use crate::dimension::{arrow_pieces, Arrow};
use crate::{Entity, MText, Point};

/// Where the sample's first segment points when the style leaves it free.
const DEFAULT_ANGLE: f64 = FRAC_PI_4;
/// Corner-cutting rounds that turn the sample's points into a spline.
const SMOOTHING: usize = 4;

fn along(p: Point, angle: f64, length: f64) -> Point {
    Point::new(p.x + length * angle.cos(), p.y + length * angle.sin())
}

/// The leader's points, arrow end first, as `style` allows them: a second
/// segment only when more than two points are allowed and its angle is held.
fn path(style: &MLeaderStyle, unit: f64) -> Vec<Point> {
    let first = style.first_angle.unwrap_or(DEFAULT_ANGLE);
    let start = Point::new(0.0, 0.0);
    let bend = along(start, first, 8.0 * unit);
    let mut points = vec![start, bend];
    if let (Some(second), true) = (style.second_angle, style.max_points != Some(2)) {
        points.push(along(bend, second, 6.0 * unit));
    }
    points
}

/// The same points drawn as a smooth curve through the ends.
fn smooth(points: &[Point]) -> Vec<Point> {
    let mut curve = points.to_vec();
    if curve.len() == 2 {
        let (a, b) = (curve[0], curve[1]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        curve.insert(
            1,
            Point::new((a.x + b.x) / 2.0 - 0.18 * dy, (a.y + b.y) / 2.0 + 0.18 * dx),
        );
    }
    for _ in 0..SMOOTHING {
        let last = curve.len() - 1;
        let mut next = vec![curve[0]];
        for (i, pair) in curve.windows(2).enumerate() {
            let (a, b) = (pair[0], pair[1]);
            let cut = |t: f64| Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            if i > 0 {
                next.push(cut(0.25));
            }
            next.push(cut(0.75));
            if i + 1 == last {
                next.push(b);
            }
        }
        curve = next;
    }
    curve
}

fn polyline(points: Vec<Point>) -> Entity {
    Entity::Polyline {
        points,
        closed: false,
        bulges: Vec::new(),
        widths: Vec::new(),
    }
}

/// The sample for `style`, showing `text` (the style's default text, or a
/// stand-in when it has none) to the right of the leader. Everything is sized
/// from the style's own text height, arrow size and scale.
pub fn sample(style: &MLeaderStyle, text: &str) -> Vec<Entity> {
    let scale = style.overall_scale;
    let content = &style.content;
    let height = (content.text_height * scale).max(1e-6);
    let arrow = style.arrow_size * scale;
    let gap = content.landing_gap * scale;
    let mut pieces = Vec::new();

    let points = path(style, height);
    let end = points[points.len() - 1];
    let landing_end = if style.dogleg {
        along(end, 0.0, style.dogleg_length * scale)
    } else {
        end
    };
    if style.leader_type != MLeaderLineType::None {
        let line = if style.leader_type == MLeaderLineType::Spline {
            smooth(&points)
        } else {
            points.clone()
        };
        let (tip, next) = (line[0], line[1]);
        let (dx, dy) = (next.x - tip.x, next.y - tip.y);
        let length = dx.hypot(dy);
        if style.arrow != Arrow::None && length >= 2.0 * arrow && length > 0.0 {
            pieces.extend(arrow_pieces(
                style.arrow,
                tip,
                (dx / length, dy / length),
                arrow,
            ));
        }
        pieces.push(polyline(line));
        if landing_end != end {
            pieces.push(polyline(vec![end, landing_end]));
        }
    }

    if content.kind == MLeaderContentType::MText {
        let inset = if content.frame { gap } else { 0.0 };
        let (attachment, at) = place_text(
            content.left_attachment,
            true,
            landing_end,
            gap + inset,
            height,
        );
        let mut label = MText::new(at, height, text, content.text_style);
        label.attachment = attachment;
        let shape = Entity::MText(Box::new(label));
        let bounds = shape.bounds();
        let underline = matches!(
            content.left_attachment,
            LeaderAttachment::UnderlineBottomLine
                | LeaderAttachment::UnderlineTopLine
                | LeaderAttachment::UnderlineAllText
        );
        if content.frame {
            pieces.push(Entity::Rectangle {
                min: Point::new(bounds.min.x - gap, bounds.min.y - gap),
                max: Point::new(bounds.max.x + gap, bounds.max.y + gap),
            });
        } else if underline && style.leader_type != MLeaderLineType::None {
            let y = landing_end.y;
            pieces.push(polyline(vec![
                Point::new(landing_end.x, y),
                Point::new(bounds.max.x, y),
            ]));
        }
        pieces.push(shape);
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(pieces: &[Entity], keep: impl Fn(&Entity) -> bool) -> usize {
        pieces.iter().filter(|e| keep(e)).count()
    }

    fn finite(pieces: &[Entity]) -> bool {
        pieces.iter().all(|e| {
            let b = e.bounds();
            [b.min.x, b.min.y, b.max.x, b.max.y]
                .iter()
                .all(|v| v.is_finite())
        })
    }

    #[test]
    fn the_stock_style_draws_an_arrow_a_leader_a_landing_and_text() {
        let pieces = sample(&MLeaderStyle::standard(), "Note");
        assert!(finite(&pieces));
        assert_eq!(count(&pieces, |e| matches!(e, Entity::MText(_))), 1);
        // Arrow, leader line and landing.
        assert_eq!(count(&pieces, |e| matches!(e, Entity::Polyline { .. })), 3);
    }

    #[test]
    fn no_leader_type_leaves_only_the_text() {
        let style = MLeaderStyle {
            leader_type: MLeaderLineType::None,
            ..MLeaderStyle::standard()
        };
        let pieces = sample(&style, "Note");
        assert_eq!(pieces.len(), 1);
        assert!(matches!(pieces[0], Entity::MText(_)));
    }

    #[test]
    fn no_content_leaves_only_the_leader() {
        let mut style = MLeaderStyle::standard();
        style.content.kind = MLeaderContentType::None;
        let pieces = sample(&style, "Note");
        assert!(!pieces.iter().any(|e| matches!(e, Entity::MText(_))));
        assert!(!pieces.is_empty());
    }

    #[test]
    fn a_frame_surrounds_the_text_and_no_landing_drops_the_line() {
        let mut style = MLeaderStyle::standard();
        style.content.frame = true;
        style.dogleg = false;
        let pieces = sample(&style, "Note");
        assert_eq!(count(&pieces, |e| matches!(e, Entity::Rectangle { .. })), 1);
        // Arrow and leader line only.
        assert_eq!(count(&pieces, |e| matches!(e, Entity::Polyline { .. })), 2);
    }

    #[test]
    fn a_spline_is_a_curve_through_the_same_ends() {
        let straight = sample(&MLeaderStyle::standard(), "x");
        let style = MLeaderStyle {
            leader_type: MLeaderLineType::Spline,
            ..MLeaderStyle::standard()
        };
        let curved = sample(&style, "x");
        let points = |pieces: &[Entity]| {
            pieces
                .iter()
                .filter_map(|e| match e {
                    Entity::Polyline { points, .. } if points.len() > 2 => Some(points.clone()),
                    _ => None,
                })
                .next()
        };
        assert!(points(&straight).is_none());
        let curve = points(&curved).expect("a smooth line");
        let line = path(&style, 2.5);
        assert_eq!(curve.first(), line.first());
        assert_eq!(curve.last(), line.last());
        assert!(finite(&curved));
    }

    #[test]
    fn held_angles_steer_the_segments_and_a_second_one_needs_more_points() {
        let mut style = MLeaderStyle::standard();
        style.first_angle = Some(0.0);
        style.second_angle = Some(std::f64::consts::FRAC_PI_2);
        assert_eq!(path(&style, 1.0).len(), 2, "two points allow one segment");
        style.max_points = Some(3);
        let points = path(&style, 1.0);
        assert_eq!(points.len(), 3);
        assert!((points[1].y - 0.0).abs() < 1e-9, "the first runs along X");
        assert!(points[2].y > points[1].y, "the second runs up");
    }

    #[test]
    fn a_short_first_segment_has_no_arrow() {
        let mut style = MLeaderStyle::standard();
        style.arrow_size = 1000.0;
        let pieces = sample(&style, "x");
        // Leader line and landing, no arrowhead.
        assert_eq!(count(&pieces, |e| matches!(e, Entity::Polyline { .. })), 2);
    }
}
