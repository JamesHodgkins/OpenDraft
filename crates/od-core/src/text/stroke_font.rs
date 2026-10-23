//! OD Roman: the application's own single-stroke font.
//!
//! A glyph is a few centre-line strokes, not an outline, so the weight of the
//! line is chosen when text is laid out (see `stroke_outline`). The data is
//! `assets/fonts/od_roman.odf`, written by `scripts/od_roman_import.py` from
//! the design sheet `private_material/OD Roman.svg`, and compiled in.
//!
//! Coordinates are thousandths of the cap height, y up, the baseline at 0.
//! An advance never depends on the weight, so changing it never reflows text.

mod parse;

use std::collections::HashMap;
use std::fmt;
use std::sync::OnceLock;

/// A point in font units.
pub type Pt = (f64, f64);

/// One piece of a stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    Line(Pt),
    Cubic(Pt, Pt, Pt),
}

/// A line of the glyph, drawn with a round pen. With no segments it is a
/// dot: a pen laid down once.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    /// Multiplies the pen width, so a crossbar can be finer than a stem.
    pub weight: f64,
    pub start: Pt,
    pub segments: Vec<Segment>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrokeGlyph {
    pub ch: char,
    /// Distance to the next glyph's origin.
    pub advance: f64,
    pub strokes: Vec<Stroke>,
}

/// The measures of the whole font, in font units unless noted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeMetrics {
    pub unit: f64,
    pub cap_height: f64,
    pub x_height: f64,
    /// Below the baseline, positive.
    pub descender: f64,
    pub space: f64,
    /// Pen width as a fraction of cap height when nothing says otherwise.
    pub default_weight: f64,
    /// What bold multiplies the pen width by.
    pub bold_factor: f64,
    /// The slant italic adds, in degrees.
    pub italic_degrees: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrokeFont {
    pub metrics: StrokeMetrics,
    glyphs: Vec<StrokeGlyph>,
    index: HashMap<char, u16>,
}

/// Why font data could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrokeFontError {
    /// 1-based; 0 for the file as a whole.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for StrokeFontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.message)
        } else {
            write!(f, "line {}: {}", self.line, self.message)
        }
    }
}

impl std::error::Error for StrokeFontError {}

impl StrokeFont {
    /// Read font data in the `.odf` text format.
    pub fn parse(text: &str) -> Result<Self, StrokeFontError> {
        parse::parse(text)
    }

    pub(crate) fn from_parts(
        metrics: StrokeMetrics,
        glyphs: Vec<StrokeGlyph>,
    ) -> Result<Self, StrokeFontError> {
        let mut index = HashMap::with_capacity(glyphs.len());
        for (i, g) in glyphs.iter().enumerate() {
            let slot = u16::try_from(i).map_err(|_| StrokeFontError {
                line: 0,
                message: "too many glyphs".to_owned(),
            })?;
            index.insert(g.ch, slot);
        }
        Ok(Self {
            metrics,
            glyphs,
            index,
        })
    }

    /// The glyph slot of `ch`, which is the id a laid-out glyph carries.
    pub fn slot_of(&self, ch: char) -> Option<u16> {
        self.index.get(&ch).copied()
    }

    pub fn glyph(&self, slot: u16) -> Option<&StrokeGlyph> {
        self.glyphs.get(usize::from(slot))
    }

    pub fn glyph_of(&self, ch: char) -> Option<&StrokeGlyph> {
        self.glyph(self.slot_of(ch)?)
    }

    pub fn len(&self) -> usize {
        self.glyphs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }
}

/// The font that ships with the application, or `None` if its bytes were
/// somehow unreadable (a unit test keeps that from happening).
pub fn od_roman() -> Option<&'static StrokeFont> {
    static FONT: OnceLock<Option<StrokeFont>> = OnceLock::new();
    FONT.get_or_init(|| StrokeFont::parse(include_str!("../../assets/fonts/od_roman.odf")).ok())
        .as_ref()
}

#[cfg(test)]
mod tests;
