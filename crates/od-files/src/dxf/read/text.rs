//! STYLE table records and MTEXT entities.
//!
//! A text style is read for its name, font, width factor, oblique angle,
//! fixed height and the backward, upside-down and vertical flags. The font
//! family is the one AutoCAD records in the style's extended data when there
//! is one, else the font file's name. MTEXT keeps its contents as written
//! (format codes included), its box width, direction, attachment, flow, line
//! spacing and mask; text in columns, or not drawn in the XY plane, is
//! skipped by reason.

use std::collections::HashMap;
use std::f64::consts::PI;

use od_core::mtext::{
    BackgroundFill, EntityLineSpacing, LineSpacingStyle, MTextAttachment, MTextBackground,
    MTextFlow,
};
use od_core::{Entity, FontDescriptor, MText, Point, TextStyle, TextStyleId};

use super::{aci_color, decode, Importer, Record};

/// The text styles the file named, by lower-cased name and by handle.
#[derive(Default)]
pub(super) struct Styles {
    pub(super) by_name: HashMap<String, TextStyleId>,
    pub(super) by_handle: HashMap<String, TextStyleId>,
}

/// The font a STYLE record asks for.
fn font_of(record: &Record) -> FontDescriptor {
    let file = decode(record.text(3).unwrap_or(""));
    if file.to_ascii_lowercase().ends_with(".shx") {
        return FontDescriptor::from_name(&file, false, false);
    }
    // `1001 ACAD` then `1000 family`: the family name AutoCAD recorded.
    let family = record
        .groups
        .iter()
        .skip_while(|g| !(g.code == 1001 && g.value == "ACAD"))
        .find(|g| g.code == 1000)
        .map(|g| decode(&g.value));
    let stem = file
        .rsplit_once('.')
        .map_or(file.as_str(), |(stem, _)| stem)
        .to_owned();
    match family.filter(|f| !f.is_empty()).unwrap_or(stem) {
        name if name.is_empty() => FontDescriptor::BundledSans,
        name => FontDescriptor::from_name(&name, false, false),
    }
}

/// The value of a group as stored: trimmed, except in the words of text,
/// where a space at either end is part of them (a long text is cut into
/// chunks, and the cut can fall after a space).
pub(super) fn value_of(record: &str, code: i32, raw: &str) -> String {
    match (record, code) {
        // A dimension's one space means "no text".
        ("MTEXT", 1 | 3) | ("MULTILEADER", 304) | ("DIMENSION", 1) => {
            raw.trim_end_matches(['\r', '\n']).to_owned()
        }
        _ => raw.trim().to_owned(),
    }
}

impl Importer {
    /// Read the STYLE table before any text refers to a style.
    pub(super) fn scan_text_styles(&mut self, records: &[Record]) {
        for record in records.iter().filter(|r| r.name == "STYLE") {
            let name = decode(record.text(2).unwrap_or(""));
            if name.trim().is_empty() {
                continue;
            }
            let key = name.to_lowercase();
            let standard = key == "standard";
            if self.text_styles.by_name.contains_key(&key) {
                continue;
            }
            let id = if standard {
                TextStyleId::STANDARD
            } else {
                self.document.allocate_text_style_id()
            };
            let flags = record.int(70).unwrap_or(0);
            let generation = record.int(71).unwrap_or(0);
            let mut style = TextStyle::new(id, name);
            style.font = font_of(record);
            style.big_font = record.text(4).map(decode).filter(|f| !f.trim().is_empty());
            style.fixed_height = record.number(40).filter(|h| *h > 0.0);
            style.width_factor = record.number(41).unwrap_or(1.0);
            style.oblique_radians = record.number(50).unwrap_or(0.0).to_radians();
            style.vertical = flags & 4 != 0;
            style.backward = generation & 2 != 0;
            style.upside_down = generation & 4 != 0;
            if !style.is_valid() {
                if standard {
                    self.text_styles.by_name.insert(key, id);
                }
                continue;
            }
            if standard {
                self.document.replace_text_style(style);
            } else {
                self.document.push_text_style(style);
            }
            if let Some(handle) = record.text(5) {
                self.text_styles.by_handle.insert(handle.to_owned(), id);
            }
            self.text_styles.by_name.insert(key, id);
        }
    }

    /// An MTEXT entity: into the drawing, or counted by why not.
    pub(super) fn mtext(&mut self, record: &Record) {
        match self.read_mtext(record) {
            Ok(text) if text.validate().is_ok() => self.add(record, Entity::MText(Box::new(text))),
            Ok(_) => self.damaged += 1,
            Err(why) => self.skip(&format!("MTEXT ({why})")),
        }
    }

