//! Text from a DWG: the style table, and the TEXT and MTEXT shapes as
//! `MText` objects. This build has one text object, so a single line of text
//! becomes an MText without a box: hung from its alignment point, with its
//! own width factor and slant written as format codes.

use std::collections::{HashMap, HashSet};

use od_core::block::{AttributeText, TextHorizontal, TextVertical};
use od_core::mtext::{
    BackgroundFill, EntityLineSpacing, LineSpacingStyle, MTextAttachment, MTextBackground,
    MTextFlow,
};
use od_core::{Document, Entity, FontDescriptor, MText, Point, TextStyle, TextStyleId};
use od_dwg::{DwgHandle, DwgTextStyle, Shape};

use super::convert::color;

/// The styles of the file by handle.
#[derive(Default)]
pub(super) struct Styles {
    by_handle: HashMap<DwgHandle, TextStyleId>,
}

/// The font a style record asks for: a shape file by its file name, a
/// TrueType font by its family (the file's stem).
fn font_of(file: &str) -> FontDescriptor {
    let file = file.trim();
    if file.to_ascii_lowercase().ends_with(".shx") {
        return FontDescriptor::from_name(file, false, false);
    }
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    match stem {
        "" => FontDescriptor::BundledSans,
        name => FontDescriptor::from_name(name, false, false),
    }
}

/// Puts the file's text styles in the document. `Standard` takes the place of
/// the document's own; a style that makes no valid style, or repeats a name,
/// is left out and its text uses `Standard`.
pub(super) fn import_styles(document: &mut Document, styles: &[DwgTextStyle]) -> Styles {
    let mut out = Styles::default();
    let mut taken: HashSet<String> = HashSet::new();
    for s in styles {
        let name = s.name.trim();
        let key = name.to_lowercase();
        if name.is_empty() || !taken.insert(key.clone()) {
            continue;
        }
        let standard = key == "standard";
        let id = if standard {
            TextStyleId::STANDARD
        } else {
            document.allocate_text_style_id()
        };
        let mut style = TextStyle::new(id, name);
        style.font = font_of(&s.font);
        style.big_font = Some(s.big_font.trim().to_owned()).filter(|f| !f.is_empty());
        style.fixed_height = Some(s.fixed_height).filter(|h| *h > 0.0);
        style.width_factor = s.width_factor;
        style.oblique_radians = s.oblique;
        style.vertical = s.vertical;
        style.backward = s.generation & 2 != 0;
        style.upside_down = s.generation & 4 != 0;
        if !style.is_valid() {
            if standard {
                out.by_handle.insert(s.handle, id);
            }
            continue;
        }
        if standard {
            document.replace_text_style(style);
        } else {
            document.push_text_style(style);
        }
        out.by_handle.insert(s.handle, id);
    }
    out
}

impl Styles {
    fn id(&self, handle: Option<DwgHandle>) -> TextStyleId {
        handle
            .and_then(|h| self.by_handle.get(&h))
            .copied()
            .unwrap_or(TextStyleId::STANDARD)
    }
}

/// `%%c`, `%%d`, `%%p` and `%%%`, the character codes AutoCAD's text takes,
/// as the characters; `%%u` and `%%o` (and, in single-line text only, which
/// has no other markup) as underline and overline switches.
fn percent_codes(text: &str, markup: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let (mut under, mut over) = (false, false);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' || chars.peek() != Some(&'%') {
            out.push(c);
            continue;
        }
        chars.next();
        match chars.peek().map(char::to_ascii_lowercase) {
            Some('c') => out.push('\u{D8}'),
            Some('d') => out.push('\u{B0}'),
            Some('p') => out.push('\u{B1}'),
            Some('%') => out.push('%'),
            Some('u') => {
                under = !under;
                out.push_str(if under { "\\L" } else { "\\l" });
            }
            Some('o') => {
                over = !over;
                out.push_str(if over { "\\O" } else { "\\o" });
            }
            _ => {
                out.push_str("%%");
                continue;
            }
        }
        chars.next();
    }
    if markup {
        // A switch left on at the end of single-line text ends with the text.
        if under {
            out.push_str("\\l");
        }
        if over {
            out.push_str("\\o");
        }
    }
    out
}

