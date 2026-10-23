//! Multiline text as filled outlines for plotting.

use crate::aci::aci_rgb;
use crate::text::{glyph_outline, TextColor};
use crate::{Bounds, Document, MText, Point, TextStyle};

/// Outlines filled together with the non-zero rule, in one colour.
pub(crate) struct Fill {
    pub contours: Vec<Vec<Point>>,
    pub rgb: [u8; 3],
}

/// Paper is white, so white plots black.
fn on_paper(rgb: [u8; 3]) -> [u8; 3] {
    if rgb.iter().all(|c| *c >= 240) {
        [0, 0, 0]
    } else {
        rgb
    }
}

fn rgb_of(color: TextColor, object: [u8; 3]) -> [u8; 3] {
    match color {
        TextColor::Entity | TextColor::Aci(7) => object,
        TextColor::Aci(n) => {
            let (r, g, b) = aci_rgb(u8::try_from(n).unwrap_or(7));
            on_paper([r, g, b])
        }
        TextColor::Rgb(v) => on_paper([(v >> 16) as u8, (v >> 8) as u8, v as u8]),
    }
}

/// The text's glyphs and decoration bars as drawing-space contours, curves
/// within `tolerance` drawing units, consecutive same-coloured pieces joined.
/// `object_rgb` is the colour of text that takes the object's own.
pub(crate) fn fills(
    document: &Document,
    text: &MText,
    tolerance: f64,
    object_rgb: [u8; 3],
) -> Vec<Fill> {
    let stock;
    let style = match document.text_style(text.style) {
        Some(s) => s,
        None => {
            stock = TextStyle::standard();
            &stock
        }
    };
    let layout = text.layout(style);
    let mut out: Vec<Fill> = Vec::new();
    let mut add = |contours: Vec<Vec<Point>>, rgb: [u8; 3]| {
        if contours.is_empty() {
            return;
        }
        match out.last_mut() {
            Some(last) if last.rgb == rgb => last.contours.extend(contours),
            _ => out.push(Fill { contours, rgb }),
        }
    };
    for glyph in &layout.glyphs {
        let font_tolerance = tolerance / glyph.scale_y.max(1e-12);
        let contours = glyph_outline(glyph.face, glyph.id, glyph.weight, font_tolerance)
            .iter()
            .map(|contour| {
                contour
                    .iter()
                    .map(|&(fx, fy)| {
                        let (x, y) = glyph.place(fx, fy);
                        text.world_point(&layout, x, y)
                    })
                    .collect()
            })
            .collect();
        add(contours, rgb_of(glyph.color, object_rgb));
    }
    for bar in &layout.decorations {
        let half = bar.thickness / 2.0;
        let corners = [
            (bar.x0, bar.y - half),
            (bar.x1, bar.y - half),
            (bar.x1, bar.y + half),
            (bar.x0, bar.y + half),
        ];
        let contour = corners
            .iter()
            .map(|&(x, y)| text.world_point(&layout, x, y))
            .collect();
        add(vec![contour], rgb_of(bar.color, object_rgb));
    }
    out
}

/// The filled text an object draws: its own if it is text, or the words of
/// a dimension or multileader.
pub(crate) fn object_fills(
    document: &Document,
    object: &crate::Object,
    tolerance: f64,
    object_rgb: [u8; 3],
) -> Vec<Fill> {
    if let crate::Entity::MText(text) = &object.entity {
        return fills(document, text, tolerance, object_rgb);
    }
    let picture = match &object.entity {
        crate::Entity::MLeader(m) => m.picture().into(),
        _ => match object.dimension_picture() {
            Some(picture) => picture,
            None => return Vec::new(),
        },
    };
    picture
        .iter()
        .filter_map(|piece| match piece {
            crate::Entity::MText(text) => Some(fills(document, text, tolerance, object_rgb)),
            _ => None,
        })
        .flatten()
        .collect()
}

