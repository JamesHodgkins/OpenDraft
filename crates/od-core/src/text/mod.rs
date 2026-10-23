//! Multiline text contents: the AutoCAD format-code stream that `MText`
//! keeps as its authority, and the lossless parser that reads it.
//!
//! The parser never rewrites or normalises anything. Every byte of the
//! contents belongs to exactly one [`Node`], in order, so a reader can edit
//! the nodes it understands and leave the rest alone. A control it does not
//! understand, or that is malformed, becomes an opaque node with a reason.

mod caret;
mod charstate;
mod edit;
mod error;
mod font;
mod fontctl;
mod format;
mod layout;
mod limits;
mod list;
mod memo;
mod numbers;
mod paragraph;
mod paragraph_edit;
mod parse;
mod stack;
mod stroke_font;
mod stroke_outline;

pub use caret::SelectionRect;
pub use edit::{escape_for_contents, plain_text_of, replace_text, EditRefusal, Edited};
pub use error::{TextError, TextResource};
pub use font::{glyph_contours, glyph_outline, Contour, FontFace};
pub use fontctl::{font_control, font_parts, FontFlags, FontParts};
pub use format::{apply_char_edit, format_in, CharEdit, Formatted, Mixed, SelectionFormat};
pub use layout::{
    layout_text, CaretStop, Decoration, DecorationKind, LaidOutLine, LayoutDiagnostic, LayoutIssue,
    PositionedGlyph, TextColor, TextLayout,
};
pub use limits::TextLimits;
pub use list::{apply_list, break_list_item, list_kind_in, ListKind};
pub(crate) use memo::layout as memoized_layout;
pub use paragraph::{
    encode as paragraph_control, ListSource, NumberingKind, Paragraph, ParagraphAlignment,
    ParagraphStyle, TabKind, TabStop,
};
pub use paragraph_edit::{apply_paragraph_edit, paragraph_view_in, ParagraphEdit, ParagraphView};
pub use parse::{
    parse, parse_with, Diagnostic, DiagnosticKind, Format, Node, NodeKind, OpaqueReason,
    ParsedMText, Scalar,
};
pub use stack::{split_stack, toggle_stack, StackParts};
pub use stroke_font::{
    od_roman, Segment, Stroke, StrokeFont, StrokeFontError, StrokeGlyph, StrokeMetrics,
};
pub use stroke_outline::{stroke_contours, MAX_WEIGHT, MIN_WEIGHT};

impl ParsedMText {
    /// The text a reader sees, with every formatting control dropped:
    /// paragraph breaks become newlines, a non-breaking space a space, and a
    /// stack its two halves joined by its divider. Fields and opaque controls
    /// contribute nothing.
    pub fn plain_text(&self, source: &str) -> String {
        let mut out = String::new();
        for node in &self.nodes {
            let Some(text) = source.get(node.source.clone()) else {
                continue;
            };
            match &node.kind {
                NodeKind::Text | NodeKind::Tab => out.push_str(text),
                NodeKind::EscapedChar => out.extend(text.chars().nth(1)),
                NodeKind::ParagraphBreak => out.push('\n'),
                NodeKind::NonBreakingSpace => out.push(' '),
                NodeKind::Unicode(c) => out.push(*c),
                NodeKind::Stack => {
                    let inner = text.get(2..text.len().saturating_sub(1)).unwrap_or("");
                    out.push_str(inner);
                }
                _ => {}
            }
        }
        out
    }
}

#[cfg(test)]
mod caret_tests;
#[cfg(test)]
mod tests;
