//! How dimensions and their styles are written to the native file. Only the
//! style values that differ from the stock style are written, so a dimension
//! stays short and a style added later reads as the default.

use od_core::{AngleFormat, Arrow, DimKind, DimStyle, DimText, Dimension, LengthFormat};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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
    },
    ArcLength {
        center: PointDto,
        start: PointDto,
        end: PointDto,
        arc_point: PointDto,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        leader: bool,
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
    /// The style's differences from the stock one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    style: Option<Value>,
}

impl DimensionDto {
    pub(super) fn from_dimension(d: &Dimension) -> Self {
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
            } => DimKindDto::Radial {
                center: point(*center),
                chord_point: point(*chord_point),
                diameter: *diameter,
                leader_length: *leader_length,
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
            style: sparse_style(&d.style),
        }
    }

    pub(super) fn to_dimension(&self) -> Result<Dimension, FileError> {
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
            } => {
                if !leader_length.is_finite() {
                    return invalid("a dimension leader length is not a finite number");
                }
                DimKind::Radial {
                    center: pt(center)?,
                    chord_point: pt(chord_point)?,
                    diameter: *diameter,
                    leader_length: *leader_length,
                }
            }
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
            style: style_from(self.style.as_ref())?,
            text: DimText::from_entry(&self.text),
            text_pos: self.text_pos.as_ref().map(pt).transpose()?,
            text_angle: angle(&self.text_angle)?,
        };
        if dimension.measurement().is_none() {
            return invalid("a dimension has nothing to measure");
        }
        Ok(dimension)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub(super) struct DimStyleDto {
    name: String,
    scale: f64,
    arrow_size: f64,
    /// 0 closed filled, 1 open, 2 tick, 3 none.
    arrow: i64,
    ext_offset: f64,
    ext_extend: f64,
    text_height: f64,
    text_gap: f64,
    centre_mark: f64,
    text_above: u8,
    text_just: u8,
    inside_horizontal: bool,
    outside_horizontal: bool,
    text_inside: bool,
    force_line: bool,
    fit: u8,
    text_move: u8,
    suppress_line: [bool; 2],
    suppress_ext: [bool; 2],
    linear_factor: f64,
    round_to: f64,
    /// `LUNITS` code: 1 scientific ... 5 fractional.
    length_format: i64,
    precision: u8,
    trim_zeros: bool,
    decimal_separator: String,
    post: String,
    arc_symbol: u8,
    /// `AUNITS` code of an angular dimension's angle.
    angle_format: i64,
    angle_precision: u8,
    /// The text style's id.
    text_style: u64,
}

impl Default for DimStyleDto {
    fn default() -> Self {
        Self::from(&DimStyle::default())
    }
}

impl From<&DimStyle> for DimStyleDto {
    fn from(s: &DimStyle) -> Self {
        Self {
            name: s.name.clone(),
            scale: s.scale,
            arrow_size: s.arrow_size,
            arrow: s.arrow.code(),
            ext_offset: s.ext_offset,
            ext_extend: s.ext_extend,
            text_height: s.text_height,
            text_gap: s.text_gap,
            centre_mark: s.centre_mark,
            text_above: s.text_above,
            text_just: s.text_just,
            inside_horizontal: s.inside_horizontal,
            outside_horizontal: s.outside_horizontal,
            text_inside: s.text_inside,
            force_line: s.force_line,
            fit: s.fit,
            text_move: s.text_move,
            suppress_line: s.suppress_line,
            suppress_ext: s.suppress_ext,
            linear_factor: s.linear_factor,
            round_to: s.round_to,
            length_format: s.length_format.code(),
            precision: s.precision,
            trim_zeros: s.trim_zeros,
            decimal_separator: s.decimal_separator.to_string(),
            post: s.post.clone(),
            arc_symbol: s.arc_symbol,
            angle_format: s.angle_format.code(),
            angle_precision: s.angle_precision,
            text_style: s.text_style.0,
        }
    }
}

