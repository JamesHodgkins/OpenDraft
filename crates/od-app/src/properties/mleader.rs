//! The Properties rows of a multileader, in AutoCAD's words: the leader
//! (type, arrowhead and its size), the landing (on or off, its length and the
//! gap to the text) and the text (height, frame). The points and the text's
//! words are edited with grips and the text editor.
//!
//! These are the values the multileader keeps from its style; a spline
//! leader type is shown but cannot be chosen, as it is drawn straight.

use od_core::dimension::Arrow;
use od_core::mleader::{MLeader, MLeaderContent, MLeaderLineType};
use od_core::Point;

use super::{row, Field, Row, Unit};

const WITH_TEXT: &[Row] = &[
    row(Field::LeaderType, "Leader type", Unit::LeaderType, true),
    row(Field::Arrowhead, "Arrowhead", Unit::Arrowhead, true),
    row(Field::ArrowSize, "Arrowhead size", Unit::Length, true),
    row(Field::Lines, "Leader lines", Unit::Count, false),
    row(Field::Vertices, "Vertices", Unit::Count, false),
    row(Field::Dogleg, "Use landing", Unit::Flag, true),
    row(Field::DoglegLength, "Landing distance", Unit::Length, true),
    row(Field::LandingGap, "Landing gap", Unit::Length, true),
    row(Field::ContentKind, "Content type", Unit::Content, false),
    row(Field::Height, "Text height", Unit::Length, true),
    row(Field::Frame, "Text frame", Unit::Flag, true),
];

const NO_CONTENT: &[Row] = &[
    row(Field::LeaderType, "Leader type", Unit::LeaderType, true),
    row(Field::Arrowhead, "Arrowhead", Unit::Arrowhead, true),
    row(Field::ArrowSize, "Arrowhead size", Unit::Length, true),
    row(Field::Lines, "Leader lines", Unit::Count, false),
    row(Field::Vertices, "Vertices", Unit::Count, false),
    row(Field::Dogleg, "Use landing", Unit::Flag, true),
    row(Field::DoglegLength, "Landing distance", Unit::Length, true),
    row(Field::ContentKind, "Content type", Unit::Content, false),
];

pub(super) fn rows(m: &MLeader) -> &'static [Row] {
    match m.content {
        MLeaderContent::MText(_) => WITH_TEXT,
        MLeaderContent::None => NO_CONTENT,
    }
}

fn flag(on: bool) -> f64 {
    if on {
        1.0
    } else {
        0.0
    }
}

pub(super) fn line_type_code(t: MLeaderLineType) -> f64 {
    MLeaderLineType::ALL
        .iter()
        .position(|a| *a == t)
        .map_or(0.0, |i| i as f64)
}

pub(super) fn value(m: &MLeader, field: Field) -> f64 {
    let text = m.text();
    match field {
        Field::LeaderType => line_type_code(m.look.leader_type),
        Field::Arrowhead => m.look.arrow.code() as f64,
        Field::ArrowSize => m.look.arrow_size,
        Field::Lines => m.lines.len() as f64,
        Field::Vertices => m.lines.iter().map(Vec::len).sum::<usize>() as f64,
        Field::Dogleg => flag(m.look.dogleg),
        Field::DoglegLength => m.look.dogleg_length,
        Field::LandingGap => m.look.landing_gap,
        Field::ContentKind => flag(text.is_some()),
        Field::Height => text.map_or(f64::NAN, |t| t.text.nominal_height),
        Field::Frame => text.map_or(f64::NAN, |t| flag(t.frame)),
        _ => f64::NAN,
    }
}

/// Where the text sits relative to the landing's end moves with it, so the
/// text keeps its gap when the landing grows or the gap changes.
fn anchor(m: &MLeader) -> Point {
    let end = m.landing_end();
    Point::new(
        end.x + m.landing_dir.x * m.look.landing_gap,
        end.y + m.landing_dir.y * m.look.landing_gap,
    )
}

fn follow(before: Point, m: &mut MLeader) {
    let after = anchor(m);
    if let MLeaderContent::MText(t) = &mut m.content {
        t.text.insertion.x += after.x - before.x;
        t.text.insertion.y += after.y - before.y;
    }
}