    fn read_mtext(&self, record: &Record) -> Result<MText, &'static str> {
        let at = Self::point(record, 10, 20).ok_or("damaged")?;
        let height = record.number(40).filter(|h| *h > 0.0).ok_or("damaged")?;
        let normal = (
            record.number(210).unwrap_or(0.0),
            record.number(220).unwrap_or(0.0),
            record.number(230).unwrap_or(1.0),
        );
        if normal != (0.0, 0.0, 1.0) {
            return Err("not drawn in the XY plane");
        }
        if record.int(75).is_some_and(|columns| columns != 0) {
            return Err("columns");
        }
        // The chunks (3) come first and the last (1) ends the contents.
        let mut contents = String::new();
        for g in record.groups.iter().filter(|g| g.code == 3 || g.code == 1) {
            contents.push_str(&g.value);
        }
        let style = record
            .text(7)
            .and_then(|n| self.text_styles.by_name.get(&decode(n).to_lowercase()))
            .copied()
            .unwrap_or(TextStyleId::STANDARD);
        let mut text = MText::new(at, height, decode(&contents), style);
        text.reference_width = record.number(41).filter(|w| *w > 0.0).unwrap_or(0.0);
        text.attachment = record
            .int(71)
            .and_then(MTextAttachment::from_code)
            .unwrap_or(MTextAttachment::TopLeft);
        text.flow = record
            .int(72)
            .and_then(MTextFlow::from_code)
            .unwrap_or(MTextFlow::LeftToRight);
        text.direction = match Self::point(record, 11, 21) {
            Some(d) if d.x.hypot(d.y) > 1e-12 => d,
            _ => {
                let angle = record.number(50).unwrap_or(0.0) * PI / 180.0;
                Point::new(angle.cos(), angle.sin())
            }
        };
        if let Some(factor) = record.number(44).filter(|f| (0.25..=4.0).contains(f)) {
            text.line_spacing = EntityLineSpacing {
                style: record
                    .int(73)
                    .and_then(LineSpacingStyle::from_code)
                    .unwrap_or(LineSpacingStyle::AtLeast),
                factor,
            };
        }
        let fill = record.int(90).unwrap_or(0);
        if fill & 3 != 0 {
            let scale = record.number(45).filter(|s| *s >= 1.0).unwrap_or(1.5);
            let fill = match (fill & 1 != 0, record.int(63)) {
                (true, Some(index)) => BackgroundFill::Color(aci_color(index.abs())),
                _ => BackgroundFill::Window,
            };
            text.background = Some(MTextBackground { fill, scale });
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dxf::read::read as read_dxf;

    fn texts(bytes: &[u8]) -> Vec<MText> {
        read_dxf(bytes)
            .unwrap()
            .document
            .objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::MText(m) => Some((**m).clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn text_written_by_the_dxf_writer_reads_back_with_its_style() {
        let mut doc = od_core::Document::default();
        let notes = doc.allocate_text_style_id();
        let mut style = TextStyle::new(notes, "Notes");
        style.width_factor = 0.8;
        style.oblique_radians = 0.2;
        doc.push_text_style(style);
        let mut text = MText::new(Point::new(3.0, 4.0), 2.5, r"Hello \Pthere ", notes);
        text.attachment = MTextAttachment::BottomRight;
        text.reference_width = 12.0;
        text.direction = Point::new(0.0, 1.0);
        doc.add(Entity::MText(Box::new(text.clone())));
        let bytes = crate::dxf::write2000::write(&doc).unwrap().bytes;
        let back = read_dxf(&bytes).unwrap().document;
        let found = back.objects().iter().find_map(|o| match &o.entity {
            Entity::MText(m) => Some((**m).clone()),
            _ => None,
        });
        let found = found.unwrap();
        let style = back.text_style(found.style).unwrap();
        assert_eq!(style.name, "Notes");
        assert!((style.width_factor - 0.8).abs() < 1e-9);
        assert!((style.oblique_radians - 0.2).abs() < 1e-9);
        assert_eq!(found.contents, text.contents);
        assert_eq!(found.attachment, text.attachment);
        assert_eq!(found.reference_width, 12.0);
        assert_eq!(found.direction, Point::new(0.0, 1.0));
        assert!(back.validate().is_ok());
    }

    fn file(entity: &str) -> Vec<u8> {
        format!("0\nSECTION\n2\nENTITIES\n0\nMTEXT\n{entity}0\nENDSEC\n0\nEOF\n").into_bytes()
    }

    #[test]
    fn chunks_join_and_the_last_group_ends_the_text() {
        let bytes = file("10\n1\n20\n2\n40\n2.5\n3\nHello \n3\nthere \n1\nworld\n");
        let found = texts(&bytes);
        assert_eq!(found[0].contents, "Hello there world");
    }

    #[test]
    fn rotation_or_a_direction_turns_the_text_and_a_mask_and_spacing_come_in() {
        let turned = texts(&file("10\n0\n20\n0\n40\n2\n1\nx\n50\n90\n"));
        assert!(turned[0].direction.x.abs() < 1e-9 && (turned[0].direction.y - 1.0).abs() < 1e-9);
        let along = texts(&file("10\n0\n20\n0\n40\n2\n1\nx\n11\n0\n21\n-1\n"));
        assert_eq!(along[0].direction, Point::new(0.0, -1.0));
        let masked = texts(&file(
            "10\n0\n20\n0\n40\n2\n1\nx\n90\n1\n63\n1\n45\n1.5\n73\n2\n44\n2\n",
        ));
        assert!(masked[0].background.is_some());
        assert_eq!(masked[0].line_spacing.factor, 2.0);
    }

    #[test]
    fn what_cannot_come_in_is_said_by_reason_and_bad_records_are_damaged() {
        for (entity, reason) in [
            ("10\n0\n20\n0\n40\n2\n1\nx\n75\n1\n", "columns"),
            (
                "10\n0\n20\n0\n40\n2\n1\nx\n230\n-1\n",
                "not drawn in the XY plane",
            ),
        ] {
            let loaded = read_dxf(&file(entity)).unwrap();
            assert!(loaded.document.objects().is_empty());
            assert!(loaded.warnings.join(" ").contains(reason), "{reason}");
        }
        let loaded = read_dxf(&file("10\n0\n20\n0\n40\n0\n1\nx\n")).unwrap();
        assert!(loaded.document.objects().is_empty());
        assert!(loaded.warnings.join(" ").contains("damaged"));
    }
}
