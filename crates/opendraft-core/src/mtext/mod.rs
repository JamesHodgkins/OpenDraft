//! Multiline text (`MTEXT`): one block of formatted text in a reference box.
//!
//! The entity keeps AutoCAD's own fields and, as its authority, the exact
//! format-code stream in `contents`. Everything about how the text looks
//! (glyphs, wrapping, the real extent) is derived from it by the text engine
//! and is never stored. Until that engine exists the entity is drawn and
//! picked as its reference frame, sized by [`MText::frame`].

mod corners;
mod fields;
mod frame;
mod transform;

pub use corners::place_in_corners;
pub use fields::{marker, marker_index};
use std::fmt;

use crate::block::DataField;
use crate::layer::Color;
use crate::text::{self, TextError};
use crate::text_style::TextStyleId;
use crate::Point;

/// Where the insertion point sits on the text's box (DXF group 71).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MTextAttachment {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    MiddleCenter,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl MTextAttachment {
    const ALL: [MTextAttachment; 9] = [
        Self::TopLeft,
        Self::TopCenter,
        Self::TopRight,
        Self::MiddleLeft,
        Self::MiddleCenter,
        Self::MiddleRight,
        Self::BottomLeft,
        Self::BottomCenter,
        Self::BottomRight,
    ];

    /// The DXF code, 1 to 9.
    pub fn code(self) -> u8 {
        Self::ALL.iter().position(|a| *a == self).unwrap_or(0) as u8 + 1
    }

    pub fn from_code(code: i64) -> Option<Self> {
        let index = usize::try_from(code.checked_sub(1)?).ok()?;
        Self::ALL.get(index).copied()
    }

    /// How far along the box the point is: across (0 left, 1 right) and
    /// down (0 top, 1 bottom).
    pub(crate) fn fractions(self) -> (f64, f64) {
        let index = usize::from(self.code() - 1);
        ((index % 3) as f64 / 2.0, (index / 3) as f64 / 2.0)
    }
}

/// The direction text runs (DXF group 72).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MTextFlow {
    LeftToRight,
    TopToBottom,
    ByStyle,
}

impl MTextFlow {
    pub fn code(self) -> u8 {
        match self {
            Self::LeftToRight => 1,
            Self::TopToBottom => 3,
            Self::ByStyle => 5,
        }
    }