pub(super) fn with_value(m: &MLeader, field: Field, new: f64) -> Option<MLeader> {
    let mut out = m.clone();
    let before = anchor(m);
    let size = new >= 0.0;
    match field {
        Field::LeaderType => {
            let t = *MLeaderLineType::ALL.get(new.round() as usize)?;
            if t == MLeaderLineType::Spline {
                return None;
            }
            out.look.leader_type = t;
        }
        Field::Arrowhead => out.look.arrow = Arrow::from_code(new.round() as i64)?,
        Field::ArrowSize if size => out.look.arrow_size = new,
        Field::Dogleg => {
            out.look.dogleg = new != 0.0;
            follow(before, &mut out);
        }
        Field::DoglegLength if size => {
            out.look.dogleg_length = new;
            follow(before, &mut out);
        }
        Field::LandingGap if size => {
            out.look.landing_gap = new;
            follow(before, &mut out);
        }
        Field::Height if new > 0.0 => match &mut out.content {
            MLeaderContent::MText(t) => t.text.nominal_height = new,
            MLeaderContent::None => return None,
        },
        Field::Frame => match &mut out.content {
            MLeaderContent::MText(t) => t.frame = new != 0.0,
            MLeaderContent::None => return None,
        },
        _ => return None,
    }
    out.validate().ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use od_core::mleader::{MLeaderContentType, MLeaderStyle};
    use od_core::{Entity, Units};

    use super::super::{format, parse, rows, value, with_value, Field, Unit};
    use super::*;

    fn made(style: &MLeaderStyle) -> Entity {
        let p = Point::new;
        Entity::MLeader(Box::new(
            MLeader::build(style, vec![p(0.0, 0.0), p(10.0, 10.0)], "Note").unwrap(),
        ))
    }

    fn mleader(e: &Entity) -> &MLeader {
        match e {
            Entity::MLeader(m) => m,
            _ => panic!("a multileader"),
        }
    }

    #[test]
    fn the_rows_show_what_the_style_gave_it() {
        let e = made(&MLeaderStyle::standard());
        let labels: Vec<&str> = rows(&e).iter().map(|r| r.label).collect();
        assert!(labels.contains(&"Arrowhead size") && labels.contains(&"Text height"));
        let units = Units::default();
        assert_eq!(
            format(&units, Unit::LeaderType, value(&e, Field::LeaderType)),
            "Straight"
        );
        assert_eq!(
            format(&units, Unit::Arrowhead, value(&e, Field::Arrowhead)),
            "Closed filled"
        );
        assert_eq!(
            format(&units, Unit::Content, value(&e, Field::ContentKind)),
            "MText"
        );
        assert_eq!(value(&e, Field::Lines), 1.0);
        assert_eq!(value(&e, Field::Vertices), 2.0);
    }

    #[test]
    fn a_multileader_without_content_has_no_text_rows() {
        let mut style = MLeaderStyle::standard();
        style.content.kind = MLeaderContentType::None;
        let e = made(&style);
        assert!(rows(&e).iter().all(|r| r.label != "Text height"));
        let units = Units::default();
        assert_eq!(
            format(&units, Unit::Content, value(&e, Field::ContentKind)),
            "None"
        );
        assert!(with_value(&e, Field::Height, 3.0).is_none());
    }

    #[test]
    fn sizes_and_choices_can_be_changed() {
        let e = made(&MLeaderStyle::standard());
        let bigger = with_value(&e, Field::ArrowSize, 7.0).unwrap();
        assert_eq!(value(&bigger, Field::ArrowSize), 7.0);
        let none = with_value(&e, Field::LeaderType, 2.0).unwrap();
        assert_eq!(mleader(&none).look.leader_type, MLeaderLineType::None);
        let open = with_value(&e, Field::Arrowhead, 1.0).unwrap();
        assert_eq!(mleader(&open).look.arrow, Arrow::Open);
        let framed = with_value(&e, Field::Frame, 1.0).unwrap();
        assert_eq!(value(&framed, Field::Frame), 1.0);
        let taller = with_value(&e, Field::Height, 6.0).unwrap();
        assert_eq!(value(&taller, Field::Height), 6.0);
        assert!(with_value(&e, Field::ArrowSize, -1.0).is_none());
        assert!(with_value(&e, Field::Height, 0.0).is_none());
        assert!(
            with_value(&e, Field::LeaderType, 1.0).is_none(),
            "spline is drawn straight"
        );
        assert!(with_value(&e, Field::Lines, 2.0).is_none(), "derived");
    }

    #[test]
    fn the_landing_moves_the_text_with_it() {
        let e = made(&MLeaderStyle::standard());
        let was = mleader(&e).text().unwrap().text.insertion;
        let longer = with_value(&e, Field::DoglegLength, 20.0).unwrap();
        let now = mleader(&longer).text().unwrap().text.insertion;
        assert!((now.x - was.x - (20.0 - 8.0)).abs() < 1e-9);
        assert_eq!(now.y, was.y);
        let off = with_value(&e, Field::Dogleg, 0.0).unwrap();
        assert!(mleader(&off).text().unwrap().text.insertion.x < was.x);
        assert!(mleader(&off).validate().is_ok());
    }

    #[test]
    fn choices_are_typed_by_name_or_number() {
        let units = Units::default();
        assert_eq!(parse(&units, Unit::LeaderType, "none"), Some(2.0));
        assert_eq!(parse(&units, Unit::LeaderType, "Straight"), Some(0.0));
        assert_eq!(parse(&units, Unit::Arrowhead, "open"), Some(1.0));
        assert_eq!(parse(&units, Unit::Arrowhead, "tick"), Some(2.0));
        assert_eq!(parse(&units, Unit::Arrowhead, "wavy"), None);
        assert_eq!(
            parse(&units, Unit::Content, "mtext"),
            None,
            "chosen by the style"
        );
    }
}
