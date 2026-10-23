//! Multiline text and text styles as R2000 `MTEXT` and `STYLE` records.

use od_core::mtext::{BackgroundFill, LineSpacingStyle, MTextFlow};
use od_core::{Document, FontDescriptor, MText, TextStyle};

use super::{Handles, Ids};
use crate::dxf::write::{aci_for, dxf_name, finite, Out};
use crate::FileError;

/// Characters per `MTEXT` text group; longer text goes in several.
const CHUNK: usize = 250;

/// The font AutoCAD is asked for when a style uses the bundled sans: the
/// nearest common one.
const BUNDLED_AS: &str = "Arial";

/// Contents as DXF carries them in a Latin-1 file: anything beyond U+00FF
/// becomes `\U+XXXX`, which MTEXT reads back as that character.
pub(super) fn escaped(contents: &str) -> String {
    let mut out = String::with_capacity(contents.len());
    for c in contents.chars() {
        if u32::from(c) < 0x100 {
            out.push(c);
        } else {
            let mut units = [0u16; 2];
            for unit in c.encode_utf16(&mut units) {
                out.push_str(&format!("\\U+{unit:04X}"));
            }
        }
    }
    out
}

/// The name a style goes by in the file.
pub(super) fn style_name(document: &Document, text: &MText) -> String {
    document
        .text_style(text.style)
        .map_or_else(|| "Standard".to_owned(), |s| dxf_name(&s.name))
}

/// The `AcDbMText` part of an entity whose common part is written.
pub(super) fn mtext(out: &mut Out, document: &Document, text: &MText) -> Result<(), FileError> {
    out.pair(100, "AcDbMText");
    out.point3(
        10,
        finite(text.insertion.x)?,
        finite(text.insertion.y)?,
        0.0,
    );
    out.pair(40, finite(text.nominal_height)?);
    out.pair(41, finite(text.reference_width)?);
    out.pair(71, text.attachment.code());
    out.pair(
        72,
        match text.flow {
            MTextFlow::LeftToRight => 1,
            MTextFlow::TopToBottom => 3,
            MTextFlow::ByStyle => 5,
        },
    );
    let contents: Vec<char> = escaped(&text.contents).chars().collect();
    let mut chunks = contents.chunks(CHUNK).map(|c| c.iter().collect::<String>());
    let last = chunks.next_back().unwrap_or_default();
    for chunk in chunks {
        out.pair(3, chunk);
    }
    out.pair(1, last);
    out.pair(7, style_name(document, text));
    out.point3(210, 0.0, 0.0, 1.0);
    out.point3(
        11,
        finite(text.direction.x)?,
        finite(text.direction.y)?,
        0.0,
    );
    out.pair(
        73,
        match text.line_spacing.style {
            LineSpacingStyle::AtLeast => 1,
            LineSpacingStyle::Exact => 2,
        },
    );
    out.pair(44, finite(text.line_spacing.factor)?);
    if let Some(back) = &text.background {
        let extra = text.preserved.background_extra_bits;
        match back.fill {
            BackgroundFill::Window => out.pair(90, 2 | extra),
            BackgroundFill::Color(color) => {
                out.pair(90, 1 | extra);
                out.pair(63, aci_for(color).0);
            }
        }
        out.pair(45, finite(back.scale)?);
    }
    Ok(())
}

fn flags_of(style: &TextStyle) -> (i32, i32) {
    let generation = i32::from(style.backward) * 2 + i32::from(style.upside_down) * 4;
    (i32::from(style.vertical) * 4, generation)
}

