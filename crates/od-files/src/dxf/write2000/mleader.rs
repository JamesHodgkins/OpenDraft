//! MULTILEADER entities and the MLEADERSTYLE objects they name, in the layout
//! AutoCAD 2018 writes (`private_material/LEADERS1.dxf`): the context data with
//! its nested `LEADER{` and `LEADER_LINE{`, then the values the entity itself
//! holds. Fixed values are AutoCAD's own for the defaults. Not yet reopened
//! in AutoCAD itself.
//!
//! The entity's override flags (group 90) are written as 0: the effective
//! values are all written, and which bits mean what is not documented.
//! Proxy graphics (groups 160 and 310) are left out; AutoCAD makes its own.

use od_core::mleader::{
    LeaderAttachment, MLeader, MLeaderContentType, MLeaderLineType, MLeaderStyle, TextAngle,
};
use od_core::{Attr, Color, Document, Lineweight};

use super::text::escaped;
use super::{hex, Ids};
use crate::dxf::write::{dxf_name, finite, Out};
use crate::FileError;

/// Colour code 0xC0000000: by layer.
const BY_LAYER: i64 = -1_073_741_824;
/// Colour code 0xC1000000: by block (the block colour AutoCAD writes).
const BY_BLOCK: i64 = -1_056_964_608;
/// The default direction group values for the attachment direction.
const ATTACH_DIRECTION: i32 = 9;

fn colour_code(colour: &Attr<Color>) -> i64 {
    match colour {
        Attr::Set(Color::Rgb(r, g, b)) => {
            0xC200_0000_i64 | (i64::from(*r) << 16) | (i64::from(*g) << 8) | i64::from(*b)
        }
        _ => BY_LAYER,
    }
}

fn weight_code(weight: &Attr<Lineweight>) -> i32 {
    match weight {
        Attr::Set(w) => i32::from(w.0),
        Attr::ByLayer => -1,
    }
}

fn attachment_code(a: LeaderAttachment) -> usize {
    LeaderAttachment::ALL
        .iter()
        .position(|x| *x == a)
        .unwrap_or(1)
}

fn angle_code(a: TextAngle) -> i32 {
    match a {
        TextAngle::AsInserted => 0,
        TextAngle::KeepHorizontal => 1,
        TextAngle::RightReading => 2,
    }
}

fn leader_code(t: MLeaderLineType) -> i32 {
    match t {
        MLeaderLineType::None => 0,
        MLeaderLineType::Straight => 1,
        MLeaderLineType::Spline => 2,
    }
}

fn point(out: &mut Out, code: i32, x: f64, y: f64) -> Result<(), FileError> {
    out.point3(code, finite(x)?, finite(y)?, 0.0);
    Ok(())
}

