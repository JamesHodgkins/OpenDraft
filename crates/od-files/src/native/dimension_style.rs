//! Dimension styles as the native file keeps them. Every style is written in
//! full, so a file means the same in every build whatever the stock style
//! becomes. They live in one pool without repeats (`dim_styles`): the named
//! table first, then the copies that dimensions carry. The table
//! (`named_dim_styles`), the current style (`current_dim_style`, by name) and
//! each dimension refer to the pool by position.

use od_core::{DimStyle, Document};
use serde::{Deserialize, Serialize};

use super::entity::invalid;
use super::names::{
    AngleFormatDto, ArcSymbolDto, ArrowDto, FitDto, LengthFormatDto, TextJustifyDto, TextMoveDto,
    TextPlacementDto, ToleranceDto,
};
use crate::FileError;

#[derive(Serialize, Deserialize)]
struct DimStyleDto {
    name: String,
    scale: f64,
    arrow_size: f64,
    arrow: ArrowDto,
    ext_offset: f64,
    ext_extend: f64,
    text_height: f64,
    text_gap: f64,
    centre_mark: f64,
    text_above: TextPlacementDto,
    text_just: TextJustifyDto,
    inside_horizontal: bool,
    outside_horizontal: bool,
    text_inside: bool,
    force_line: bool,
    fit: FitDto,
    text_move: TextMoveDto,
    suppress_line: [bool; 2],
    suppress_ext: [bool; 2],
    linear_factor: f64,
    round_to: f64,
    length_format: LengthFormatDto,
    precision: u8,
    trim_zeros: bool,
    decimal_separator: String,
    post: String,
    arc_symbol: ArcSymbolDto,
    baseline_spacing: f64,
    tol_mode: ToleranceDto,
    tol_plus: f64,
    tol_minus: f64,
    tol_precision: u8,
    tol_scale: f64,
    alt_on: bool,
    alt_factor: f64,
    alt_precision: u8,
    alt_post: String,
    text_frame: bool,
    text_fill: bool,
    /// The second arrowhead, when it differs from the first.
    #[serde(skip_serializing_if = "Option::is_none")]
    arrow2: Option<ArrowDto>,
    /// Fixed extension line length, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    ext_fixed: Option<f64>,
    angle_format: AngleFormatDto,
    angle_precision: u8,
    /// The text style's id.
    text_style: u64,
}

impl TryFrom<&DimStyle> for DimStyleDto {
    type Error = FileError;

    fn try_from(s: &DimStyle) -> Result<Self, FileError> {
        let bad = |what: &str, code: u8| {
            FileError::Invalid(format!(
                "dimension style {} has {what} {code}, which AutoCAD does not define",
                s.name
            ))
        };
        Ok(Self {
            name: s.name.clone(),
            scale: s.scale,
            arrow_size: s.arrow_size,
            arrow: s.arrow.into(),
            ext_offset: s.ext_offset,
            ext_extend: s.ext_extend,
            text_height: s.text_height,
            text_gap: s.text_gap,
            centre_mark: s.centre_mark,
            text_above: TextPlacementDto::from_code(s.text_above)
                .ok_or_else(|| bad(stringify!(text_above), s.text_above))?,
            text_just: TextJustifyDto::from_code(s.text_just)
                .ok_or_else(|| bad(stringify!(text_just), s.text_just))?,
            inside_horizontal: s.inside_horizontal,
            outside_horizontal: s.outside_horizontal,
            text_inside: s.text_inside,
            force_line: s.force_line,
            fit: FitDto::from_code(s.fit).ok_or_else(|| bad(stringify!(fit), s.fit))?,
            text_move: TextMoveDto::from_code(s.text_move)
                .ok_or_else(|| bad(stringify!(text_move), s.text_move))?,
            suppress_line: s.suppress_line,
            suppress_ext: s.suppress_ext,
            linear_factor: s.linear_factor,
            round_to: s.round_to,
            length_format: s.length_format.into(),
            precision: s.precision,
            trim_zeros: s.trim_zeros,
            decimal_separator: s.decimal_separator.to_string(),
            post: s.post.clone(),
            arc_symbol: ArcSymbolDto::from_code(s.arc_symbol)
                .ok_or_else(|| bad(stringify!(arc_symbol), s.arc_symbol))?,
            baseline_spacing: s.baseline_spacing,
            tol_mode: ToleranceDto::from_code(s.tol_mode)
                .ok_or_else(|| bad(stringify!(tol_mode), s.tol_mode))?,
            tol_plus: s.tol_plus,
            tol_minus: s.tol_minus,
            tol_precision: s.tol_precision,
            tol_scale: s.tol_scale,
            alt_on: s.alt_on,
            alt_factor: s.alt_factor,
            alt_precision: s.alt_precision,
            alt_post: s.alt_post.clone(),
            text_frame: s.text_frame,
            text_fill: s.text_fill,
            arrow2: s.arrow2.map(Into::into),
            ext_fixed: s.ext_fixed,
            angle_format: s.angle_format.into(),
            angle_precision: s.angle_precision,
            text_style: s.text_style.0,
        })
    }
}

