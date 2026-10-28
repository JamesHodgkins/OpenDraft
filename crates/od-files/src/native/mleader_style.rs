//! The native format's description of multileader styles.

use std::collections::HashSet;

use od_core::mleader::{
    LeaderAttachment, MLeaderContentType, MLeaderLineType, MLeaderStyle, MLeaderStyleContent,
    MLeaderStyleId, TextAngle,
};
use od_core::{Document, Lineweight, TextStyleId};
use serde::{Deserialize, Serialize};

use super::entity::invalid;
use super::names::{mirror, ArrowDto};
use super::{attr_from_dto, attr_to_dto, AttrDto, ColorDto};
use crate::FileError;

mirror!(LineTypeDto <-> MLeaderLineType { None, Straight, Spline });
mirror!(ContentTypeDto <-> MLeaderContentType { None, MText });
mirror!(AngleDto <-> TextAngle { KeepHorizontal, RightReading, AsInserted });
mirror!(AttachmentDto <-> LeaderAttachment {
    TopOfTopLine,
    MiddleOfTopLine,
    MiddleOfText,
    MiddleOfBottomLine,
    BottomOfBottomLine,
    BottomLine,
    UnderlineBottomLine,
    UnderlineTopLine,
    UnderlineAllText,
});

#[derive(Serialize, Deserialize)]
struct ContentDto {
    kind: ContentTypeDto,
    default_text: String,
    text_style: u64,
    text_angle: AngleDto,
    text_color: AttrDto<ColorDto>,
    text_height: f64,
    always_left_justify: bool,
    frame: bool,
    left_attachment: AttachmentDto,
    right_attachment: AttachmentDto,
    landing_gap: f64,
    extend_to_text: bool,
}

impl ContentDto {
    fn from_content(c: &MLeaderStyleContent) -> Self {
        Self {
            kind: c.kind.into(),
            default_text: c.default_text.clone(),
            text_style: c.text_style.0,
            text_angle: c.text_angle.into(),
            text_color: attr_to_dto(&c.text_color, |c| (*c).into()),
            text_height: c.text_height,
            always_left_justify: c.always_left_justify,
            frame: c.frame,
            left_attachment: c.left_attachment.into(),
            right_attachment: c.right_attachment.into(),
            landing_gap: c.landing_gap,
            extend_to_text: c.extend_to_text,
        }
    }

    fn into_content(self) -> MLeaderStyleContent {
        MLeaderStyleContent {
            kind: self.kind.into(),
            default_text: self.default_text,
            text_style: TextStyleId(self.text_style),
            text_angle: self.text_angle.into(),
            text_color: attr_from_dto(self.text_color, |c| c.into()),
            text_height: self.text_height,
            always_left_justify: self.always_left_justify,
            frame: self.frame,
            left_attachment: self.left_attachment.into(),
            right_attachment: self.right_attachment.into(),
            landing_gap: self.landing_gap,
            extend_to_text: self.extend_to_text,
        }
    }
}

/// Everything about a style but who it is, all written.
#[derive(Serialize, Deserialize)]
struct BodyDto {
    description: String,
    leader_type: LineTypeDto,
    leader_color: AttrDto<ColorDto>,
    leader_linetype: AttrDto<String>,
    leader_lineweight: AttrDto<u16>,
    arrow: ArrowDto,
    arrow_size: f64,
    break_gap: f64,
    max_points: Option<u32>,
    /// Radians.
    first_angle: Option<f64>,
    second_angle: Option<f64>,
    dogleg: bool,
    dogleg_length: f64,
    overall_scale: f64,
    content: ContentDto,
}

