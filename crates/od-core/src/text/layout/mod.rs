//! Laying out multiline text: shaping, wrapping, tabs and line spacing into
//! positioned glyphs.
//!
//! One result drives everything that needs to know where text is: bounds,
//! hit testing, painting and plotting. Coordinates are local to the text
//! block: the origin is its top-left corner before the attachment is
//! applied, x runs along the text, y runs up (so lines sit at negative y).
//!
//! What it does not do yet: paragraph formatting (those controls are
//! opaque), stacked fractions, fields, installed or per-run fonts, columns,
//! vertical flow and mixed-direction (bidi) lines. Each of those is
//! reported as a [`LayoutIssue`] rather than dropped without a word.

mod items;
mod lines;

use std::ops::Range;

use super::font::{self, FontFace, Metrics};
use super::parse::OpaqueReason;
use super::{parse, TextError};
use crate::mtext::{LineSpacingStyle, MText, MTextFlow};
use crate::text_style::{FontDescriptor, TextStyle};

/// Where a run of text takes its colour from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextColor {
    /// The object's own colour.
    Entity,
    /// An AutoCAD colour index, 1 to 255.
    Aci(u16),
    /// 0xRRGGBB.
    Rgb(u32),
}

/// One glyph placed in the block. A point `(fx, fy)` of its outline, in font
/// units, lands at [`PositionedGlyph::place`].
#[derive(Clone, Debug, PartialEq)]
pub struct PositionedGlyph {
    pub id: u16,
    /// The face the glyph id belongs to.
    pub face: FontFace,
    pub x: f64,
    pub y: f64,
    /// Drawing units per font unit, across and up.
    pub scale_x: f64,
    pub scale_y: f64,
    /// How far across a point moves per unit up (the tangent of the slant).
    pub shear: f64,
    /// Pen width in thousandths of the cap height, for a stroke face; 0 for
    /// an outline face.
    pub weight: u16,
    pub color: TextColor,
    /// The bytes of the contents it came from.
    pub source: Range<usize>,
}