/// What follows the entity header (`0`, `5`, `330`, `100 AcDbEntity`, layer):
/// the whole of the multileader's own data.
pub(super) fn entity(
    out: &mut Out,
    ids: &Ids,
    document: &Document,
    m: &MLeader,
) -> Result<(), FileError> {
    let style = document
        .mleader_style_named(&m.style_name)
        .cloned()
        .unwrap_or_else(MLeaderStyle::standard);
    let style_handle = document
        .mleader_styles()
        .iter()
        .position(|s| s.id == style.id)
        .and_then(|i| ids.mleader_styles.get(i))
        .copied()
        .unwrap_or_default();
    let text_style = |id| {
        document
            .text_styles()
            .iter()
            .position(|s| s.id == id)
            .and_then(|i| ids.text_styles.get(i))
            .copied()
            .unwrap_or(ids.text_styles.first().copied().unwrap_or_default())
    };
    let line = m.lines.first().map(Vec::as_slice).unwrap_or_default();
    let (Some(tip), Some(last)) = (line.first().copied(), line.last().copied()) else {
        return Err(FileError::Invalid("a multileader has no line".into()));
    };
    let end = m.landing_end();
    let (left, right) = (
        attachment_code(style.content.left_attachment),
        attachment_code(style.content.right_attachment),
    );
    let mtext = m.text();
    let height = mtext.map_or(style.content.text_height, |t| t.text.nominal_height);

    out.pair(100, "AcDbMLeader");
    out.pair(270, 2);
    out.pair(300, "CONTEXT_DATA{");
    out.pair(40, 1.0);
    point(out, 10, end.x, end.y)?;
    out.pair(41, finite(height)?);
    out.pair(140, finite(m.look.arrow_size)?);
    out.pair(145, finite(m.look.landing_gap)?);
    out.pair(174, left);
    out.pair(175, right);
    out.pair(176, 0);
    out.pair(177, 0);
    if let Some(t) = mtext {
        let text = &t.text;
        out.pair(290, 1);
        out.pair(304, escaped(&text.contents));
        out.point3(11, 0.0, 0.0, 1.0);
        out.pair(340, hex(text_style(text.style)));
        point(out, 12, text.insertion.x, text.insertion.y)?;
        let d = text.direction;
        out.point3(13, finite(d.x)?, finite(d.y)?, 0.0);
        out.pair(42, 0.0);
        out.pair(43, finite(text.reference_width)?);
        out.pair(44, 0.0);
        out.pair(45, 1.0);
        out.pair(170, 1);
        out.pair(90, BY_LAYER);
        out.pair(171, 1);
        out.pair(172, 5);
        out.pair(91, BY_LAYER);
        out.pair(141, 0.0);
        out.pair(92, 0);
        out.pair(291, 0);
        out.pair(292, 0);
        out.pair(173, 0);
        out.pair(293, 0);
        out.pair(142, 0.0);
        out.pair(143, 0.0);
        out.pair(294, 0);
        out.pair(295, 0);
    } else {
        out.pair(290, 0);
    }
    out.pair(296, 0);
    point(out, 110, tip.x, tip.y)?;
    out.point3(111, 1.0, 0.0, 0.0);
    out.point3(112, 0.0, 1.0, 0.0);
    out.pair(297, 0);
    out.pair(302, "LEADER{");
    out.pair(290, 1);
    out.pair(291, 1);
    point(out, 10, last.x, last.y)?;
    out.point3(11, finite(m.landing_dir.x)?, finite(m.landing_dir.y)?, 0.0);
    out.pair(90, 0);
    out.pair(40, finite(m.look.dogleg_length)?);
    out.pair(304, "LEADER_LINE{");
    for p in &line[..line.len() - 1] {
        point(out, 10, p.x, p.y)?;
    }
    out.pair(91, 0);
    out.pair(305, "}");
    out.pair(271, 0);
    out.pair(303, "}");
    out.pair(272, ATTACH_DIRECTION);
    out.pair(273, ATTACH_DIRECTION);
    out.pair(301, "}");

    out.pair(340, hex(style_handle));
    out.pair(90, 0);
    out.pair(170, leader_code(m.look.leader_type));
    out.pair(91, colour_code(&style.leader_color));
    out.pair(341, hex(ids.bylayer_ltype));
    out.pair(171, weight_code(&style.leader_lineweight));
    out.pair(290, 1);
    out.pair(291, i32::from(m.look.dogleg));
    out.pair(41, finite(m.look.dogleg_length)?);
    out.pair(42, finite(m.look.arrow_size)?);
    out.pair(172, if mtext.is_some() { 2 } else { 0 });
    out.pair(343, hex(text_style(style.content.text_style)));
    out.pair(173, left);
    out.pair(95, right);
    out.pair(174, angle_code(style.content.text_angle));
    out.pair(175, 0);
    out.pair(92, colour_code(&style.content.text_color));
    out.pair(292, i32::from(mtext.is_some_and(|t| t.frame)));
    out.pair(93, BY_LAYER);
    out.point3(10, 1.0, 1.0, 1.0);
    out.pair(43, 0.0);
    out.pair(176, 0);
    out.pair(293, 0);
    out.pair(294, 0);
    out.pair(178, 0);
    out.pair(179, 1);
    out.pair(45, 1.0);
    out.pair(271, 0);
    out.pair(272, ATTACH_DIRECTION);
    out.pair(273, ATTACH_DIRECTION);
    out.pair(295, 0);
    Ok(())
}

/// The `ACAD_MLEADERSTYLE` dictionary and a `MLEADERSTYLE` object for each
/// style of the table. `root` owns the dictionary.
pub(super) fn styles(
    out: &mut Out,
    ids: &Ids,
    document: &Document,
    root: u32,
) -> Result<(), FileError> {
    let dictionary = ids.mleader_dict;
    out.pair(0, "DICTIONARY");
    out.pair(5, hex(dictionary));
    out.pair(330, hex(root));
    out.pair(100, "AcDbDictionary");
    out.pair(281, 1);
    for (style, handle) in document.mleader_styles().iter().zip(&ids.mleader_styles) {
        out.pair(3, dxf_name(&style.name));
        out.pair(350, hex(*handle));
    }
    for (style, handle) in document.mleader_styles().iter().zip(&ids.mleader_styles) {
        style_object(out, ids, document, style, *handle)?;
    }
    Ok(())
}