impl DimStyleDto {
    fn into_style(self) -> Result<DimStyle, FileError> {
        let Some(arrow) = Arrow::from_code(self.arrow) else {
            return invalid(format!("dimension arrow {} does not exist", self.arrow));
        };
        let Some(length_format) = LengthFormat::from_code(self.length_format) else {
            return invalid(format!(
                "dimension length format {} does not exist",
                self.length_format
            ));
        };
        let Some(angle_format) = AngleFormat::from_code(self.angle_format) else {
            return invalid(format!(
                "dimension angle format {} does not exist",
                self.angle_format
            ));
        };
        let numbers = [
            self.scale,
            self.arrow_size,
            self.ext_offset,
            self.ext_extend,
            self.text_height,
            self.text_gap,
            self.centre_mark,
            self.linear_factor,
            self.round_to,
        ];
        if numbers.iter().any(|n| !n.is_finite()) {
            return invalid("a dimension style value is not a finite number");
        }
        let mut separator = self.decimal_separator.chars();
        let decimal_separator = match (separator.next(), separator.next()) {
            (Some(c), None) => c,
            _ => return invalid("a dimension decimal separator is not one character"),
        };
        Ok(DimStyle {
            name: self.name,
            scale: self.scale,
            arrow_size: self.arrow_size,
            arrow,
            ext_offset: self.ext_offset,
            ext_extend: self.ext_extend,
            text_height: self.text_height,
            text_gap: self.text_gap,
            centre_mark: self.centre_mark,
            text_above: self.text_above,
            text_just: self.text_just,
            inside_horizontal: self.inside_horizontal,
            outside_horizontal: self.outside_horizontal,
            text_inside: self.text_inside,
            force_line: self.force_line,
            fit: self.fit,
            text_move: self.text_move,
            suppress_line: self.suppress_line,
            suppress_ext: self.suppress_ext,
            linear_factor: self.linear_factor,
            round_to: self.round_to,
            length_format,
            precision: self.precision,
            trim_zeros: self.trim_zeros,
            decimal_separator,
            post: self.post,
            arc_symbol: self.arc_symbol,
            angle_format,
            angle_precision: self.angle_precision,
            text_style: od_core::TextStyleId(self.text_style),
        }
        .sanitized())
    }
}