/// The part of a polygon inside `frame` (Sutherland-Hodgman). Concave
/// polygons can come back with zero-width bridges along the frame, which fill
/// no area.
pub(crate) fn clip_polygon(points: &[Point], frame: &Bounds) -> Vec<Point> {
    let mut ring = points.to_vec();
    // The frame's four sides: left, right, bottom, top.
    for side in 0..4 {
        if ring.is_empty() {
            break;
        }
        let inside = |p: Point| match side {
            0 => p.x >= frame.min.x,
            1 => p.x <= frame.max.x,
            2 => p.y >= frame.min.y,
            _ => p.y <= frame.max.y,
        };
        let cut = |a: Point, b: Point| match side {
            0 => cut_x(a, b, frame.min.x),
            1 => cut_x(a, b, frame.max.x),
            2 => cut_y(a, b, frame.min.y),
            _ => cut_y(a, b, frame.max.y),
        };
        let mut next = Vec::with_capacity(ring.len() + 4);
        for (i, &current) in ring.iter().enumerate() {
            let previous = ring[(i + ring.len() - 1) % ring.len()];
            match (inside(previous), inside(current)) {
                (true, true) => next.push(current),
                (true, false) => next.push(cut(previous, current)),
                (false, true) => {
                    next.push(cut(previous, current));
                    next.push(current);
                }
                (false, false) => {}
            }
        }
        ring = next;
    }
    if ring.len() < 3 {
        Vec::new()
    } else {
        ring
    }
}

fn cut_x(a: Point, b: Point, x: f64) -> Point {
    let t = (x - a.x) / (b.x - a.x);
    Point::new(x, a.y + t * (b.y - a.y))
}

fn cut_y(a: Point, b: Point, y: f64) -> Point {
    let t = (y - a.y) / (b.y - a.y);
    Point::new(a.x + t * (b.x - a.x), y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextStyleId;

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ]
    }

    fn area(points: &[Point]) -> f64 {
        let n = points.len();
        (0..n)
            .map(|i| points[i].x * points[(i + 1) % n].y - points[(i + 1) % n].x * points[i].y)
            .sum::<f64>()
            / 2.0
    }

    #[test]
    fn clipping_keeps_what_is_inside_and_cuts_the_rest() {
        let frame = Bounds::from_points(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        let inside = square(1.0, 1.0, 2.0, 2.0);
        assert_eq!(clip_polygon(&inside, &frame), inside);
        assert!(clip_polygon(&square(20.0, 20.0, 30.0, 30.0), &frame).is_empty());
        let across = clip_polygon(&square(5.0, 5.0, 15.0, 15.0), &frame);
        assert!((area(&across).abs() - 25.0).abs() < 1e-9);
        let covers = clip_polygon(&square(-5.0, -5.0, 15.0, 15.0), &frame);
        assert!((area(&covers).abs() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn text_becomes_filled_contours_in_the_right_colours() {
        let document = Document::default();
        let text = MText::new(
            Point::new(10.0, 20.0),
            3.0,
            "H\\C1;H \\Lu",
            TextStyleId::STANDARD,
        );
        let fills = fills(&document, &text, 0.01, [0, 0, 0]);
        assert_eq!(
            fills.len(),
            2,
            "a black H, then everything after the colour change"
        );
        assert_eq!(fills[0].rgb, [0, 0, 0]);
        assert_eq!(fills[1].rgb, [255, 0, 0]);
        // An H is one contour of twelve points, and every point is near the text.
        assert_eq!(fills[0].contours.len(), 1);
        assert_eq!(fills[0].contours[0].len(), 12);
        for p in fills.iter().flat_map(|f| f.contours.iter().flatten()) {
            assert!(
                p.x >= 10.0 - 1.0 && p.y <= 20.0 + 1.0 && p.y >= 20.0 - 6.0,
                "{p:?}"
            );
        }
        // The second H, the u and the underline bar are one fill.
        assert!(fills[1].contours.len() >= 3);
    }

    #[test]
    fn od_roman_plots_as_filled_pens_that_widen_with_the_weight() {
        let mut document = Document::default();
        let mut stem_width = |weight: f64| {
            let id = document.allocate_text_style_id();
            document.push_text_style(crate::TextStyle {
                font: crate::FontDescriptor::OdRoman {
                    bold: false,
                    italic: false,
                },
                stroke_weight: weight,
                ..crate::TextStyle::new(id, "Roman")
            });
            // A capital I is one upright stroke: as wide as the pen.
            let text = MText::new(Point::new(0.0, 0.0), 3.0, "I", id);
            let fills = fills(&document, &text, 0.005, [0, 0, 0]);
            assert_eq!(fills.len(), 1);
            let xs = fills[0].contours.iter().flatten().map(|p| p.x);
            xs.clone().fold(f64::MIN, f64::max) - xs.fold(f64::MAX, f64::min)
        };
        let (fine, heavy) = (stem_width(0.05), stem_width(0.30));
        assert!((fine - 0.15).abs() < 0.02, "{fine}");
        assert!((heavy - 0.90).abs() < 0.02, "{heavy}");
    }
}
