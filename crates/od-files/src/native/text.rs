//! The native format's description of text styles and multiline text.

use std::collections::HashSet;

use od_core::mtext::{
    BackgroundFill, ColumnKind, EntityLineSpacing, MTextBackground, MTextColumns,
    PreservedMTextFields,
};
use od_core::{
    Document, FontDescriptor, MText, SingleLineText, TextAlignment, TextAlignmentMode, TextStyle,
    TextStyleId,
};
use serde::{Deserialize, Serialize};

use super::attribute::{AttributeTextDto, DataFieldDto};
use super::entity::{from_point, invalid, point, PointDto};
use super::names::{AttachmentPointDto, FlowDto, SpacingStyleDto};
use super::ColorDto;
use crate::FileError;

fn east() -> PointDto {
    [1.0, 0.0]
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum FontDto {
    BundledSans,
    TrueType {
        family: String,
        bold: bool,
        italic: bool,
    },
    Shx {
        file_name: String,
    },
    OdRoman {
        bold: bool,
        italic: bool,
    },
    Opaque {
        #[serde(rename = "dxf_font")]
        raw_primary: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "dxf_typeface"
        )]
        raw_typeface: Option<i32>,
    },
}

#[derive(Serialize, Deserialize)]
pub(super) struct TextStyleDto {
    id: u64,
    name: String,
    font: FontDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    big_font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fixed_height: Option<f64>,
    width_factor: f64,
    /// Radians.
    oblique: f64,
    backward: bool,
    upside_down: bool,
    vertical: bool,
    /// Pen width of a stroke font, as a fraction of the text height.
    stroke_weight: f64,
}

impl TextStyleDto {
    pub(super) fn from_style(style: &TextStyle) -> Self {
        let font = match &style.font {
            FontDescriptor::BundledSans => FontDto::BundledSans,
            FontDescriptor::TrueType {
                family,
                bold,
                italic,
            } => FontDto::TrueType {
                family: family.clone(),
                bold: *bold,
                italic: *italic,
            },
            FontDescriptor::Shx { file_name } => FontDto::Shx {
                file_name: file_name.clone(),
            },
            FontDescriptor::OdRoman { bold, italic } => FontDto::OdRoman {
                bold: *bold,
                italic: *italic,
            },
            FontDescriptor::Opaque {
                raw_primary,
                raw_typeface,
            } => FontDto::Opaque {
                raw_primary: raw_primary.clone(),
                raw_typeface: *raw_typeface,
            },
        };
        Self {
            id: style.id.0,
            name: style.name.clone(),
            font,
            big_font: style.big_font.clone(),
            fixed_height: style.fixed_height,
            width_factor: style.width_factor,
            oblique: style.oblique_radians,
            backward: style.backward,
            upside_down: style.upside_down,
            vertical: style.vertical,
            stroke_weight: style.stroke_weight,
        }
    }

    fn to_style(&self) -> Result<TextStyle, FileError> {
        let font = match &self.font {
            FontDto::BundledSans => FontDescriptor::BundledSans,
            FontDto::TrueType {
                family,
                bold,
                italic,
            } => FontDescriptor::TrueType {
                family: family.clone(),
                bold: *bold,
                italic: *italic,
            },
            FontDto::Shx { file_name } => FontDescriptor::Shx {
                file_name: file_name.clone(),
            },
            FontDto::OdRoman { bold, italic } => FontDescriptor::OdRoman {
                bold: *bold,
                italic: *italic,
            },
            FontDto::Opaque {
                raw_primary,
                raw_typeface,
            } => FontDescriptor::Opaque {
                raw_primary: raw_primary.clone(),
                raw_typeface: *raw_typeface,
            },
        };
        let style = TextStyle {
            id: TextStyleId(self.id),
            name: self.name.clone(),
            font,
            big_font: self.big_font.clone(),
            fixed_height: self.fixed_height,
            width_factor: self.width_factor,
            oblique_radians: self.oblique,
            backward: self.backward,
            upside_down: self.upside_down,
            vertical: self.vertical,
            stroke_weight: self.stroke_weight,
        };
        if style.is_valid() {
            Ok(style)
        } else {
            invalid(format!("text style {} is not valid", self.id))
        }
    }
}