    pub fn from_code(code: i64) -> Option<Self> {
        match code {
            1 => Some(Self::LeftToRight),
            3 => Some(Self::TopToBottom),
            5 => Some(Self::ByStyle),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineSpacingStyle {
    AtLeast,
    Exact,
}

impl LineSpacingStyle {
    pub fn code(self) -> u8 {
        match self {
            Self::AtLeast => 1,
            Self::Exact => 2,
        }
    }

    pub fn from_code(code: i64) -> Option<Self> {
        match code {
            1 => Some(Self::AtLeast),
            2 => Some(Self::Exact),
            _ => None,
        }
    }
}

/// The object's default line spacing (DXF groups 73 and 44).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityLineSpacing {
    pub style: LineSpacingStyle,
    /// A multiple of the single line pitch, 0.25 to 4.
    pub factor: f64,
}

impl Default for EntityLineSpacing {
    fn default() -> Self {
        Self {
            style: LineSpacingStyle::AtLeast,
            factor: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundFill {
    /// The colour of the drawing window.
    Window,
    Color(Color),
}

/// The mask painted behind the text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MTextBackground {
    pub fill: BackgroundFill,
    /// How far the mask extends past the text, as a multiple of its height; at least 1.
    pub scale: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColumnKind {
    #[default]
    None,
    Static,
    Dynamic,
}

/// Column settings, kept and checked but not laid out yet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MTextColumns {
    pub kind: ColumnKind,
    pub count: u32,
    pub reversed: bool,
    pub auto_height: bool,
    pub width: f64,
    pub gutter: f64,
    pub heights: Vec<f64>,
}

/// Fields read from a file that mean nothing to this build, kept so saving
/// does not lose them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreservedMTextFields {
    /// DXF group 441, which Autodesk documents as not implemented.
    pub unimplemented_441: Option<i32>,
    /// Bits of group 90 beyond the background fill modes.
    pub background_extra_bits: i32,
}

pub const MAX_COLUMNS: u32 = 100;

#[derive(Clone, Debug, PartialEq)]
pub struct MText {
    pub insertion: Point,
    /// The text's horizontal axis. Any length above `1e-12`; use
    /// [`MText::unit_direction`] for the unit vector.
    pub direction: Point,
    pub nominal_height: f64,
    /// Where lines wrap; 0 turns wrapping off.
    pub reference_width: f64,
    /// How tall the box is when it is taller than its text (the editor's
    /// grip drag, or a box dragged out with two corners); 0 for as tall as
    /// the text. The text is the same box in the drawing and in the editor.
    pub reference_height: f64,
    pub attachment: MTextAttachment,
    pub flow: MTextFlow,
    pub style: TextStyleId,
    /// The format-code stream, exactly as written. See [`text`].
    pub contents: String,
    /// The data fields that markers in `contents` point at; see [`fields`].
    pub fields: Vec<DataField>,
    pub line_spacing: EntityLineSpacing,
    pub background: Option<MTextBackground>,
    pub columns: MTextColumns,
    pub preserved: PreservedMTextFields,
}

/// What is wrong with an `MText`.
#[derive(Clone, Debug, PartialEq)]
pub enum MTextError {
    NotFinite(&'static str),
    OutOfRange(&'static str),
    Contents(TextError),
    Fields(String),
}

impl fmt::Display for MTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFinite(what) => write!(f, "{what} is not a finite number"),
            Self::OutOfRange(what) => write!(f, "{what} is out of range"),
            Self::Contents(e) => e.fmt(f),
            Self::Fields(why) => write!(f, "the data fields are not valid: {why}"),
        }
    }
}

impl std::error::Error for MTextError {}

impl MText {
    /// Left-to-right text of `contents` at `insertion`, top left, with no
    /// wrapping, in the given style.
    pub fn new(
        insertion: Point,
        height: f64,
        contents: impl Into<String>,
        style: TextStyleId,
    ) -> Self {
        Self {
            insertion,
            direction: Point::new(1.0, 0.0),
            nominal_height: height,
            reference_width: 0.0,
            reference_height: 0.0,
            attachment: MTextAttachment::TopLeft,
            flow: MTextFlow::LeftToRight,
            style,
            contents: contents.into(),
            fields: Vec::new(),
            line_spacing: EntityLineSpacing::default(),
            background: None,
            columns: MTextColumns::default(),
            preserved: PreservedMTextFields::default(),
        }
    }

    /// Whether the entity could exist. Checks its own fields; that its style
    /// exists is the document's to check.
    pub fn validate(&self) -> Result<(), MTextError> {
        let finite = |what, v: f64| {
            v.is_finite()
                .then_some(())
                .ok_or(MTextError::NotFinite(what))
        };
        finite("the insertion point", self.insertion.x)?;
        finite("the insertion point", self.insertion.y)?;
        finite("the direction", self.direction.x)?;
        finite("the direction", self.direction.y)?;
        if self.direction.x.hypot(self.direction.y) <= 1e-12 {
            return Err(MTextError::OutOfRange("the direction"));
        }
        finite("the height", self.nominal_height)?;
        if self.nominal_height <= 0.0 {
            return Err(MTextError::OutOfRange("the height"));
        }
        finite("the width", self.reference_width)?;
        if self.reference_width < 0.0 {
            return Err(MTextError::OutOfRange("the width"));
        }
        finite("the box height", self.reference_height)?;
        if self.reference_height < 0.0 {
            return Err(MTextError::OutOfRange("the box height"));
        }
        finite("the line spacing", self.line_spacing.factor)?;
        if !(0.25..=4.0).contains(&self.line_spacing.factor) {
            return Err(MTextError::OutOfRange("the line spacing"));
        }
        if let Some(background) = &self.background {
            finite("the mask scale", background.scale)?;
            if background.scale < 1.0 {
                return Err(MTextError::OutOfRange("the mask scale"));
            }
        }
        self.validate_columns()?;
        text::parse(&self.contents).map_err(MTextError::Contents)?;
        self.fields_as_text()
            .validate()
            .map_err(MTextError::Fields)?;
        Ok(())
    }

    fn validate_columns(&self) -> Result<(), MTextError> {
        let columns = &self.columns;
        if columns.kind == ColumnKind::None {
            return Ok(());
        }
        if !(1..=MAX_COLUMNS).contains(&columns.count) {
            return Err(MTextError::OutOfRange("the column count"));
        }
        let ok = |v: &f64| v.is_finite() && *v >= 0.0;
        let heights_ok =
            columns.heights.len() <= columns.count as usize && columns.heights.iter().all(ok);
        if !(ok(&columns.width) && ok(&columns.gutter) && heights_ok) {
            return Err(MTextError::OutOfRange("a column size"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