fn as_map(style: &DimStyle) -> Map<String, Value> {
    match serde_json::to_value(DimStyleDto::from(style)) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

/// The values of `style` that differ from the stock style, or `None` when
/// none do.
pub(super) fn sparse_style(style: &DimStyle) -> Option<Value> {
    let stock = as_map(&DimStyle::default());
    let changed: Map<String, Value> = as_map(style)
        .into_iter()
        .filter(|(key, value)| stock.get(key) != Some(value))
        .collect();
    (!changed.is_empty()).then_some(Value::Object(changed))
}

/// The named styles, when the drawing has more than the stock `Standard`:
/// each as its differences from the stock style (`{}` for none).
pub(super) fn sparse_table(styles: &[DimStyle]) -> Option<Vec<Value>> {
    if styles == [DimStyle::default()] {
        return None;
    }
    Some(
        styles
            .iter()
            .map(|s| sparse_style(s).unwrap_or_else(|| Value::Object(Map::new())))
            .collect(),
    )
}

/// The table a file describes. `Standard` is put first when the file lacks
/// it, `current` replaces the style of its name (or is added), and a name
/// used twice is refused.
pub(super) fn table_from(
    sparse: Option<&[Value]>,
    current: &DimStyle,
) -> Result<Vec<DimStyle>, FileError> {
    let mut table: Vec<DimStyle> = Vec::new();
    for value in sparse.unwrap_or_default() {
        let style = style_from(Some(value))?;
        if table
            .iter()
            .any(|s| s.name.eq_ignore_ascii_case(&style.name))
            || style.name.trim().is_empty()
        {
            return invalid(format!(
                "dimension style \"{}\" is empty or appears twice",
                style.name
            ));
        }
        table.push(style);
    }
    if !table
        .first()
        .is_some_and(|s| s.name.eq_ignore_ascii_case("Standard"))
    {
        table.retain(|s| !s.name.eq_ignore_ascii_case("Standard"));
        table.insert(0, DimStyle::default());
    }
    match table
        .iter_mut()
        .find(|s| s.name.eq_ignore_ascii_case(&current.name))
    {
        Some(slot) => *slot = current.clone(),
        None => table.push(current.clone()),
    }
    Ok(table)
}

/// The style a file's differences describe.
pub(super) fn style_from(sparse: Option<&Value>) -> Result<DimStyle, FileError> {
    let Some(Value::Object(changed)) = sparse else {
        return match sparse {
            None | Some(Value::Null) => Ok(DimStyle::default()),
            Some(_) => invalid("a dimension style is not an object"),
        };
    };
    let mut all = as_map(&DimStyle::default());
    all.extend(changed.clone());
    match serde_json::from_value::<DimStyleDto>(Value::Object(all)) {
        Ok(dto) => dto.into_style(),
        Err(e) => invalid(format!("a dimension style is damaged: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Document, Entity, Point};

    use super::super::{NativeFormat, CURRENT_VERSION};
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
            Dimension::arc_length(p(0.0, 0.0), p(5.0, 0.0), p(0.0, 5.0), p(6.0, 6.0), &style),
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
    fn named_styles_survive_and_a_file_without_them_still_opens() {
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

        // Only the current style written: the table is built round it.
        let text = String::from_utf8(NativeFormat.write(&document).unwrap().bytes).unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        value.as_object_mut().unwrap().remove("dim_styles");
        let old = NativeFormat.read(value.to_string().as_bytes()).unwrap();
        assert_eq!(old.document.dim_styles().len(), 2);
        assert_eq!(old.document.dim_styles()[0], DimStyle::default());
        assert!(old.document.validate().is_ok());
    }

    #[test]
    fn a_repeated_style_name_is_refused() {
        let styles = [
            serde_json::json!({}),
            serde_json::json!({"name": "A"}),
            serde_json::json!({"name": "a"}),
        ];
        assert!(table_from(Some(&styles), &DimStyle::default()).is_err());
    }

    #[test]
    fn a_default_style_writes_no_style_at_all() {
        let mut document = Document::default();
        document.add(Entity::Dimension(Box::new(Dimension::aligned(
            p(0.0, 0.0),
            p(1.0, 0.0),
            p(0.0, 1.0),
            &DimStyle::default(),
        ))));
        let text = String::from_utf8(NativeFormat.write(&document).unwrap().bytes).unwrap();
        assert!(!text.contains("\"style\""));
        assert!(!text.contains("dim_style"));
        assert!(text.contains(&format!("\"version\": {CURRENT_VERSION}")));
    }

    #[test]
    fn only_the_changed_style_values_are_written() {
        let sparse = sparse_style(&custom()).unwrap();
        let keys: Vec<&String> = sparse.as_object().unwrap().keys().collect();
        assert!(keys.contains(&&"text_height".to_owned()));
        assert!(!keys.contains(&&"scale".to_owned()));
        assert_eq!(style_from(Some(&sparse)).unwrap(), custom());
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
        for bad in [
            serde_json::json!({ "arrow": 9 }),
            serde_json::json!({ "length_format": 0 }),
            serde_json::json!({ "decimal_separator": "ab" }),
            serde_json::json!({ "text_height": "tall" }),
            serde_json::json!(5),
        ] {
            assert!(style_from(Some(&bad)).is_err(), "{bad}");
        }
        let nothing = DimensionDto {
            kind: DimKindDto::Radial {
                center: [1.0, 1.0],
                chord_point: [1.0, 1.0],
                diameter: false,
                leader_length: 1.0,
            },
            text: String::new(),
            text_pos: None,
            text_angle: None,
            style: None,
        };
        assert!(nothing.to_dimension().is_err(), "nothing to measure");
    }
}