impl PositionedGlyph {
    pub fn place(&self, fx: f64, fy: f64) -> (f64, f64) {
        (
            self.x + (fx + fy * self.shear) * self.scale_x,
            self.y + fy * self.scale_y,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecorationKind {
    Underline,
    Overline,
    Strike,
    /// The bar of a stacked fraction.
    FractionBar,
}

/// A line drawn along text: `y` is its middle, `thickness` its width.
#[derive(Clone, Debug, PartialEq)]
pub struct Decoration {
    pub kind: DecorationKind,
    pub x0: f64,
    pub x1: f64,
    pub y: f64,
    pub thickness: f64,
    pub color: TextColor,
}

/// A place the caret can sit: between two clusters, with the bytes of the
/// contents it stands at.
#[derive(Clone, Debug, PartialEq)]
pub struct CaretStop {
    pub x: f64,
    /// Index into [`TextLayout::lines`].
    pub line: usize,
    /// Byte offset in the contents. A wrapped line's end and the next line's
    /// start share one.
    pub source: usize,
    /// Where the cluster after it ends, so `source..end` is what a delete
    /// there removes. Equal to `source` at the end of a line.
    pub end: usize,
    /// The last character before it on the line, and the first after.
    pub prev: Option<char>,
    pub next: Option<char>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LaidOutLine {
    pub baseline: f64,
    /// The tallest text on the line.
    pub height: f64,
    /// Its caret stops, left to right, in [`TextLayout::carets`].
    pub carets: Range<usize>,
    /// Where the visible text ends (trailing spaces excluded).
    pub width: f64,
    pub glyphs: Range<usize>,
    pub source: Range<usize>,
}

/// Something laid out differently from how it was written.
#[derive(Clone, Debug, PartialEq)]
pub enum LayoutIssue {
    /// A control this build keeps but does not act on.
    Skipped(OpaqueReason),
    /// `%<...>%` is not evaluated, so nothing is drawn for it.
    FieldNotEvaluated,
    /// The requested font is not available; the bundled one is used.
    FontSubstituted,
    /// The font has no glyph for a character.
    MissingGlyph,
    /// Vertical flow is laid out left to right.
    FlowNotSupported,
    /// Columns are laid out as one.
    ColumnsIgnored,
    /// The contents could not be read, so nothing is drawn.
    Unreadable(TextError),
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutDiagnostic {
    pub range: Range<usize>,
    pub issue: LayoutIssue,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextLayout {
    pub glyphs: Vec<PositionedGlyph>,
    pub lines: Vec<LaidOutLine>,
    pub carets: Vec<CaretStop>,
    pub decorations: Vec<Decoration>,
    /// The widest line, and the height from the top of the first line to the
    /// bottom of the last.
    pub actual_width: f64,
    pub actual_height: f64,
    /// The box the text sits in: the reference width when wrapping, else the
    /// text's own width; as tall as the text.
    pub frame_width: f64,
    pub frame_height: f64,
    pub diagnostics: Vec<LayoutDiagnostic>,
}

/// The distance between baselines as a multiple of the text height.
pub(super) const LINE_PITCH: f64 = 5.0 / 3.0;
/// A tab runs to the next multiple of this many text heights. A stand-in:
/// AutoCAD's default interval has to be measured from its own output.
pub(super) const DEFAULT_TAB: f64 = 4.0;
/// The width of an empty text's frame, in text heights.
const EMPTY_WIDTH: f64 = 0.6;

/// What layout reads from the entity and style.
pub(super) struct Params {
    pub height: f64,
    pub wrap: Option<f64>,
    pub spacing_style: LineSpacingStyle,
    pub spacing_factor: f64,
    pub width_factor: f64,
    pub oblique: f64,
    /// The style's own bold and italic.
    pub bold: bool,
    pub italic: bool,
    /// The style asks for OD Roman, and its pen width as a fraction of height.
    pub stroke: bool,
    pub weight: f64,
}

pub(super) struct Context {
    pub metrics: Metrics,
    pub params: Params,
}

/// Lay `text` out in `style`.
pub fn layout_text(text: &MText, style: &TextStyle) -> TextLayout {
    let mut layout = TextLayout::default();
    let Some(face) = font::face() else {
        return layout;
    };
    let stroke_font = match style.font {
        FontDescriptor::OdRoman { .. } => super::stroke_font::od_roman(),
        _ => None,
    };
    let parsed = match parse(&text.contents) {
        Ok(p) => p,
        Err(e) => {
            layout.diagnostics.push(LayoutDiagnostic {
                range: 0..0,
                issue: LayoutIssue::Unreadable(e),
            });
            return layout;
        }
    };
    let bundled = match &style.font {
        FontDescriptor::BundledSans => true,
        FontDescriptor::OdRoman { .. } => stroke_font.is_some(),
        FontDescriptor::TrueType { family, .. } => family.eq_ignore_ascii_case("DejaVu Sans"),
        _ => false,
    };
    if !bundled {
        layout.diagnostics.push(LayoutDiagnostic {
            range: 0..0,
            issue: LayoutIssue::FontSubstituted,
        });
    }
    if text.flow != MTextFlow::LeftToRight || style.vertical {
        layout.diagnostics.push(LayoutDiagnostic {
            range: 0..0,
            issue: LayoutIssue::FlowNotSupported,
        });
    }
    if text.columns.kind != crate::mtext::ColumnKind::None {
        layout.diagnostics.push(LayoutDiagnostic {
            range: 0..0,
            issue: LayoutIssue::ColumnsIgnored,
        });
    }
    let cx = Context {
        metrics: stroke_font.map_or_else(
            || font::metrics(face),
            |f| font::Metrics {
                cap_height: f.metrics.cap_height,
                descent: f.metrics.descender,
            },
        ),
        params: Params {
            height: text.nominal_height,
            wrap: (text.reference_width > 0.0).then_some(text.reference_width),
            spacing_style: text.line_spacing.style,
            spacing_factor: text.line_spacing.factor,
            width_factor: style.width_factor,
            oblique: style.oblique_radians,
            bold: matches!(
                style.font,
                FontDescriptor::TrueType { bold: true, .. }
                    | FontDescriptor::OdRoman { bold: true, .. }
            ),
            italic: matches!(
                style.font,
                FontDescriptor::TrueType { italic: true, .. }
                    | FontDescriptor::OdRoman { italic: true, .. }
            ),
            stroke: stroke_font.is_some(),
            weight: style.stroke_weight,
        },
    };
    let built = items::build(&parsed, &text.contents, &cx);
    layout.diagnostics.extend(built.diagnostics);
    lines::place(
        &built.elems,
        &built.states,
        &cx,
        text.contents.len(),
        &mut layout,
    );
    layout.frame_width = match cx.params.wrap {
        Some(width) => width,
        None => layout.actual_width.max(EMPTY_WIDTH * cx.params.height),
    };
    layout.frame_height = layout.actual_height;
    layout
}

#[cfg(test)]
mod alignment_tests;
#[cfg(test)]
mod face_tests;
#[cfg(test)]
mod paragraph_tests;
#[cfg(test)]
mod stroke_tests;
#[cfg(test)]
mod tests;