impl DimStyleDto {
    fn into_style(self) -> Result<DimStyle, FileError> {
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
            self.tol_plus,
            self.tol_minus,
            self.tol_scale,
            self.alt_factor,
            self.ext_fixed.unwrap_or(0.0),
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
            arrow: self.arrow.into(),
            ext_offset: self.ext_offset,
            ext_extend: self.ext_extend,
            text_height: self.text_height,
            text_gap: self.text_gap,
            centre_mark: self.centre_mark,
            text_above: self.text_above.code(),
            text_just: self.text_just.code(),
            inside_horizontal: self.inside_horizontal,
            outside_horizontal: self.outside_horizontal,
            text_inside: self.text_inside,
            force_line: self.force_line,
            fit: self.fit.code(),
            text_move: self.text_move.code(),
            suppress_line: self.suppress_line,
            suppress_ext: self.suppress_ext,
            linear_factor: self.linear_factor,
            round_to: self.round_to,
            length_format: self.length_format.into(),
            precision: self.precision,
            trim_zeros: self.trim_zeros,
            decimal_separator,
            post: self.post,
            arc_symbol: self.arc_symbol.code(),
            baseline_spacing: self.baseline_spacing,
            tol_mode: self.tol_mode.code(),
            tol_plus: self.tol_plus,
            tol_minus: self.tol_minus,
            tol_precision: self.tol_precision,
            tol_scale: self.tol_scale,
            alt_on: self.alt_on,
            alt_factor: self.alt_factor,
            alt_precision: self.alt_precision,
            alt_post: self.alt_post,
            text_frame: self.text_frame,
            text_fill: self.text_fill,
            arrow2: self.arrow2.map(Into::into),
            ext_fixed: self.ext_fixed.filter(|l| *l > 0.0),
            angle_format: self.angle_format.into(),
            angle_precision: self.angle_precision,
            text_style: od_core::TextStyleId(self.text_style),
        }
        .sanitized())
    }
}

/// The write side: the styles met so far, without repeats.
pub(super) struct StylePool {
    styles: Vec<DimStyle>,
}

impl StylePool {
    pub(super) fn new() -> Self {
        Self { styles: Vec::new() }
    }

    /// Where `style` is in the pool, adding it when it is not there yet.
    pub(super) fn index_of(&mut self, style: &DimStyle) -> usize {
        if let Some(at) = self.styles.iter().position(|s| s == style) {
            return at;
        }
        self.styles.push(style.clone());
        self.styles.len() - 1
    }

    pub(super) fn styles(&self) -> &[DimStyle] {
        &self.styles
    }
}

/// The pool, the named table and the current style, as the drawing holds them.
#[derive(Serialize, Deserialize)]
pub(super) struct DimStylesDto {
    dim_styles: Vec<DimStyleDto>,
    /// Pool positions of the named styles, in table order.
    named_dim_styles: Vec<usize>,
    current_dim_style: String,
}

impl DimStylesDto {
    /// The table goes into `pool` first, so it is complete before the
    /// objects add their copies.
    pub(super) fn start(document: &Document, pool: &mut StylePool) -> Vec<usize> {
        document
            .dim_styles()
            .iter()
            .map(|s| pool.index_of(s))
            .collect()
    }

    pub(super) fn of(
        document: &Document,
        pool: &StylePool,
        named: Vec<usize>,
    ) -> Result<Self, FileError> {
        Ok(Self {
            dim_styles: pool
                .styles
                .iter()
                .map(DimStyleDto::try_from)
                .collect::<Result<_, _>>()?,
            named_dim_styles: named,
            current_dim_style: document.dim_style().name.clone(),
        })
    }

    /// The pool, read and checked, ready to be handed to the dimensions and
    /// to [`Styles::install`].
    pub(super) fn read(self) -> Result<Styles, FileError> {
        let pool = self
            .dim_styles
            .into_iter()
            .map(DimStyleDto::into_style)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Styles {
            pool,
            named: self.named_dim_styles,
            current: self.current_dim_style,
        })
    }
}

pub(super) struct Styles {
    pub(super) pool: Vec<DimStyle>,
    named: Vec<usize>,
    current: String,
}

impl Styles {
    /// Sets the table and current style. The table must start with
    /// `Standard`, name every style once (ignoring case) and contain the
    /// current one.
    pub(super) fn install(self, document: &mut Document) -> Result<(), FileError> {
        let mut table: Vec<DimStyle> = Vec::new();
        for at in &self.named {
            let Some(style) = self.pool.get(*at) else {
                return invalid(format!("dimension style {at} does not exist"));
            };
            if style.name.trim().is_empty()
                || table
                    .iter()
                    .any(|s| s.name.eq_ignore_ascii_case(&style.name))
            {
                return invalid(format!(
                    "dimension style \"{}\" is empty or appears twice",
                    style.name
                ));
            }
            table.push(style.clone());
        }
        if !table
            .first()
            .is_some_and(|s| s.name.eq_ignore_ascii_case("Standard"))
        {
            return invalid("the dimension styles do not start with Standard");
        }
        let Some(current) = table
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(&self.current))
            .cloned()
        else {
            return invalid(format!(
                "the current dimension style \"{}\" does not exist",
                self.current
            ));
        };
        document.set_dim_styles(table, current);
        Ok(())
    }
}
