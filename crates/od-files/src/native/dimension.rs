//! How dimensions are written to the native file. A dimension names its style
//! by position in the drawing's pool of dimension styles (see
//! `dimension_style.rs`), which holds each in full.

use od_core::dimension::{Assoc, AssocPoint};
use od_core::{DimKind, DimStyle, DimText, Dimension, EntityId};
use serde::{Deserialize, Serialize};

use super::dimension_style::StylePool;
use super::entity::{from_point, invalid, point, PointDto};
use crate::FileError;

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum DimKindDto {
    Linear {
        p1: PointDto,
        p2: PointDto,
        line_point: PointDto,
        /// Dimension line angle in radians; left out for an aligned one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rotation: Option<f64>,
        /// Extension line angle in radians; left out for square ones.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        oblique: Option<f64>,
    },
    Radial {
        center: PointDto,
        chord_point: PointDto,
        #[serde(default)]
        diameter: bool,
        leader_length: f64,
        /// The centre a jogged radius dimension is shown to head for, and
        /// its jog.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        jog: Option<(PointDto, PointDto)>,
    },
    ArcLength {
        center: PointDto,
        start: PointDto,
        end: PointDto,
        arc_point: PointDto,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        leader: bool,
    },
    Ordinate {
        feature: PointDto,
        leader_end: PointDto,
        x_datum: bool,
    },
    Angular {
        vertex: PointDto,
        p1: PointDto,
        p2: PointDto,
        arc_point: PointDto,
        #[serde(default)]
        lines: bool,
    },
}

/// A defining point tied to a point of an object.
#[derive(Serialize, Deserialize)]
struct AssocDto {
    point: usize,
    target: u64,
    /// `start`, `end`, `center`, `circumference` or `vertex:N`.
    at: String,
}

impl AssocDto {
    fn from_assoc(a: &Assoc) -> Self {
        let at = match a.at {
            AssocPoint::Start => "start".to_owned(),
            AssocPoint::End => "end".to_owned(),
            AssocPoint::Center => "center".to_owned(),
            AssocPoint::Circumference => "circumference".to_owned(),
            AssocPoint::Vertex(i) => format!("vertex:{i}"),
        };
        Self {
            point: a.point,
            target: a.target.0,
            at,
        }
    }

