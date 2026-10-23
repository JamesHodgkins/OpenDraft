//! The dimension style as the native file keeps it: only the values that
//! differ from the stock style are written.

use od_core::{AngleFormat, Arrow, DimStyle, LengthFormat};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::entity::invalid;
use crate::FileError;

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct DimStyleDto {
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
    baseline_spacing: f64,
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
            baseline_spacing: s.baseline_spacing,
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
            self.baseline_spacing,
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
            baseline_spacing: self.baseline_spacing,
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
