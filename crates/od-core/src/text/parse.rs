//! The lossless contents parser.
//!
//! One pass over the bytes, `O(n)` time, with every loop step consuming at
//! least one byte. Nodes tile the contents exactly: the first starts at 0,
//! each starts where the one before ended, and the last ends at the end.

use std::ops::Range;

use super::fontctl::{font_parts, FontFlags};
use super::numbers::format_of;
use super::paragraph::{self, Paragraph, ParagraphStyle};
use super::{TextError, TextLimits, TextResource};

/// A number from a control, and whether it was written as a multiple (`2x`)
/// of the current value rather than an absolute size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scalar {
    pub value: f64,
    pub relative: bool,
}

/// A character-level control this build understands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format {
    /// `...;` The family and other fields stay in the contents (the
    /// node's source range); the style flags are read.
    Font(FontFlags),
    Height(Scalar),
    Width(Scalar),
    Tracking(Scalar),
    /// Oblique angle in degrees.
    Oblique(f64),
    /// Colour by AutoCAD index, 0..=256.
    Aci(u16),
    /// Colour as 0xRRGGBB.
    TrueColor(u32),
    /// Vertical alignment within a line: 0 bottom, 1 centre, 2 top.
    Align(u8),
    Underline(bool),
    Overline(bool),
    Strike(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpaqueReason {
    /// `\p...;` with something in it no AutoCAD sample has shown yet.
    ParagraphControl,
    /// A real control (`\N`, `\X`) this build does not act on.
    Unsupported,
    UnknownEscape,
    /// A control with no closing `;` within bounds, or a lone backslash.
    Unterminated,
    /// A control whose number is missing, not finite or out of range.
    BadNumber,
    /// A brace beyond the eighth nesting level.
    TooDeep,
    /// A `}` with no `{` to close.
    UnmatchedClose,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    /// Literal text.
    Text,
    /// A literal tab character.
    Tab,
    /// `\\`, `\{` or `\}`: the character after the backslash is literal.
    EscapedChar,
    /// `\P`
    ParagraphBreak,
    /// `\~`
    NonBreakingSpace,
    /// `\U+XXXX` (a surrogate pair of them gives one node).
    Unicode(char),
    /// `{`
    Push,
    /// `}`
    Pop,
    Format(Format),
    /// `\p...;` with only controls AutoCAD output has confirmed: indents and
    /// tab stops. It describes the paragraph it stands in.
    Paragraph(ParagraphStyle),
    /// `\S...;` The payload (numerator, divider, denominator) stays in the contents.
    Stack,
    /// `%<...>%`, kept as written and never evaluated.
    Field,
    /// Kept exactly, never rewritten; layout and editing treat it as a unit.
    Opaque(OpaqueReason),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub kind: NodeKind,
    /// Bytes of the contents this node covers.
    pub source: Range<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticKind {
    Opaque(OpaqueReason),
    /// A `{` never closed.
    UnmatchedOpen,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub range: Range<usize>,
    pub kind: DiagnosticKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedMText {
    pub nodes: Vec<Node>,
    pub paragraphs: Vec<Paragraph>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse(source: &str) -> Result<ParsedMText, TextError> {
    parse_with(source, TextLimits::default())
}

pub fn parse_with(source: &str, limits: TextLimits) -> Result<ParsedMText, TextError> {
    if source.len() > limits.max_content_bytes {
        return Err(TextError::ResourceLimit {
            resource: TextResource::ContentBytes,
            limit: limits.max_content_bytes,
        });
    }
    let mut parser = Parser {
        src: source,
        bytes: source.as_bytes(),
        limits,
        nodes: Vec::new(),
        diagnostics: Vec::new(),
        depth: 0,
        overflow: 0,
    };
    parser.run_all()?;
    let paragraphs = parser.paragraphs()?;
    Ok(ParsedMText {
        nodes: parser.nodes,
        paragraphs,
        diagnostics: parser.diagnostics,
    })
}

struct Parser<'a> {
    src: &'a str,
    bytes: &'a [u8],
    limits: TextLimits,
    nodes: Vec<Node>,
    diagnostics: Vec<Diagnostic>,
    /// Open groups.
    depth: usize,
    /// Braces opened past the depth limit and not yet closed.
    overflow: usize,
}

fn is_special(b: u8) -> bool {
    matches!(b, b'\\' | b'{' | b'}' | b'%' | b'\t')
}

impl Parser<'_> {
    fn run_all(&mut self) -> Result<(), TextError> {
        let len = self.bytes.len();
        let mut i = 0;
        while i < len {
            let end = match self.bytes[i] {
                b'\\' => self.escape(i)?,
                b'{' => self.open(i)?,
                b'}' => self.close(i)?,
                b'%' => self.percent(i)?,
                b'\t' => self.push(NodeKind::Tab, i..i + 1)?,
                _ => self.literal(i)?,
            };
            i = end.max(i + 1);
        }
        if self.depth > 0 {
            self.diagnostics.push(Diagnostic {
                range: len..len,
                kind: DiagnosticKind::UnmatchedOpen,
            });
        }
        Ok(())
    }

    /// Add a node and return where it ends. Adjacent literal text merges.
    fn push(&mut self, kind: NodeKind, source: Range<usize>) -> Result<usize, TextError> {
        let end = source.end;
        if kind == NodeKind::Text {
            if let Some(last) = self.nodes.last_mut() {
                if last.kind == NodeKind::Text && last.source.end == source.start {
                    last.source.end = end;
                    return Ok(end);
                }
            }
        }
        if self.nodes.len() >= self.limits.max_nodes {
            return Err(TextError::ResourceLimit {
                resource: TextResource::Nodes,
                limit: self.limits.max_nodes,
            });
        }
        self.nodes.push(Node { kind, source });
        Ok(end)
    }

    fn opaque(&mut self, reason: OpaqueReason, range: Range<usize>) -> Result<usize, TextError> {
        self.diagnostics.push(Diagnostic {
            range: range.clone(),
            kind: DiagnosticKind::Opaque(reason),
        });
        self.push(NodeKind::Opaque(reason), range)
    }

    /// Text from `start` (whose first byte is taken as literal) up to the
    /// next byte that could begin something else.
    fn literal(&mut self, start: usize) -> Result<usize, TextError> {
        let mut end = start + 1;
        while end < self.bytes.len() && !is_special(self.bytes[end]) {
            end += 1;
        }
        self.push(NodeKind::Text, start..end)
    }

    fn open(&mut self, at: usize) -> Result<usize, TextError> {
        if self.depth < self.limits.max_group_depth {
            self.depth += 1;
            self.push(NodeKind::Push, at..at + 1)
        } else {
            self.overflow += 1;
            self.opaque(OpaqueReason::TooDeep, at..at + 1)
        }
    }

    fn close(&mut self, at: usize) -> Result<usize, TextError> {
        if self.overflow > 0 {
            self.overflow -= 1;
            self.push(NodeKind::Opaque(OpaqueReason::TooDeep), at..at + 1)
        } else if self.depth > 0 {
            self.depth -= 1;
            self.push(NodeKind::Pop, at..at + 1)
        } else {
            self.opaque(OpaqueReason::UnmatchedClose, at..at + 1)
        }
    }

    fn percent(&mut self, at: usize) -> Result<usize, TextError> {
        match self.field_end(at) {
            Some(end) => self.push(NodeKind::Field, at..end),
            None => self.literal(at),
        }
    }

    /// Where the field starting at `at` ends, with nested fields balanced.
    fn field_end(&self, at: usize) -> Option<usize> {
        if !self.bytes[at..].starts_with(b"%<") {
            return None;
        }
        let limit = self.bytes.len().min(at + self.limits.max_control_payload);
        let (mut depth, mut k) = (0usize, at);
        while k + 1 < limit {
            match (self.bytes[k], self.bytes[k + 1]) {
                (b'%', b'<') => {
                    depth += 1;
                    if depth > self.limits.max_group_depth {
                        return None;
                    }
                    k += 2;
                }
                (b'>', b'%') => {
                    depth -= 1;
                    k += 2;
                    if depth == 0 {
                        return Some(k);
                    }
                }
                _ => k += 1,
            }
        }
        None
    }

    fn escape(&mut self, at: usize) -> Result<usize, TextError> {
        let Some(c) = self.src[at + 1..].chars().next() else {
            return self.opaque(OpaqueReason::Unterminated, at..at + 1);
        };
        let after = at + 1 + c.len_utf8();
        let range = at..after;
        match c {
            '\\' | '{' | '}' => self.push(NodeKind::EscapedChar, range),
            'P' => self.push(NodeKind::ParagraphBreak, range),
            '~' => self.push(NodeKind::NonBreakingSpace, range),
            'L' | 'l' => self.push(NodeKind::Format(Format::Underline(c == 'L')), range),
            'O' | 'o' => self.push(NodeKind::Format(Format::Overline(c == 'O')), range),
            'K' | 'k' => self.push(NodeKind::Format(Format::Strike(c == 'K')), range),
            'N' | 'X' => self.opaque(OpaqueReason::Unsupported, range),
            'U' => self.unicode(at),
            'p' | 'f' | 'F' | 'H' | 'W' | 'T' | 'Q' | 'C' | 'c' | 'A' | 'S' => {
                self.with_payload(at, c)
            }
            _ => self.opaque(OpaqueReason::UnknownEscape, range),
        }
    }

    /// `\U+XXXX`, with a UTF-16 surrogate pair of them read as one scalar.
    fn unicode(&mut self, at: usize) -> Result<usize, TextError> {
        let Some(first) = self.hex4(at) else {
            return self.opaque(OpaqueReason::BadNumber, at..(at + 2).min(self.bytes.len()));
        };
        let single = at..at + 7;
        if let Some(c) = char::from_u32(first) {
            return self.push(NodeKind::Unicode(c), single);
        }
        if (0xD800..0xDC00).contains(&first) {
            if let Some(low) = self.hex4(at + 7).filter(|l| (0xDC00..0xE000).contains(l)) {
                let scalar = 0x10000 + ((first - 0xD800) << 10) + (low - 0xDC00);
                if let Some(c) = char::from_u32(scalar) {
                    return self.push(NodeKind::Unicode(c), at..at + 14);
                }
            }
        }
        self.opaque(OpaqueReason::BadNumber, single)
    }

    /// The four hex digits of a `\U+XXXX` that starts at `at`.
    fn hex4(&self, at: usize) -> Option<u32> {
        let digits = self.src.get(at..at + 7)?.strip_prefix("\\U+")?;
        if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        u32::from_str_radix(digits, 16).ok()
    }

    /// A control of the form `\<letter>payload;`.
    fn with_payload(&mut self, at: usize, letter: char) -> Result<usize, TextError> {
        let numeric = matches!(letter, 'H' | 'W' | 'T' | 'Q' | 'C' | 'c' | 'A');
        let cap = if numeric {
            self.limits.max_numeric_payload
        } else {
            self.limits.max_control_payload
        };
        let start = at + 2;
        let Some(semi) = self.semicolon(start, cap) else {
            return self.unterminated(at);
        };
        let range = at..semi + 1;
        let payload = &self.src[start..semi];
        let kind = match letter {
            'p' => match paragraph::decode(payload) {
                Some(style) => Some(NodeKind::Paragraph(style)),
                None => return self.opaque(OpaqueReason::ParagraphControl, range),
            },
            'f' | 'F' => Some(NodeKind::Format(Format::Font(
                font_parts(&self.src[range.clone()]).flags,
            ))),
            'S' => (!payload.is_empty()).then_some(NodeKind::Stack),
            _ => format_of(letter, payload).map(NodeKind::Format),
        };
        match kind {
            Some(kind) => self.push(kind, range),
            None => self.opaque(OpaqueReason::BadNumber, range),
        }
    }

    /// The first `;` at or after `start` that is not escaped, if it lies
    /// within `cap` bytes.
    fn semicolon(&self, start: usize, cap: usize) -> Option<usize> {
        let limit = self.bytes.len().min(start + cap + 1);
        let mut k = start;
        while k < limit {
            match self.bytes[k] {
                b';' => return Some(k),
                b'\\' => k += 2,
                _ => k += 1,
            }
        }
        None
    }

    /// A control with no usable end: keep a bounded stretch after it as one
    /// opaque piece, stopping early where another control could begin.
    fn unterminated(&mut self, at: usize) -> Result<usize, TextError> {
        let limit = self.bytes.len().min(at + self.limits.max_numeric_payload);
        let mut end = at + 2;
        while end < limit && !matches!(self.bytes[end], b'\\' | b'{' | b'}') {
            end += 1;
        }
        while !self.src.is_char_boundary(end) {
            end += 1;
        }
        self.opaque(OpaqueReason::Unterminated, at..end)
    }

    /// The paragraphs: runs of nodes ending at a paragraph break. The last
    /// one, possibly empty, ends the contents.
    fn paragraphs(&self) -> Result<Vec<Paragraph>, TextError> {
        let mut paragraphs = Vec::new();
        let (mut first_node, mut first_byte) = (0, 0);
        let (mut carried, mut unknown) = (ParagraphStyle::default(), false);
        let mut close = |end_node: usize, end_byte: usize, nodes: &[Node]| {
            if paragraphs.len() >= self.limits.max_paragraphs {
                return Err(TextError::ResourceLimit {
                    resource: TextResource::Paragraphs,
                    limit: self.limits.max_paragraphs,
                });
            }
            // A paragraph control holds for its paragraph and every one
            // after it until another says otherwise (AutoCAD writes it only
            // where it changes). One this build cannot decode makes this
            // paragraph and those that inherit from it protected.
            for node in &nodes[first_node..end_node] {
                match &node.kind {
                    NodeKind::Paragraph(style) => (carried, unknown) = (style.clone(), false),
                    NodeKind::Opaque(OpaqueReason::ParagraphControl) => unknown = true,
                    _ => {}
                }
            }
            let (style, protected) = (carried.clone(), unknown);
            paragraphs.push(Paragraph {
                source: first_byte..end_byte,
                nodes: first_node..end_node,
                style,
                protected,
            });
            first_node = end_node;
            first_byte = end_byte;
            Ok(())
        };
        for (i, node) in self.nodes.iter().enumerate() {
            if node.kind == NodeKind::ParagraphBreak {
                close(i + 1, node.source.end, &self.nodes)?;
            }
        }
        close(self.nodes.len(), self.bytes.len(), &self.nodes)?;
        Ok(paragraphs)
    }
}