/// A line of text as MText contents: its braces and backslashes kept as
/// letters (except the `\U+` forms, which are Unicode escapes), then the
/// width factor and slant it asks for.
fn line_contents(value: &str, width_factor: f64, oblique: f64) -> String {
    let mut text = String::new();
    let mut rest = value;
    while let Some(c) = rest.chars().next() {
        let len = c.len_utf8();
        match c {
            '{' | '}' => {
                text.push('\\');
                text.push(c);
            }
            '\\' if rest.starts_with("\\U+") || rest.starts_with("\\M+") => text.push(c),
            '\\' => text.push_str("\\\\"),
            _ => text.push(c),
        }
        rest = &rest[len..];
    }
    let mut out = String::new();
    if (width_factor - 1.0).abs() > 1e-9 {
        out.push_str(&format!("\\W{width_factor};"));
    }
    if oblique.abs() > 1e-9 {
        out.push_str(&format!("\\Q{};", oblique.to_degrees()));
    }
    out.push_str(&percent_codes(&text, true));
    out
}

fn attachment(horizontal: u8, vertical: u8) -> MTextAttachment {
    use MTextAttachment::*;
    let row = |top, middle, bottom| match vertical {
        0 | 3 => top,
        2 => middle,
        _ => bottom,
    };
    match horizontal {
        1 => row(TopCenter, MiddleCenter, BottomCenter),
        2 => row(TopRight, MiddleRight, BottomRight),
        _ => row(TopLeft, MiddleLeft, BottomLeft),
    }
}

/// The settings of an attribute's text (a [`Shape::Text`] placed in the
/// drawing) and the value it shows, or `None` when it is not valid text.
pub(super) fn attribute_text(shape: &Shape, styles: &Styles) -> Option<(AttributeText, String)> {
    let Shape::Text {
        at,
        rotation,
        height,
        width_factor,
        oblique,
        value,
        horizontal,
        vertical,
        backward,
        upside_down,
        style,
    } = shape
    else {
        return None;
    };
    let text = AttributeText {
        position: Point::new(at[0], at[1]),
        height: *height,
        rotation: *rotation,
        width_factor: *width_factor,
        oblique: *oblique,
        style: styles.id(*style),
        horizontal: match horizontal {
            1 => TextHorizontal::Center,
            2 => TextHorizontal::Right,
            _ => TextHorizontal::Left,
        },
        vertical: match vertical {
            1 => TextVertical::Bottom,
            2 => TextVertical::Middle,
            3 => TextVertical::Top,
            _ => TextVertical::Baseline,
        },
        generation_flags: if *backward { 2 } else { 0 } | if *upside_down { 4 } else { 0 },
    };
    text.validate().ok()?;
    Some((text, percent_codes(value, false)))
}