    fn to_assoc(&self) -> Result<Assoc, FileError> {
        let at = match self.at.as_str() {
            "start" => AssocPoint::Start,
            "end" => AssocPoint::End,
            "center" => AssocPoint::Center,
            "circumference" => AssocPoint::Circumference,
            other => match other.strip_prefix("vertex:").and_then(|n| n.parse().ok()) {
                Some(i) => AssocPoint::Vertex(i),
                None => return invalid(format!("a dimension ties to \"{other}\"")),
            },
        };
        Ok(Assoc {
            point: self.point,
            target: EntityId(self.target),
            at,
        })
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct DimensionDto {
    #[serde(flatten)]
    kind: DimKindDto,
    /// AutoCAD's text entry: empty is the measurement, one space hides it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    text: String,
    /// Where the user put the text, when they did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text_pos: Option<PointDto>,
    /// The angle the user turned the text to, in radians.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text_angle: Option<f64>,
    /// Where a jog line is put in the dimension line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    jog_line: Option<PointDto>,
    /// The objects the defining points follow.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    assoc: Vec<AssocDto>,
    /// Its position in the pool of dimension styles.
    style: usize,
}

impl DimensionDto {
    pub(super) fn from_dimension(d: &Dimension, styles: &mut StylePool) -> Self {
        let kind = match &d.kind {
            DimKind::Linear {
                p1,
                p2,
                line_point,
                rotation,
                oblique,
            } => DimKindDto::Linear {
                p1: point(*p1),
                p2: point(*p2),
                line_point: point(*line_point),
                rotation: *rotation,
                oblique: *oblique,
            },
            DimKind::Radial {
                center,
                chord_point,
                diameter,
                leader_length,
                jog,
            } => DimKindDto::Radial {
                center: point(*center),
                chord_point: point(*chord_point),
                diameter: *diameter,
                leader_length: *leader_length,
                jog: jog.map(|(shown, at)| (point(shown), point(at))),
            },
            DimKind::Ordinate {
                feature,
                leader_end,
                x_datum,
            } => DimKindDto::Ordinate {
                feature: point(*feature),
                leader_end: point(*leader_end),
                x_datum: *x_datum,
            },
            DimKind::Angular {
                vertex,
                p1,
                p2,
                arc_point,
                lines,
            } => DimKindDto::Angular {
                vertex: point(*vertex),
                p1: point(*p1),
                p2: point(*p2),
                arc_point: point(*arc_point),
                lines: *lines,
            },
            DimKind::ArcLength {
                center,
                start,
                end,
                arc_point,
                leader,
            } => DimKindDto::ArcLength {
                center: point(*center),
                start: point(*start),
                end: point(*end),
                arc_point: point(*arc_point),
                leader: *leader,
            },
        };
        Self {
            kind,
            text: d.text.to_entry(),
            text_pos: d.text_pos.map(point),
            text_angle: d.text_angle,
            jog_line: d.jog_line.map(point),
            assoc: d.assoc.iter().map(AssocDto::from_assoc).collect(),
            style: styles.index_of(&d.style),
        }
    }

    pub(super) fn to_dimension(&self, styles: &[DimStyle]) -> Result<Dimension, FileError> {
        let pt = |p: &PointDto| from_point(*p);
        let angle = |a: &Option<f64>| match a {
            Some(v) if !v.is_finite() => invalid("a dimension angle is not a finite number"),
            other => Ok(*other),
        };
        let kind = match &self.kind {
            DimKindDto::Linear {
                p1,
                p2,
                line_point,
                rotation,
                oblique,
            } => DimKind::Linear {
                p1: pt(p1)?,
                p2: pt(p2)?,
                line_point: pt(line_point)?,
                rotation: angle(rotation)?,
                oblique: angle(oblique)?,
            },
            DimKindDto::Radial {
                center,
                chord_point,
                diameter,
                leader_length,
                jog,
            } => {
                if !leader_length.is_finite() {
                    return invalid("a dimension leader length is not a finite number");
                }
                DimKind::Radial {
                    center: pt(center)?,
                    chord_point: pt(chord_point)?,
                    diameter: *diameter,
                    leader_length: *leader_length,
                    jog: jog
                        .as_ref()
                        .map(|(shown, at)| Ok::<_, FileError>((pt(shown)?, pt(at)?)))
                        .transpose()?,
                }
            }
            DimKindDto::Ordinate {
                feature,
                leader_end,
                x_datum,
            } => DimKind::Ordinate {
                feature: pt(feature)?,
                leader_end: pt(leader_end)?,
                x_datum: *x_datum,
            },
            DimKindDto::Angular {
                vertex,
                p1,
                p2,
                arc_point,
                lines,
            } => DimKind::Angular {
                vertex: pt(vertex)?,
                p1: pt(p1)?,
                p2: pt(p2)?,
                arc_point: pt(arc_point)?,
                lines: *lines,
            },
            DimKindDto::ArcLength {
                center,
                start,
                end,
                arc_point,
                leader,
            } => DimKind::ArcLength {
                center: pt(center)?,
                start: pt(start)?,
                end: pt(end)?,
                arc_point: pt(arc_point)?,
                leader: *leader,
            },
        };
        let dimension = Dimension {
            kind,
            style: match styles.get(self.style) {
                Some(style) => style.clone(),
                None => return invalid(format!("dimension style {} does not exist", self.style)),
            },
            text: DimText::from_entry(&self.text),
            text_pos: self.text_pos.as_ref().map(pt).transpose()?,
            text_angle: angle(&self.text_angle)?,
            jog_line: self.jog_line.as_ref().map(pt).transpose()?,
            assoc: self
                .assoc
                .iter()
                .map(AssocDto::to_assoc)
                .collect::<Result<_, _>>()?,
        };
        if dimension.measurement().is_none() {
            return invalid("a dimension has nothing to measure");
        }
        Ok(dimension)
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Arrow, DimStyle, Document, Entity, LengthFormat, Point};

    use super::super::NativeFormat;
    use super::*;
    use crate::FileFormat;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn round_trip(document: &Document) -> Document {
        let bytes = NativeFormat.write(document).unwrap().bytes;
        NativeFormat.read(&bytes).unwrap().document
    }

    fn custom() -> DimStyle {
        DimStyle {
            text_height: 3.5,
            arrow: Arrow::Tick,
            post: "<> mm".to_owned(),
            decimal_separator: ',',
            suppress_ext: [true, false],
            length_format: LengthFormat::Architectural,
            ..DimStyle::default()
        }
    }

    #[test]
    fn every_kind_of_dimension_survives_a_round_trip() {
        let style = custom();
        let mut linear = Dimension::linear(p(0.0, 0.0), p(10.0, 3.0), p(0.0, 8.0), 0.5, &style);
        linear.text = DimText::Custom("<> TYP".to_owned());
        linear.text_pos = Some(p(4.0, 9.0));
        linear.text_angle = Some(0.75);
        linear.jog_line = Some(p(2.0, 8.0));
        linear.assoc = vec![
            Assoc {
                point: 0,
                target: EntityId(3),
                at: AssocPoint::Vertex(2),
            },
            Assoc {
                point: 1,
                target: EntityId(4),
                at: AssocPoint::Circumference,
            },
        ];
        let mut aligned = Dimension::aligned(p(1.0, 1.0), p(5.0, 9.0), p(0.0, 0.0), &style);
        if let DimKind::Linear { oblique, .. } = &mut aligned.kind {
            *oblique = Some(1.2);
        }
        aligned.text = DimText::Hidden;
        let all = [
            linear,
            aligned,
            Dimension::radius(p(0.0, 0.0), p(5.0, 0.0), &style),
            Dimension::diameter(p(0.0, 0.0), p(0.0, 5.0), &DimStyle::default()),
            Dimension::jogged(
                p(0.0, 0.0),
                p(5.0, 0.0),
                p(2.0, 3.0),
                p(3.0, 1.5),
                &DimStyle::default(),
            ),
            Dimension::arc_length(p(0.0, 0.0), p(5.0, 0.0), p(0.0, 5.0), p(6.0, 6.0), &style),
            Dimension::ordinate(p(3.0, 4.0), p(3.0, 9.0), true, &style),
            Dimension::angular(
                p(0.0, 0.0),
                p(5.0, 0.0),
                p(0.0, 5.0),
                p(3.0, 3.0),
                true,
                &DimStyle {
                    angle_format: od_core::AngleFormat::Radians,
                    angle_precision: 3,
                    ..DimStyle::default()
                },
            ),
        ];
        let mut document = Document::default();
        for d in &all {
            document.add(Entity::Dimension(Box::new(d.clone())));
        }
        let back = round_trip(&document);
        let got: Vec<Entity> = back.objects().iter().map(|o| o.entity.clone()).collect();
        let want: Vec<Entity> = all
            .into_iter()
            .map(|d| Entity::Dimension(Box::new(d)))
            .collect();
        assert_eq!(got, want);
    }

    #[test]
    fn a_dimension_text_style_survives_and_must_exist() {
        let mut document = Document::default();
        let notes = document.allocate_text_style_id();
        document.push_text_style(od_core::TextStyle::new(notes, "Notes"));
        let style = DimStyle {
            text_style: notes,
            ..DimStyle::default()
        };
        document.add(Entity::Dimension(Box::new(Dimension::aligned(
            p(0.0, 0.0),
            p(1.0, 0.0),
            p(0.0, 1.0),
            &style,
        ))));
        let back = round_trip(&document);
        let Some(Entity::Dimension(d)) = back.objects().first().map(|o| &o.entity) else {
            panic!("dimension expected");
        };
        assert_eq!(d.style.text_style, notes);
        let bytes = NativeFormat.write(&document).unwrap().bytes;
        let text = String::from_utf8(bytes).unwrap();
        let orphan = text.replace("\"text_style\": 1", "\"text_style\": 7");
        assert!(NativeFormat.read(orphan.as_bytes()).is_err());
    }

    #[test]
    fn named_styles_survive_and_the_current_one_is_kept() {
        let mut document = Document::default();
        let metric = DimStyle {
            name: "Metric".to_owned(),
            text_height: 3.5,
            ..DimStyle::default()
        };
        let mut table = document.dim_styles().to_vec();
        table.push(metric.clone());
        document.set_dim_styles(table, metric.clone());
        let back = round_trip(&document);
        assert_eq!(back.dim_styles().len(), 2);
        assert_eq!(back.dim_style_named("metric"), Some(&metric));
        assert_eq!(*back.dim_style(), metric);
        assert!(back.validate().is_ok());
    }

    /// The file holds each style once, in full, and dimensions point at it.
    #[test]
    fn styles_are_written_in_full_and_once() {
        let mut document = Document::default();
        for _ in 0..3 {
            document.add(Entity::Dimension(Box::new(Dimension::aligned(
                p(0.0, 0.0),
                p(1.0, 0.0),
                p(0.0, 1.0),
                &custom(),
            ))));
        }
        let text = String::from_utf8(NativeFormat.write(&document).unwrap().bytes).unwrap();
        let file: serde_json::Value = serde_json::from_str(&text).unwrap();
        let pool = file["drawing"]["dim_styles"].as_array().unwrap();
        assert_eq!(pool.len(), 2, "Standard and the one custom style");
        assert_eq!(pool[0]["text_height"], DimStyle::default().text_height);
        assert!(pool[1].as_object().unwrap().len() > 40, "every value");
        assert_eq!(file["drawing"]["named_dim_styles"], serde_json::json!([0]));
        assert_eq!(file["drawing"]["current_dim_style"], "Standard");
        let used: Vec<_> = file["drawing"]["objects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["entity"]["style"].clone())
            .collect();
        assert_eq!(used, vec![serde_json::json!(1); 3]);
    }

    #[test]
    fn style_settings_are_named_and_a_number_autocad_does_not_define_is_refused() {
        let file = written();
        let style = &file["drawing"]["dim_styles"][0];
        for (key, name) in [
            ("text_above", "above"),
            ("text_just", "centred"),
            ("fit", "best_fit"),
            ("text_move", "move_line"),
            ("arc_symbol", "before"),
            ("tol_mode", "none"),
        ] {
            assert_eq!(style[key], name, "{key}");
        }
        let mut document = Document::default();
        // Styles are brought into range when they are made, so only a field
        // set by hand can be out of it.
        let mut dimension =
            Dimension::aligned(p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0), &DimStyle::default());
        dimension.style.fit = 9;
        document.add(Entity::Dimension(Box::new(dimension)));
        assert!(NativeFormat.write(&document).is_err());
    }

    fn written() -> serde_json::Value {
        let text =
            String::from_utf8(NativeFormat.write(&Document::default()).unwrap().bytes).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn a_damaged_style_table_is_refused() {
        let edits: [(&str, serde_json::Value); 5] = [
            ("named_dim_styles", serde_json::json!([])),
            ("named_dim_styles", serde_json::json!([0, 0])),
            ("named_dim_styles", serde_json::json!([5])),
            ("current_dim_style", serde_json::json!("Nowhere")),
            ("dim_styles", serde_json::json!([{}])),
        ];
        for (key, value) in edits {
            let mut file = written();
            file["drawing"][key] = value.clone();
            assert!(
                NativeFormat.read(file.to_string().as_bytes()).is_err(),
                "{key}: {value}"
            );
        }
        let mut file = written();
        file["drawing"]
            .as_object_mut()
            .unwrap()
            .remove("dim_styles");
        assert!(NativeFormat.read(file.to_string().as_bytes()).is_err());
    }

    #[test]
    fn a_table_that_does_not_start_with_standard_is_refused() {
        let mut document = Document::default();
        let metric = DimStyle {
            name: "Metric".to_owned(),
            ..DimStyle::default()
        };
        let table = vec![DimStyle::default(), metric.clone()];
        document.set_dim_styles(table, metric);
        let text = String::from_utf8(NativeFormat.write(&document).unwrap().bytes).unwrap();
        let mut file: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(NativeFormat.read(file.to_string().as_bytes()).is_ok());
        file["drawing"]["named_dim_styles"] = serde_json::json!([1, 0]);
        assert!(NativeFormat.read(file.to_string().as_bytes()).is_err());
    }

    #[test]
    fn the_current_dimension_style_is_saved_with_the_drawing() {
        let mut document = Document::default();
        document.set_dim_style(custom());
        assert_eq!(*round_trip(&document).dim_style(), custom());
        assert_eq!(
            *round_trip(&Document::default()).dim_style(),
            DimStyle::default()
        );
    }

    #[test]
    fn damaged_dimensions_are_refused() {
        for (key, bad) in [
            ("arrow", serde_json::json!(9)),
            ("length_format", serde_json::json!(0)),
            ("decimal_separator", serde_json::json!("ab")),
            ("text_height", serde_json::json!("tall")),
        ] {
            let mut file = written();
            file["drawing"]["dim_styles"][0][key] = bad.clone();
            assert!(
                NativeFormat.read(file.to_string().as_bytes()).is_err(),
                "{key}: {bad}"
            );
        }
        let nothing = DimensionDto {
            kind: DimKindDto::Radial {
                center: [1.0, 1.0],
                chord_point: [1.0, 1.0],
                diameter: false,
                leader_length: 1.0,
                jog: None,
            },
            text: String::new(),
            text_pos: None,
            text_angle: None,
            jog_line: None,
            assoc: Vec::new(),
            style: 0,
        };
        assert!(
            nothing.to_dimension(&[DimStyle::default()]).is_err(),
            "nothing to measure"
        );
    }
}
