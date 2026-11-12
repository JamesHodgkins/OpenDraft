//! Character formatting in force at a point of the contents, and how each
//! control changes it. Shared by layout (which draws with it) and the
//! formatting queries (which show it).

use super::layout::TextColor;
use super::parse::{Format, Scalar};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CharState {
    pub height: f64,
    /// A multiple of the style's width factor.
    pub width: f64,
    /// A multiple of the normal advance.
    pub tracking: f64,
    /// Slant in degrees, added to the style's.
    pub oblique: f64,
    pub underline: bool,
    pub overline: bool,
    pub strike: bool,
    pub bold: bool,
    pub italic: bool,
    /// Drawn in OD Roman (the style's font, or a `` control naming it).
    pub stroke: bool,
    /// A pen width the text sets for itself, in thousandths of the cap
    /// height; `None` is the style's.
    pub weight: Option<u16>,
    pub color: TextColor,
}

fn apply(value: Scalar, current: f64) -> f64 {
    if value.relative {
        current * value.value
    } else {
        value.value
    }
}

/// How far one step of tracking moves each character on, as a share of the
/// text height: AutoCAD adds `(tracking - 1)` of this after every character
/// (letters and spaces alike, so wide and narrow ones are spaced evenly),
/// where 1.0 is normal. Measured from AutoCAD's own output for Arial: 0.44 of
/// the font size, which is 0.6145 of the cap height.
pub const TRACKING_PER_HEIGHT: f64 = 0.6145;

impl CharState {
    /// The extra space after each character that tracking makes (negative
    /// below 1.0), in drawing units.
    pub fn tracking_extra(&self) -> f64 {
        (self.tracking - 1.0) * TRACKING_PER_HEIGHT * self.height
    }

    /// Plain text of `height`.
    pub fn plain(height: f64) -> Self {
        Self {
            height,
            width: 1.0,
            tracking: 1.0,
            oblique: 0.0,
            underline: false,
            overline: false,
            strike: false,
            bold: false,
            italic: false,
            stroke: false,
            weight: None,
            color: TextColor::Entity,
        }
    }

    /// The state after `format`, or `None` if it changes nothing that is
    /// laid out (a font, an alignment).
    pub fn after(&self, format: Format) -> Option<CharState> {
        let mut s = *self;
        match format {
            Format::Height(v) => s.height = apply(v, s.height),
            Format::Width(v) => s.width = apply(v, s.width),
            Format::Tracking(v) => s.tracking = apply(v, s.tracking),
            Format::Oblique(degrees) => s.oblique = degrees,
            Format::Aci(n) => {
                s.color = match n {
                    1..=255 => TextColor::Aci(n),
                    _ => TextColor::Entity,
                }
            }
            Format::TrueColor(rgb) => s.color = TextColor::Rgb(rgb),
            Format::Underline(on) => s.underline = on,
            Format::Overline(on) => s.overline = on,
            Format::Strike(on) => s.strike = on,
            Format::Font(flags) => {
                (s.bold, s.italic) = (flags.bold, flags.italic);
                s.weight = flags.weight;
            }
            Format::Align(_) => return None,
        }
        Some(s)
    }
}