impl BodyDto {
    fn of(s: &MLeaderStyle) -> Self {
        Self {
            description: s.description.clone(),
            leader_type: s.leader_type.into(),
            leader_color: attr_to_dto(&s.leader_color, |c| (*c).into()),
            leader_linetype: attr_to_dto(&s.leader_linetype, String::clone),
            leader_lineweight: attr_to_dto(&s.leader_lineweight, |w| w.0),
            arrow: s.arrow.into(),
            arrow_size: s.arrow_size,
            break_gap: s.break_gap,
            max_points: s.max_points,
            first_angle: s.first_angle,
            second_angle: s.second_angle,
            dogleg: s.dogleg,
            dogleg_length: s.dogleg_length,
            overall_scale: s.overall_scale,
            content: ContentDto::from_content(&s.content),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct MLeaderStyleDto {
    id: u64,
    name: String,
    #[serde(flatten)]
    body: BodyDto,
}

impl MLeaderStyleDto {
    pub(super) fn from_style(style: &MLeaderStyle) -> Self {
        Self {
            id: style.id.0,
            name: style.name.clone(),
            body: BodyDto::of(style),
        }
    }

    fn into_style(self) -> Result<MLeaderStyle, FileError> {
        let b = self.body;
        Ok(MLeaderStyle {
            id: MLeaderStyleId(self.id),
            name: self.name,
            description: b.description,
            leader_type: b.leader_type.into(),
            leader_color: attr_from_dto(b.leader_color, |c| c.into()),
            leader_linetype: attr_from_dto(b.leader_linetype, |n| n),
            leader_lineweight: attr_from_dto(b.leader_lineweight, Lineweight),
            arrow: b.arrow.into(),
            arrow_size: b.arrow_size,
            break_gap: b.break_gap,
            max_points: b.max_points,
            first_angle: b.first_angle,
            second_angle: b.second_angle,
            dogleg: b.dogleg,
            dogleg_length: b.dogleg_length,
            overall_scale: b.overall_scale,
            content: b.content.into_content(),
        })
    }
}

/// The multileader styles in full, the current one and the id counter.
#[derive(Serialize, Deserialize)]
pub(super) struct TableDto {
    mleader_styles: Vec<MLeaderStyleDto>,
    current_mleader_style: u64,
    next_mleader_style_id: u64,
}

impl TableDto {
    pub(super) fn of(document: &Document) -> Self {
        Self {
            mleader_styles: document
                .mleader_styles()
                .iter()
                .map(MLeaderStyleDto::from_style)
                .collect(),
            current_mleader_style: document.current_mleader_style().0,
            next_mleader_style_id: document.next_mleader_style_id(),
        }
    }

    /// Put the file's styles in the drawing: `Standard` first and the current
    /// style among them.
    pub(super) fn load(self, document: &mut Document) -> Result<(), FileError> {
        let (mut ids, mut names) = (HashSet::new(), HashSet::new());
        let mut table = Vec::with_capacity(self.mleader_styles.len());
        for dto in self.mleader_styles {
            let style = dto.into_style()?;
            if !ids.insert(style.id) {
                return invalid(format!("multileader style {} appears twice", style.id.0));
            }
            if !names.insert(style.name.to_ascii_lowercase()) {
                return invalid(format!(
                    "two multileader styles are called \"{}\"",
                    style.name
                ));
            }
            if !style.is_valid() {
                return invalid(format!("multileader style \"{}\" is not valid", style.name));
            }
            if document.text_style(style.content.text_style).is_none() {
                return invalid(format!(
                    "multileader style \"{}\" uses a text style that does not exist",
                    style.name
                ));
            }
            table.push(style);
        }
        if table.first().map(|s| s.id) != Some(MLeaderStyleId::STANDARD) {
            return invalid("there is no multileader style 0 first");
        }
        let current = MLeaderStyleId(self.current_mleader_style);
        if !ids.contains(&current) {
            return invalid(format!(
                "the current multileader style {} does not exist",
                current.0
            ));
        }
        let floor = table.iter().map(|s| s.id.0 + 1).max().unwrap_or(1);
        document.set_mleader_styles(table, current, self.next_mleader_style_id.max(floor));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use od_core::mleader::MLeaderStyleContent;
    use od_core::{Arrow, Attr, Color};

    use crate::native::tests::{read, roundtrip};
    use crate::native::NativeFormat;
    use crate::FileFormat;

    use super::*;

    fn with_styles() -> Document {
        let mut document = Document::default();
        let notes = MLeaderStyleId(document.next_mleader_style_id());
        let detail = MLeaderStyleId(notes.0 + 1);
        let mut table = document.mleader_styles().to_vec();
        table.push(MLeaderStyle {
            description: "Notes in red".into(),
            leader_type: MLeaderLineType::Spline,
            leader_color: Attr::Set(Color::Rgb(200, 0, 0)),
            leader_linetype: Attr::Set("DASHED".into()),
            leader_lineweight: Attr::Set(Lineweight(70)),
            arrow: Arrow::Open,
            arrow_size: 3.0,
            max_points: None,
            first_angle: Some(0.5),
            second_angle: Some(1.0),
            dogleg: false,
            dogleg_length: 5.0,
            overall_scale: 2.0,
            content: MLeaderStyleContent {
                default_text: "NOTE".into(),
                text_angle: TextAngle::RightReading,
                text_color: Attr::Set(Color::Foreground),
                frame: true,
                left_attachment: LeaderAttachment::UnderlineAllText,
                ..MLeaderStyleContent::default()
            },
            ..MLeaderStyle::new(notes, "Notes")
        });
        table.push(MLeaderStyle {
            content: MLeaderStyleContent {
                kind: MLeaderContentType::None,
                ..MLeaderStyleContent::default()
            },
            ..MLeaderStyle::new(detail, "Detail")
        });
        document.set_mleader_styles(table, notes, detail.0 + 1);
        document
    }

    #[test]
    fn styles_survive_a_round_trip_exactly() {
        let document = with_styles();
        document.validate().unwrap();
        assert_eq!(roundtrip(&document), document);
    }

    #[test]
    fn a_drawing_with_stock_styles_writes_them_in_full_and_a_file_without_is_refused() {
        let text =
            String::from_utf8(NativeFormat.write(&Document::default()).unwrap().bytes).unwrap();
        assert!(text.contains("mleader_styles") && text.contains("\"landing_gap\""));
        let loaded = roundtrip(&Document::default());
        assert_eq!(loaded.mleader_styles().len(), 1);
        assert_eq!(loaded.current_mleader_style(), MLeaderStyleId::STANDARD);
        assert_eq!(loaded.next_mleader_style_id(), 1);
        let bare = super::super::tests::MINIMAL;
        assert!(NativeFormat.read(bare.as_bytes()).is_err());
    }

    #[test]
    fn a_changed_current_style_alone_is_kept() {
        let mut document = Document::default();
        let table = document.mleader_styles().to_vec();
        document.set_mleader_styles(table, MLeaderStyleId::STANDARD, 4);
        let loaded = roundtrip(&document);
        assert_eq!(loaded.next_mleader_style_id(), 4);
    }

    #[test]
    fn ids_are_not_reused_after_loading() {
        let document = with_styles();
        let loaded = roundtrip(&document);
        assert_eq!(
            loaded.next_mleader_style_id(),
            document.next_mleader_style_id()
        );
    }

    fn file(styles: &str) -> String {
        super::super::tests::MINIMAL.replace(
            "\"objects\": []",
            &format!("\"objects\": [], \"mleader_styles\": [{styles}]"),
        )
    }

    /// The stock style as the file writes it, under another id and name.
    fn full(id: u64, name: &str) -> serde_json::Value {
        let mut style =
            serde_json::to_value(MLeaderStyleDto::from_style(&MLeaderStyle::standard())).unwrap();
        style["id"] = id.into();
        style["name"] = name.into();
        style
    }

    fn file_of(styles: &[serde_json::Value]) -> String {
        let joined: Vec<String> = styles.iter().map(ToString::to_string).collect();
        file(&joined.join(", "))
    }

    #[test]
    fn a_style_missing_a_value_is_refused_not_filled_from_the_stock_style() {
        assert!(read(&file_of(&[full(0, "Standard")])).is_ok());
        assert!(read(&file(r#"{"id": 0, "name": "Standard"}"#)).is_err());
    }

    #[test]
    fn bad_tables_are_refused() {
        let with = |key: &str, value: serde_json::Value| {
            let mut style = full(0, "Standard");
            style[key] = value;
            style
        };
        let mut no_text_style = full(0, "Standard");
        no_text_style["content"]["text_style"] = 9.into();
        let cases = [
            // Twice the same id, twice the same name, no Standard first,
            // a negative size, a missing text style and an unknown arrow.
            vec![full(0, "Standard"), full(0, "B")],
            vec![full(0, "Standard"), full(1, "STANDARD")],
            vec![full(3, "Standard")],
            vec![with("arrow_size", (-1.0).into())],
            vec![no_text_style],
            vec![with("arrow", 99.into())],
        ];
        for styles in cases {
            assert!(read(&file_of(&styles)).is_err(), "{styles:?}");
        }
    }

    #[test]
    fn a_current_style_that_does_not_exist_is_refused() {
        let text =
            String::from_utf8(NativeFormat.write(&Document::default()).unwrap().bytes).unwrap();
        let mut file: serde_json::Value = serde_json::from_str(&text).unwrap();
        file["drawing"]["current_mleader_style"] = 7.into();
        assert!(NativeFormat.read(file.to_string().as_bytes()).is_err());
    }
}