fn style_object(
    out: &mut Out,
    ids: &Ids,
    document: &Document,
    s: &MLeaderStyle,
    handle: u32,
) -> Result<(), FileError> {
    let c = &s.content;
    let text_style = document
        .text_styles()
        .iter()
        .position(|t| t.id == c.text_style)
        .and_then(|i| ids.text_styles.get(i))
        .copied()
        .unwrap_or_default();
    out.pair(0, "MLEADERSTYLE");
    out.pair(5, hex(handle));
    out.pair(102, "{ACAD_REACTORS");
    out.pair(330, hex(ids.mleader_dict));
    out.pair(102, "}");
    out.pair(330, hex(ids.mleader_dict));
    out.pair(100, "AcDbMLeaderStyle");
    out.pair(179, 2);
    out.pair(
        170,
        if c.kind == MLeaderContentType::MText {
            2
        } else {
            0
        },
    );
    out.pair(171, 1);
    out.pair(172, 0);
    out.pair(90, s.max_points.unwrap_or(0));
    out.pair(40, finite(s.first_angle.unwrap_or(0.0))?);
    out.pair(41, finite(s.second_angle.unwrap_or(0.0))?);
    out.pair(173, leader_code(s.leader_type));
    out.pair(91, colour_code(&s.leader_color));
    out.pair(340, hex(ids.bylayer_ltype));
    out.pair(92, weight_code(&s.leader_lineweight));
    out.pair(290, 1);
    out.pair(42, finite(c.landing_gap)?);
    out.pair(291, i32::from(s.dogleg));
    out.pair(43, finite(s.dogleg_length)?);
    out.pair(3, escaped(&s.description));
    out.pair(44, finite(s.arrow_size)?);
    out.pair(300, escaped(&c.default_text));
    out.pair(342, hex(text_style));
    out.pair(174, attachment_code(c.left_attachment));
    out.pair(178, attachment_code(c.right_attachment));
    out.pair(175, angle_code(c.text_angle));
    out.pair(176, 0);
    out.pair(93, colour_code(&c.text_color));
    out.pair(45, finite(c.text_height)?);
    out.pair(292, i32::from(c.frame));
    out.pair(297, i32::from(c.always_left_justify));
    out.pair(46, finite(c.text_height)?);
    out.pair(94, BY_BLOCK);
    out.pair(47, 1.0);
    out.pair(49, 1.0);
    out.pair(140, 1.0);
    out.pair(293, 1);
    out.pair(141, 0.0);
    out.pair(294, 1);
    out.pair(177, 0);
    out.pair(142, finite(s.overall_scale)?);
    out.pair(295, 0);
    out.pair(296, 0);
    out.pair(143, finite(s.break_gap)?);
    out.pair(271, 0);
    out.pair(272, ATTACH_DIRECTION);
    out.pair(273, ATTACH_DIRECTION);
    out.pair(298, 0);
    Ok(())
}