/// Load the style table into `document`: it must hold text style 0 and the
/// current style.
pub(super) fn load_styles(
    document: &mut Document,
    styles: &[TextStyleDto],
    current: u64,
    next: u64,
) -> Result<(), FileError> {
    let (mut ids, mut names) = (HashSet::new(), HashSet::new());
    for dto in styles {
        let style = dto.to_style()?;
        if !ids.insert(style.id) {
            return invalid(format!("text style {} appears twice", dto.id));
        }
        if !names.insert(style.name.to_ascii_lowercase()) {
            return invalid(format!("two text styles are called \"{}\"", style.name));
        }
        if style.id == TextStyleId::STANDARD {
            document.replace_text_style(style);
        } else {
            document.push_text_style(style);
        }
    }
    if !ids.contains(&TextStyleId::STANDARD) {
        return invalid("there is no text style 0");
    }
    if !ids.contains(&TextStyleId(current)) {
        return invalid(format!("the current text style {current} does not exist"));
    }
    document.reserve_text_style_id(next);
    document.set_current_text_style(TextStyleId(current));
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FillDto {
    Window,
    Color(ColorDto),
}

#[derive(Serialize, Deserialize)]
struct BackgroundDto {
    fill: FillDto,
    scale: f64,
    /// Bits of the DXF fill setting this build does not know.
    #[serde(default, skip_serializing_if = "is_zero_int", rename = "dxf_fill_bits")]
    extra_bits: i32,
}

fn is_zero_int(v: &i32) -> bool {
    *v == 0
}

#[derive(Serialize, Deserialize, Default)]
struct ColumnsDto {
    kind: String,
    count: u32,
    #[serde(default)]
    reversed: bool,
    #[serde(default)]
    auto_height: bool,
    #[serde(default)]
    width: f64,
    #[serde(default)]
    gutter: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    heights: Vec<f64>,
}

#[derive(Serialize, Deserialize)]
struct SpacingDto {
    style: SpacingStyleDto,
    factor: f64,
}

#[derive(Serialize, Deserialize)]
pub(super) struct MTextDto {
    insertion: PointDto,
    #[serde(default = "east")]
    direction: PointDto,
    height: f64,
    /// Where lines wrap; absent for no wrapping.
    #[serde(default, skip_serializing_if = "is_zero")]
    width: f64,
    /// How tall the box is when taller than its text; absent for as tall as it.
    #[serde(default, skip_serializing_if = "is_zero")]
    box_height: f64,
    attachment: AttachmentPointDto,
    flow: FlowDto,
    #[serde(default)]
    style: u64,
    /// The AutoCAD format-code stream, exactly.
    contents: String,
    /// The data fields the contents' markers point at.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "super::limits::field_items"
    )]
    fields: Vec<DataFieldDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    line_spacing: Option<SpacingDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    background: Option<BackgroundDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    columns: Option<ColumnsDto>,
    /// DXF group 441, kept and never used.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "dxf_group_441"
    )]
    unimplemented_441: Option<i32>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct SingleLineTextDto {
    contents: String,
    placement: AttributeTextDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    alignment: Option<TextAlignmentDto>,
}

#[derive(Serialize, Deserialize)]
struct TextAlignmentDto {
    mode: TextAlignmentModeDto,
    second: PointDto,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TextAlignmentModeDto {
    Align,
    Fit,
}

impl SingleLineTextDto {
    pub(super) fn of(text: &SingleLineText) -> Self {
        Self {
            contents: text.contents.clone(),
            placement: AttributeTextDto::of(&text.placement),
            alignment: text.alignment.map(|a| TextAlignmentDto {
                mode: match a.mode {
                    TextAlignmentMode::Align => TextAlignmentModeDto::Align,
                    TextAlignmentMode::Fit => TextAlignmentModeDto::Fit,
                },
                second: point(a.second),
            }),
        }
    }

