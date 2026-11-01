//! The native format's description of a multileader.

use od_core::mleader::{MLeader, MLeaderContent, MLeaderLook, MLeaderText};
use od_core::{Arrow, Point};
use serde::{Deserialize, Serialize};

use super::entity::{from_point, invalid, point, PointDto};
use super::mleader_style::LineTypeDto;
use super::text::MTextDto;
use crate::FileError;

#[derive(Serialize, Deserialize)]
struct LookDto {
    leader_type: LineTypeDto,
    /// `Arrow::code`.
    arrow: i64,
    arrow_size: f64,
    dogleg: bool,
    dogleg_length: f64,
    landing_gap: f64,
}

#[derive(Serialize, Deserialize)]
struct TextDto {
    text: MTextDto,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    frame: bool,
}

#[derive(Serialize, Deserialize)]
pub(super) struct MLeaderDto {
    style: String,
    look: LookDto,
    lines: Vec<Vec<PointDto>>,
    landing_dir: PointDto,
    /// Left out for a multileader with no text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content: Option<TextDto>,
}

impl MLeaderDto {
    pub(super) fn from_mleader(m: &MLeader) -> Result<Self, FileError> {
        if let Err(why) = m.validate() {
            return invalid(format!("a multileader is not valid: {why}"));
        }
        Ok(Self {
            style: m.style_name.clone(),
            look: LookDto {
                leader_type: m.look.leader_type.into(),
                arrow: m.look.arrow.code(),
                arrow_size: m.look.arrow_size,
                dogleg: m.look.dogleg,
                dogleg_length: m.look.dogleg_length,
                landing_gap: m.look.landing_gap,
            },
            lines: m
                .lines
                .iter()
                .map(|l| l.iter().map(|p| point(*p)).collect())
                .collect(),
            landing_dir: point(m.landing_dir),
            content: m.text().map(|t| TextDto {
                text: MTextDto::from_mtext(&t.text),
                frame: t.frame,
            }),
        })
    }

    pub(super) fn to_mleader(&self) -> Result<MLeader, FileError> {
        let Some(arrow) = Arrow::from_code(self.look.arrow) else {
            return invalid("a multileader has an unknown arrowhead");
        };
        let lines = self
            .lines
            .iter()
            .map(|l| {
                l.iter()
                    .map(|p| from_point(*p))
                    .collect::<Result<Vec<Point>, _>>()
            })
            .collect::<Result<_, _>>()?;
        let content = match &self.content {
            Some(t) => MLeaderContent::MText(Box::new(MLeaderText {
                text: t.text.to_mtext()?,
                frame: t.frame,
            })),
            None => MLeaderContent::None,
        };
        let m = MLeader {
            style_name: self.style.clone(),
            look: MLeaderLook {
                leader_type: self.look.leader_type.into(),
                arrow,
                arrow_size: self.look.arrow_size,
                dogleg: self.look.dogleg,
                dogleg_length: self.look.dogleg_length,
                landing_gap: self.look.landing_gap,
            },
            lines,
            landing_dir: from_point(self.landing_dir)?,
            content,
        };
        match m.validate() {
            Ok(()) => Ok(m),
            Err(why) => invalid(format!("a multileader is not valid: {why}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use od_core::mleader::{LeaderAttachment, MLeaderStyle};
    use od_core::{Document, Entity};

    use crate::native::tests::{read, roundtrip, MINIMAL};
    use crate::native::NativeFormat;
    use crate::FileFormat;

    use super::*;

    fn with_mleaders() -> Document {
        let mut document = Document::default();
        let mut style = MLeaderStyle::standard();
        style.content.frame = true;
        style.content.left_attachment = LeaderAttachment::MiddleOfText;
        style.dogleg_length = 12.0;
        let plain = MLeader::build(
            &MLeaderStyle::standard(),
            vec![Point::new(0.0, 0.0), Point::new(10.0, 5.0)],
            "Hello",
        )
        .unwrap();
        let framed = MLeader::build(
            &style,
            vec![
                Point::new(0.0, 0.0),
                Point::new(-4.0, 3.0),
                Point::new(-10.0, 3.0),
            ],
            "A\\Pframed note",
        )
        .unwrap();
        let mut bare_style = MLeaderStyle::standard();
        bare_style.content.kind = od_core::mleader::MLeaderContentType::None;
        let bare = MLeader::build(
            &bare_style,
            vec![Point::new(1.0, 1.0), Point::new(2.0, 2.0)],
            "",
        )
        .unwrap();
        for m in [plain, framed, bare] {
            document.add(Entity::MLeader(Box::new(m)));
        }
        document
    }

    #[test]
    fn multileaders_survive_a_round_trip_exactly() {
        let document = with_mleaders();
        let loaded = roundtrip(&document);
        assert_eq!(loaded, document);
        let text = String::from_utf8(NativeFormat.write(&document).unwrap().bytes).unwrap();
        assert!(text.contains("\"type\": \"mleader\""));
        assert!(text.contains("\"version\": 0"));
    }

    fn file(entity: &str) -> String {
        MINIMAL.replace(
            "\"objects\": []",
            &format!("\"objects\": [{{\"id\": 0, \"layer\": 0, \"entity\": {entity}}}]"),
        )
    }

    #[test]
    fn a_bad_multileader_is_refused_not_repaired() {
        let look = r#""look": {"leader_type": "straight", "arrow": 0, "arrow_size": 2.5,
            "dogleg": true, "dogleg_length": 8.0, "landing_gap": 2.0}"#;
        let good = format!(
            r#"{{"type": "mleader", "style": "Standard", {look},
            "lines": [[[0.0, 0.0], [5.0, 5.0]]], "landing_dir": [1.0, 0.0]}}"#
        );
        let loaded = read(&file(&good));
        assert!(loaded.is_ok(), "{:?}", loaded.err());
        for bad in [
            good.replace("[[[0.0, 0.0], [5.0, 5.0]]]", "[[[0.0, 0.0]]]"),
            good.replace("[[[0.0, 0.0], [5.0, 5.0]]]", "[]"),
            good.replace("[1.0, 0.0]", "[0.0, 0.0]"),
            good.replace("\"arrow\": 0", "\"arrow\": 99"),
            good.replace("2.5", "-2.5"),
        ] {
            assert!(read(&file(&bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_file_of_another_version_is_still_refused() {
        let newer = MINIMAL.replace("\"version\": 0", "\"version\": 1");
        assert!(matches!(
            read(&newer),
            Err(crate::FileError::WrongVersion { .. })
        ));
    }
}