/// Whether a drawing has anything that needs the multileader objects.
pub(super) fn has_any(document: &Document) -> bool {
    document
        .objects()
        .iter()
        .any(|o| matches!(o.entity, od_core::Entity::MLeader(_)))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use od_core::mleader::MLeaderContentType;
    use od_core::{Entity, MLeaderStyle, Point};

    use super::*;
    use crate::dxf::read::read as read_dxf;
    use crate::dxf::write2000::write;

    fn groups(bytes: &[u8]) -> Vec<(i32, String)> {
        let text = String::from_utf8_lossy(bytes).replace('\r', "");
        let lines: Vec<&str> = text.lines().collect();
        lines
            .chunks(2)
            .filter_map(|c| Some((c[0].trim().parse().ok()?, c.get(1)?.trim().to_owned())))
            .collect()
    }

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn drawing() -> Document {
        let mut doc = Document::default();
        let mut notes = MLeaderStyle::new(od_core::MLeaderStyleId(1), "Notes");
        notes.max_points = None;
        notes.content.frame = true;
        notes.overall_scale = 2.0;
        notes.dogleg_length = 5.0;
        let mut bare = MLeaderStyle::new(od_core::MLeaderStyleId(2), "Bare");
        bare.content.kind = MLeaderContentType::None;
        doc.set_mleader_styles(
            vec![MLeaderStyle::standard(), notes.clone(), bare.clone()],
            od_core::MLeaderStyleId::STANDARD,
            3,
        );
        let make = |style: &MLeaderStyle, points: Vec<Point>, text: &str| {
            Entity::MLeader(Box::new(MLeader::build(style, points, text).unwrap()))
        };
        doc.add(make(
            &MLeaderStyle::standard(),
            vec![p(0.0, 0.0), p(10.0, 5.0)],
            "Note",
        ));
        doc.add(make(
            &MLeaderStyle::standard(),
            vec![p(0.0, 0.0), p(-10.0, 5.0)],
            "Left\\Pside",
        ));
        doc.add(make(
            &notes,
            vec![p(1.0, 1.0), p(4.0, 6.0), p(9.0, 6.0)],
            "Framed",
        ));
        doc.add(make(&bare, vec![p(2.0, 2.0), p(6.0, 3.0)], ""));
        doc
    }

    fn leaders(doc: &Document) -> Vec<MLeader> {
        doc.objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::MLeader(m) => Some((**m).clone()),
                _ => None,
            })
            .collect()
    }

    /// A 2007 file's class records all say how many objects they have.
    #[test]
    fn every_class_of_a_2007_file_has_an_instance_count() {
        let doc = drawing();
        let made = doc
            .objects()
            .iter()
            .filter(|o| matches!(o.entity, Entity::MLeader(_)))
            .count();
        let pairs = groups(&write(&doc).unwrap().bytes);
        let mut classes = 0;
        for (i, (code, value)) in pairs.iter().enumerate() {
            if (*code, value.as_str()) != (0, "CLASS") {
                continue;
            }
            classes += 1;
            let record: Vec<_> = pairs[i + 1..].iter().take_while(|(c, _)| *c != 0).collect();
            let count = record.iter().find(|(c, _)| *c == 91);
            assert!(count.is_some(), "class {record:?} has no group 91");
            if record.iter().any(|(_, v)| v == "MULTILEADER") {
                assert_eq!(count.unwrap().1, made.to_string());
            }
        }
        assert_eq!(classes, 5);
    }

    #[test]
    fn what_is_written_reads_back_as_the_same_multileaders_and_styles() {
        let doc = drawing();
        let bytes = write(&doc).unwrap().bytes;
        let back = read_dxf(&bytes).unwrap();
        assert!(back.warnings.is_empty(), "{:?}", back.warnings);
        assert_eq!(leaders(&back.document), leaders(&doc));
        for style in doc.mleader_styles() {
            let read = back.document.mleader_style_named(&style.name).unwrap();
            assert_eq!(read.content.frame, style.content.frame);
            assert_eq!(read.dogleg_length, style.dogleg_length);
            assert_eq!(read.overall_scale, style.overall_scale);
            assert_eq!(read.content.kind, style.content.kind);
            assert_eq!(read.max_points, style.max_points);
        }
        assert!(back.document.validate().is_ok());
    }

    #[test]
    fn the_file_is_2007_with_its_classes_and_every_reference_resolves() {
        let bytes = write(&drawing()).unwrap().bytes;
        let all = groups(&bytes);
        let version = all.iter().position(|(c, v)| *c == 9 && v == "$ACADVER");
        assert_eq!(all[version.unwrap() + 1].1, "AC1021");
        for class in ["MLEADERSTYLE", "MULTILEADER"] {
            assert!(all.iter().any(|(c, v)| *c == 1 && v == class), "{class}");
        }
        let mut handles = HashSet::new();
        for (code, value) in &all {
            if matches!(code, 5 | 105) {
                assert!(handles.insert(value.clone()), "handle {value} twice");
            }
        }
        for (code, value) in &all {
            if matches!(code, 330 | 340 | 341 | 343 | 342 | 350) && value != "0" {
                assert!(handles.contains(value), "{code} {value} points at nothing");
            }
        }
        let seed_at = all
            .iter()
            .position(|(c, v)| *c == 9 && v == "$HANDSEED")
            .unwrap();
        let seed = u32::from_str_radix(&all[seed_at + 1].1, 16).unwrap();
        handles.remove(&all[seed_at + 1].1);
        let top = handles
            .iter()
            .map(|h| u32::from_str_radix(h, 16).unwrap())
            .max();
        assert!(top < Some(seed), "highest handle {top:x?}, seed {seed:x}");
    }
}