/// The STYLE table: every text style of the drawing, Standard first.
pub(super) fn styles(out: &mut Out, h: &mut Handles, ids: &Ids, document: &Document) {
    let all = document.text_styles();
    let owner = super::tables::table(out, h, "STYLE", all.len());
    for (style, handle) in all.iter().zip(&ids.text_styles) {
        super::tables::record(out, "STYLE", owner, "AcDbTextStyleTableRecord", *handle);
        let (flags, generation) = flags_of(style);
        out.pair(2, dxf_name(&style.name));
        out.pair(70, flags);
        out.pair(40, style.fixed_height.unwrap_or(0.0));
        out.pair(41, style.width_factor);
        out.pair(50, style.oblique_radians.to_degrees());
        out.pair(71, generation);
        out.pair(42, 2.5);
        let (file, family) = match &style.font {
            FontDescriptor::BundledSans => {
                ("arial.ttf".to_owned(), Some((BUNDLED_AS, false, false)))
            }
            FontDescriptor::TrueType {
                family,
                bold,
                italic,
            } => (
                format!("{family}.ttf"),
                Some((family.as_str(), *bold, *italic)),
            ),
            FontDescriptor::Shx { file_name } => (file_name.clone(), None),
            // AutoCAD has no OD Roman: name its nearest stroke font.
            FontDescriptor::OdRoman { .. } => ("RomanS.shx".to_owned(), None),
            FontDescriptor::Opaque { raw_primary, .. } => (raw_primary.clone(), None),
        };
        out.pair(3, dxf_name(&file));
        out.pair(
            4,
            style.big_font.as_deref().map_or_else(String::new, dxf_name),
        );
        if let Some((family, bold, italic)) = family {
            out.pair(1001, "ACAD");
            out.pair(1000, dxf_name(family));
            out.pair(
                1071,
                i32::from(bold) * 0x0100_0000 + i32::from(italic) * 0x0200_0000,
            );
        }
    }
    out.pair(0, "ENDTAB");
}

#[cfg(test)]
mod tests {
    use od_core::{DimStyle, Dimension, Entity, Point, TextStyleId};

    use super::*;
    use crate::dxf::DxfFormat;
    use crate::FileFormat;

    fn written(document: &Document) -> (String, Vec<String>) {
        let encoded = DxfFormat.write(document).unwrap();
        (
            String::from_utf8_lossy(&encoded.bytes).into_owned(),
            encoded.notes,
        )
    }

    #[test]
    fn text_is_written_as_mtext_in_a_2000_file_with_its_style() {
        let mut doc = Document::default();
        let notes = doc.allocate_text_style_id();
        let mut style = TextStyle::new(notes, "Notes");
        style.width_factor = 0.8;
        style.oblique_radians = 0.2;
        doc.push_text_style(style);
        doc.add(Entity::MText(Box::new(MText::new(
            Point::new(1.0, 2.0),
            2.5,
            "Hello\\Pworld \u{2312}",
            notes,
        ))));
        let (text, notes) = written(&doc);
        assert!(text.contains("AC1015") && text.contains("\r\nMTEXT\r\n"));
        assert!(text.contains("AcDbMText") && text.contains("Hello\\Pworld \\U+2312"));
        assert!(text.contains("  7\r\nNotes\r\n"));
        assert!(text.contains("  2\r\nNotes\r\n") && text.contains("  2\r\nStandard\r\n"));
        assert!(text.contains(" 41\r\n0.8\r\n"));
        assert!(notes.iter().any(|n| n.contains("multiline text is kept")));
    }

    #[test]
    fn long_text_is_split_into_groups() {
        let mut doc = Document::default();
        let long = "x".repeat(CHUNK * 2 + 5);
        doc.add(Entity::MText(Box::new(MText::new(
            Point::new(0.0, 0.0),
            2.5,
            long,
            TextStyleId::STANDARD,
        ))));
        let (text, _) = written(&doc);
        assert_eq!(text.matches("  3\r\nxxxxxxxx").count(), 2);
        assert!(text.contains("  1\r\nxxxxx\r\n"));
    }

    #[test]
    fn a_dimension_keeps_its_text_as_text() {
        let mut doc = Document::default();
        doc.add(Entity::Dimension(Box::new(Dimension::aligned(
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 5.0),
            &DimStyle::default(),
        ))));
        let (text, notes) = written(&doc);
        assert!(text.contains("MTEXT") && text.contains("10.00"));
        assert!(notes.iter().any(|n| n.contains("Dimensions")));
    }

    #[test]
    fn characters_beyond_latin1_are_escaped() {
        assert_eq!(escaped("Ø10 °"), "Ø10 °");
        assert_eq!(escaped("a\u{2312}b"), "a\\U+2312b");
        assert_eq!(escaped("\u{1F600}"), "\\U+D83D\\U+DE00");
    }
}