/// The object for a text shape, or `None` when its values make no text.
pub(super) fn entity(shape: &Shape, styles: &Styles) -> Option<Entity> {
    let text = match shape {
        Shape::Text {
            at,
            rotation,
            height,
            width_factor,
            oblique,
            value,
            horizontal,
            vertical,
            style,
            ..
        } => {
            let (sin, cos) = rotation.sin_cos();
            // A line is as tall as its text, so text hung by its baseline is
            // the same as text hung by its top, one height higher.
            let lift = if *vertical == 0 { *height } else { 0.0 };
            let mut text = MText::new(
                Point::new(at[0] - lift * sin, at[1] + lift * cos),
                *height,
                line_contents(value, *width_factor, *oblique),
                styles.id(*style),
            );
            text.attachment = attachment(*horizontal, *vertical);
            text.direction = Point::new(cos, sin);
            text
        }
        Shape::MText {
            at,
            direction,
            height,
            width,
            attachment,
            flow,
            contents,
            spacing_style,
            spacing,
            background,
            style,
        } => {
            let mut text = MText::new(
                Point::new(at[0], at[1]),
                *height,
                percent_codes(contents, false),
                styles.id(*style),
            );
            text.reference_width = *width;
            text.attachment = MTextAttachment::from_code(i64::from(*attachment))
                .unwrap_or(MTextAttachment::TopLeft);
            text.flow = MTextFlow::from_code(i64::from(*flow)).unwrap_or(MTextFlow::LeftToRight);
            text.direction = Point::new(direction[0], direction[1]);
            if (0.25..=4.0).contains(spacing) {
                text.line_spacing = EntityLineSpacing {
                    style: LineSpacingStyle::from_code(i64::from(*spacing_style))
                        .unwrap_or(LineSpacingStyle::AtLeast),
                    factor: *spacing,
                };
            }
            text.background = background.map(|(scale, own)| MTextBackground {
                fill: own.map_or(BackgroundFill::Window, |c| BackgroundFill::Color(color(c))),
                scale: if scale >= 1.0 { scale } else { 1.5 },
            });
            text
        }
        _ => return None,
    };
    text.validate().ok()?;
    Some(Entity::MText(Box::new(text)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(horizontal: u8, vertical: u8, rotation: f64) -> Shape {
        Shape::Text {
            at: [10.0, 20.0],
            rotation,
            height: 2.0,
            width_factor: 1.0,
            oblique: 0.0,
            value: "Hi".into(),
            horizontal,
            vertical,
            backward: false,
            upside_down: false,
            style: None,
        }
    }

    fn made(shape: &Shape) -> MText {
        match entity(shape, &Styles::default()) {
            Some(Entity::MText(m)) => *m,
            _ => panic!("an mtext"),
        }
    }

    #[test]
    fn percent_codes_become_characters_and_switches() {
        assert_eq!(
            percent_codes("45%%d %%c10 %%p1 100%%%", false),
            "45° Ø10 ±1 100%"
        );
        assert_eq!(percent_codes("%%uab%%u c", true), "\\Lab\\l c");
        assert_eq!(percent_codes("%%oab", true), "\\Oab\\o");
        assert_eq!(percent_codes("%%x", true), "%%x");
    }

    #[test]
    fn markup_in_a_line_is_text_not_codes() {
        assert_eq!(line_contents("a{b}\\c", 1.0, 0.0), "a\\{b\\}\\\\c");
        assert_eq!(line_contents("\\U+00B0", 1.0, 0.0), "\\U+00B0");
        assert_eq!(line_contents("x", 0.5, 0.0), "\\W0.5;x");
        assert!(line_contents("x", 1.0, 0.2).starts_with("\\Q11.4"));
    }

    #[test]
    fn baseline_text_hangs_from_its_top_one_height_up() {
        let m = made(&text(0, 0, 0.0));
        assert_eq!(m.attachment, MTextAttachment::TopLeft);
        assert_eq!((m.insertion.x, m.insertion.y), (10.0, 22.0));
        let turned = made(&text(0, 0, std::f64::consts::FRAC_PI_2));
        assert!(
            (turned.insertion.x - 8.0).abs() < 1e-9 && (turned.insertion.y - 20.0).abs() < 1e-9
        );
        assert!(turned.direction.x.abs() < 1e-9 && (turned.direction.y - 1.0).abs() < 1e-9);
    }

    #[test]
    fn the_other_alignments_pick_their_attachment() {
        assert_eq!(
            made(&text(1, 2, 0.0)).attachment,
            MTextAttachment::MiddleCenter
        );
        assert_eq!(made(&text(2, 3, 0.0)).attachment, MTextAttachment::TopRight);
        assert_eq!(
            made(&text(1, 1, 0.0)).attachment,
            MTextAttachment::BottomCenter
        );
        assert_eq!(made(&text(1, 2, 0.0)).insertion.y, 20.0);
    }

    #[test]
    fn an_mtext_keeps_its_box_attachment_and_mask() {
        let shape = Shape::MText {
            at: [1.0, 2.0],
            direction: [0.0, 1.0],
            height: 2.5,
            width: 30.0,
            attachment: 5,
            flow: 5,
            contents: "A\\PB%%d".into(),
            spacing_style: 2,
            spacing: 1.5,
            background: Some((1.2, Some(od_dwg::DwgColor::Aci(1)))),
            style: None,
        };
        let m = made(&shape);
        assert_eq!(m.contents, "A\\PB°");
        assert_eq!(
            (m.reference_width, m.attachment),
            (30.0, MTextAttachment::MiddleCenter)
        );
        assert_eq!(m.line_spacing.factor, 1.5);
        assert_eq!(m.background.map(|b| b.scale), Some(1.2));
    }

    #[test]
    fn styles_come_in_with_standard_in_its_place() {
        let mut doc = Document::default();
        let style = |handle, name: &str, font: &str| DwgTextStyle {
            handle: DwgHandle(handle),
            name: name.into(),
            font: font.into(),
            big_font: String::new(),
            vertical: false,
            shape_file: false,
            fixed_height: 0.0,
            width_factor: 0.8,
            oblique: 0.0,
            generation: 0,
        };
        let styles = import_styles(
            &mut doc,
            &[
                style(1, "Standard", "arial.ttf"),
                style(2, "Notes", "romans.shx"),
                style(3, "NOTES", "arial.ttf"),
            ],
        );
        assert_eq!(styles.id(Some(DwgHandle(1))), TextStyleId::STANDARD);
        let notes = styles.id(Some(DwgHandle(2)));
        assert_ne!(notes, TextStyleId::STANDARD);
        assert_eq!(doc.text_style(notes).map(|s| s.width_factor), Some(0.8));
        // The repeated name is dropped; its text falls back to Standard.
        assert_eq!(styles.id(Some(DwgHandle(3))), TextStyleId::STANDARD);
        assert!(doc.validate().is_ok());
    }
}