    pub(super) fn text(&self) -> Result<SingleLineText, FileError> {
        let alignment = self
            .alignment
            .as_ref()
            .map(|a| -> Result<TextAlignment, FileError> {
                Ok(TextAlignment {
                    mode: match a.mode {
                        TextAlignmentModeDto::Align => TextAlignmentMode::Align,
                        TextAlignmentModeDto::Fit => TextAlignmentMode::Fit,
                    },
                    second: from_point(a.second)?,
                })
            })
            .transpose()?;
        let text = SingleLineText {
            contents: self.contents.clone(),
            placement: self.placement.text()?,
            alignment,
        };
        text.validate()
            .or_else(|why| invalid(format!("single-line text is not valid: {why}")))?;
        Ok(text)
    }
}

impl MTextDto {
    pub(super) fn from_mtext(m: &MText) -> Self {
        let spacing = m.line_spacing;
        let background = m.background.map(|b| BackgroundDto {
            fill: match b.fill {
                BackgroundFill::Window => FillDto::Window,
                BackgroundFill::Color(c) => FillDto::Color(c.into()),
            },
            scale: b.scale,
            extra_bits: m.preserved.background_extra_bits,
        });
        let columns = (m.columns.kind != ColumnKind::None).then(|| ColumnsDto {
            kind: match m.columns.kind {
                ColumnKind::Dynamic => "dynamic",
                _ => "static",
            }
            .to_owned(),
            count: m.columns.count,
            reversed: m.columns.reversed,
            auto_height: m.columns.auto_height,
            width: m.columns.width,
            gutter: m.columns.gutter,
            heights: m.columns.heights.clone(),
        });
        Self {
            insertion: point(m.insertion),
            direction: point(m.direction),
            height: m.nominal_height,
            width: m.reference_width,
            box_height: m.reference_height,
            attachment: m.attachment.into(),
            flow: m.flow.into(),
            style: m.style.0,
            contents: m.contents.clone(),
            fields: m.fields.iter().map(DataFieldDto::of).collect(),
            line_spacing: (spacing != EntityLineSpacing::default()).then(|| SpacingDto {
                style: spacing.style.into(),
                factor: spacing.factor,
            }),
            background,
            columns,
            unimplemented_441: m.preserved.unimplemented_441,
        }
    }

    pub(super) fn to_mtext(&self) -> Result<MText, FileError> {
        let line_spacing = match &self.line_spacing {
            None => EntityLineSpacing::default(),
            Some(s) => EntityLineSpacing {
                style: s.style.into(),
                factor: s.factor,
            },
        };
        let background = self.background.as_ref().map(|b| MTextBackground {
            fill: match &b.fill {
                FillDto::Window => BackgroundFill::Window,
                FillDto::Color(c) => BackgroundFill::Color((*c).into()),
            },
            scale: b.scale,
        });
        let columns = match &self.columns {
            None => MTextColumns::default(),
            Some(c) => MTextColumns {
                kind: match c.kind.as_str() {
                    "static" => ColumnKind::Static,
                    "dynamic" => ColumnKind::Dynamic,
                    other => return invalid(format!("text has a column kind \"{other}\"")),
                },
                count: c.count,
                reversed: c.reversed,
                auto_height: c.auto_height,
                width: c.width,
                gutter: c.gutter,
                heights: c.heights.clone(),
            },
        };
        let text = MText {
            insertion: from_point(self.insertion)?,
            direction: from_point(self.direction)?,
            nominal_height: self.height,
            reference_width: self.width,
            reference_height: self.box_height,
            attachment: self.attachment.into(),
            flow: self.flow.into(),
            style: TextStyleId(self.style),
            contents: self.contents.clone(),
            fields: self.fields.iter().map(DataFieldDto::field).collect(),
            line_spacing,
            background,
            columns,
            preserved: PreservedMTextFields {
                unimplemented_441: self.unimplemented_441,
                background_extra_bits: self.background.as_ref().map_or(0, |b| b.extra_bits),
            },
        };
        match text.validate() {
            Ok(()) => Ok(text),
            Err(e) => invalid(format!("text is not valid: {e}")),
        }
    }
}
