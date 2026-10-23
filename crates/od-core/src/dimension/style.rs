//! The dimension style: the `DIM*` variables that decide how a dimension
//! looks. A dimension keeps a copy of the style it was made with, so changing
//! the current style never moves an existing dimension.

use crate::text_style::TextStyleId;
use crate::units::{LengthFormat, Units};

/// What the ends of a dimension line look like.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Arrow {
    /// A filled triangle (`DIMBLK` closed filled).
    #[default]
    Closed,
    /// Two lines forming a V.
    Open,
    /// A short oblique stroke (`DIMTSZ > 0`).
    Tick,
    None,
}

impl Arrow {
    pub const ALL: [Arrow; 4] = [Arrow::Closed, Arrow::Open, Arrow::Tick, Arrow::None];

    pub fn code(self) -> i64 {
        self as i64
    }

    pub fn from_code(code: i64) -> Option<Self> {
        usize::try_from(code)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
    }

    pub fn label(self) -> &'static str {
        match self {
            Arrow::Closed => "Closed filled",
            Arrow::Open => "Open",
            Arrow::Tick => "Architectural tick",
            Arrow::None => "None",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DimStyle {
    pub name: String,
    /// `DIMSCALE`: multiplies every size below; never the measured value.
    pub scale: f64,
    /// `DIMASZ`: arrow length.
    pub arrow_size: f64,
    pub arrow: Arrow,
    /// `DIMEXO`: gap between the measured point and its extension line.
    pub ext_offset: f64,
    /// `DIMEXE`: extension line past the dimension line.
    pub ext_extend: f64,
    /// `DIMTXT`: text height.
    pub text_height: f64,
    /// `DIMGAP`: clearance around the text.
    pub text_gap: f64,
    /// `DIMCEN`: centre mark size for radial dimensions (0 for none).
    pub centre_mark: f64,
    /// `DIMTAD`: 0 centred, 1 above, 2 outside, 3 JIS, 4 below.
    pub text_above: u8,
    /// `DIMJUST`: 0 centred, 1 at extension line 1, 2 at extension line 2.
    pub text_just: u8,
    /// `DIMTIH`: text inside the extension lines stays horizontal.
    pub inside_horizontal: bool,
    /// `DIMTOH`: text outside the extension lines stays horizontal.
    pub outside_horizontal: bool,
    /// `DIMTIX`: always put the text between the extension lines.
    pub text_inside: bool,
    /// `DIMTOFL`: draw a dimension line between the extension lines even
    /// when the arrows and text are outside.
    pub force_line: bool,
    /// `DIMATFIT`: what moves outside first when it does not fit
    /// (0 both, 1 arrows, 2 text, 3 best fit).
    pub fit: u8,
    /// `DIMTMOVE`: 0 the dimension line follows moved text, 1 a leader joins
    /// it, 2 it floats free.
    pub text_move: u8,
    /// `DIMSD1`, `DIMSD2`, `DIMSE1`, `DIMSE2`.
    pub suppress_line: [bool; 2],
    pub suppress_ext: [bool; 2],
    /// `DIMLFAC`: scales the measured value for display.
    pub linear_factor: f64,
    /// `DIMRND`: round the displayed value to a multiple (0 for none).
    pub round_to: f64,
    /// `DIMLUNIT`.
    pub length_format: LengthFormat,
    /// `DIMDEC`, or the fraction step for fractional formats.
    pub precision: u8,
    /// `DIMZIN`: drop trailing zeros of decimals.
    pub trim_zeros: bool,
    /// `DIMDSEP`.
    pub decimal_separator: char,
    /// `DIMPOST`: text around the value, `<>` standing for it ("<> mm").
    pub post: String,
    /// `DIMARCSYM`: 0 symbol before the text, 1 above it, 2 none.
    pub arc_symbol: u8,
    /// `DIMTXSTY`: the text style the words are written in. Sizes are
    /// worked out in the Standard style; the style only shapes the glyphs
    /// when they are drawn or plotted.
    pub text_style: TextStyleId,
}

impl Default for DimStyle {
    /// ISO-25: millimetres, text 2.5 high.
    fn default() -> Self {
        Self {
            name: "Standard".to_owned(),
            scale: 1.0,
            arrow_size: 2.5,
            arrow: Arrow::Closed,
            ext_offset: 0.625,
            ext_extend: 1.25,
            text_height: 2.5,
            text_gap: 0.625,
            centre_mark: 2.5,
            text_above: 1,
            text_just: 0,
            inside_horizontal: false,
            outside_horizontal: true,
            text_inside: false,
            force_line: false,
            fit: 3,
            text_move: 0,
            suppress_line: [false; 2],
            suppress_ext: [false; 2],
            linear_factor: 1.0,
            round_to: 0.0,
            length_format: LengthFormat::Decimal,
            precision: 2,
            trim_zeros: false,
            decimal_separator: '.',
            post: String::new(),
            arc_symbol: 0,
            text_style: TextStyleId::STANDARD,
        }
    }
}

impl DimStyle {
    /// The style with every value in range: sizes finite and not negative,
    /// codes within their choices, a usable scale.
    pub fn sanitized(mut self) -> Self {
        let size = |v: f64, fallback: f64| {
            if v.is_finite() && v >= 0.0 {
                v
            } else {
                fallback
            }
        };
        let d = Self::default();
        self.scale = if self.scale.is_finite() && self.scale > 0.0 {
            self.scale
        } else {
            1.0
        };
        self.arrow_size = size(self.arrow_size, d.arrow_size);
        self.ext_offset = size(self.ext_offset, d.ext_offset);
        self.ext_extend = size(self.ext_extend, d.ext_extend);
        self.text_height = if self.text_height.is_finite() && self.text_height > 0.0 {
            self.text_height
        } else {
            d.text_height
        };
        self.text_gap = size(self.text_gap, d.text_gap);
        self.centre_mark = size(self.centre_mark, d.centre_mark);
        self.round_to = size(self.round_to, 0.0);
        self.linear_factor = if self.linear_factor.is_finite() && self.linear_factor != 0.0 {
            self.linear_factor
        } else {
            1.0
        };
        self.text_above = self.text_above.min(4);
        self.text_just = self.text_just.min(2);
        self.fit = self.fit.min(3);
        self.text_move = self.text_move.min(2);
        self.precision = self.precision.min(crate::units::MAX_PRECISION);
        self.arc_symbol = self.arc_symbol.min(2);
        self
    }

    /// The same style with `scale` folded into the sizes, for drawing.
    pub(crate) fn resolved(&self) -> DimStyle {
        let s = self.scale;
        DimStyle {
            scale: 1.0,
            arrow_size: self.arrow_size * s,
            ext_offset: self.ext_offset * s,
            ext_extend: self.ext_extend * s,
            text_height: self.text_height * s,
            text_gap: self.text_gap * s,
            centre_mark: self.centre_mark * s,
            ..self.clone()
        }
    }

    /// The drawing units used to write a measured length.
    pub(crate) fn units(&self) -> Units {
        Units {
            length: self.length_format,
            length_precision: self.precision,
            ..Units::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizing_repairs_bad_values() {
        let style = DimStyle {
            scale: -1.0,
            text_height: f64::NAN,
            text_above: 9,
            precision: 200,
            linear_factor: 0.0,
            ..DimStyle::default()
        }
        .sanitized();
        assert_eq!(style.scale, 1.0);
        assert_eq!(style.text_height, 2.5);
        assert_eq!(style.text_above, 4);
        assert_eq!(style.precision, 8);
        assert_eq!(style.linear_factor, 1.0);
    }

    #[test]
    fn scale_multiplies_sizes_only() {
        let style = DimStyle {
            scale: 2.0,
            linear_factor: 3.0,
            ..DimStyle::default()
        };
        let r = style.resolved();
        assert_eq!(r.arrow_size, 5.0);
        assert_eq!(r.text_height, 5.0);
        assert_eq!(r.linear_factor, 3.0);
    }

    #[test]
    fn arrow_codes_round_trip() {
        for a in Arrow::ALL {
            assert_eq!(Arrow::from_code(a.code()), Some(a));
        }
        assert_eq!(Arrow::from_code(9), None);
    }
}
