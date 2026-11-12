//! Dimension text as multiline text. A dimension's words are an `MText` in
//! the picture, so they take the same bundled font, shaping and outlines as
//! any other text and plot as filled glyphs. This module measures a label
//! and places one with the middle of its capitals on a point.

use crate::mtext::MText;
use crate::text::escape_for_contents;
use crate::text_style::{TextStyle, TextStyleId};
use crate::Point;

fn label_at_origin(contents: &str, height: f64) -> MText {
    MText::new(
        Point::new(0.0, 0.0),
        height,
        contents.to_owned(),
        TextStyleId::STANDARD,
    )
}

/// Width of `text` at cap height `height`.
pub fn text_width(text: &str, height: f64) -> f64 {
    contents_width(&escape_for_contents(text), height)
}

/// Width of text written as multiline text `contents` (stacks, sizes).
pub fn contents_width(contents: &str, height: f64) -> f64 {
    label_at_origin(contents, height)
        .layout(&TextStyle::standard())
        .frame_width
}

/// `text` as a text object whose capitals are centred on `centre`, running
/// along `angle`. `None` when there is nothing to show.
pub fn place(
    text: &str,
    height: f64,
    centre: Point,
    angle: f64,
    style: TextStyleId,
) -> Option<MText> {
    place_contents(&escape_for_contents(text), height, centre, angle, style)
}

/// As [`place`] for text written as multiline text `contents`.
pub fn place_contents(
    contents: &str,
    height: f64,
    centre: Point,
    angle: f64,
    style: TextStyleId,
) -> Option<MText> {
    if contents.is_empty() {
        return None;
    }
    let mut label = label_at_origin(contents, height);
    label.style = style;
    let layout = label.layout(&TextStyle::standard());
    let baseline = layout.lines.first()?.baseline;
    // Layout `x` runs along the text and `y` up from the top of the block;
    // the insertion point is the block's top left.
    let (across, up) = (layout.frame_width / 2.0, baseline + height / 2.0);
    let (c, s) = (angle.cos(), angle.sin());
    label.direction = Point::new(c, s);
    label.insertion = Point::new(
        centre.x - across * c + up * s,
        centre.y - across * s - up * c,
    );
    Some(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_grows_with_each_character() {
        let one = text_width("1", 6.0);
        let two = text_width("11", 6.0);
        assert!(one > 0.0 && two > one);
    }

    #[test]
    fn width_scales_with_height() {
        let a = text_width("12.5", 2.5);
        let b = text_width("12.5", 5.0);
        assert!((b - 2.0 * a).abs() < 1e-6);
    }

    #[test]
    fn nothing_to_say_places_nothing() {
        assert!(place("", 3.0, Point::new(0.0, 0.0), 0.0, TextStyleId::STANDARD).is_none());
    }

    #[test]
    fn placed_text_has_its_capitals_centred() {
        let centre = Point::new(10.0, 10.0);
        let label = place("H", 4.0, centre, 0.0, TextStyleId::STANDARD).unwrap();
        let layout = label.layout(&TextStyle::standard());
        let mid_y = label
            .world_point(&layout, 0.0, layout.lines[0].baseline + 2.0)
            .y;
        let mid_x = label.world_point(&layout, layout.frame_width / 2.0, 0.0).x;
        assert!((mid_y - 10.0).abs() < 1e-9 && (mid_x - 10.0).abs() < 1e-9);
    }

    #[test]
    fn turned_text_stays_centred() {
        let centre = Point::new(-3.0, 7.0);
        let angle = 0.7;
        let label = place("12.5", 2.5, centre, angle, TextStyleId(3)).unwrap();
        let layout = label.layout(&TextStyle::standard());
        let at = label.world_point(
            &layout,
            layout.frame_width / 2.0,
            layout.lines[0].baseline + 1.25,
        );
        assert!((at.x - centre.x).abs() < 1e-9 && (at.y - centre.y).abs() < 1e-9);
    }

    #[test]
    fn the_symbols_dimensions_use_are_in_the_font() {
        for symbol in ["Ø", "°", "±"] {
            let label = label_at_origin(&escape_for_contents(symbol), 2.5);
            let layout = label.layout(&TextStyle::standard());
            assert!(
                layout.diagnostics.is_empty() && !layout.glyphs.is_empty(),
                "{symbol}"
            );
        }
    }
}
