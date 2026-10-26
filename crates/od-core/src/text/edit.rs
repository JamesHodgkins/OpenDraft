//! Editing contents as plain text.
//!
//! An editor works in byte offsets of the contents (the offsets caret stops
//! carry) and in plain text. This turns the plain text typed into contents
//! (escaping what would otherwise be read as a control) and replaces a range,
//! refusing the edits that would damage something it cannot see: a control it
//! keeps but does not understand, or half of a formatting group.

use std::fmt;
use std::ops::Range;

use super::limits::TextLimits;
use super::parse::{parse_with, NodeKind, ParsedMText};

/// Why a range could not be replaced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditRefusal {
    /// The range is not inside the contents, or an end of it falls inside a
    /// control.
    BadRange,
    /// It covers a control that is kept but not understood.
    Protected,
    /// It covers one end of a `{ }` group but not the other.
    SplitsGroup,
    /// The result would be bigger than the limits allow.
    TooLarge,
    /// A formatting value is not a usable number.
    BadValue,
    /// The change would nest groups deeper than eight levels.
    TooDeep,
    /// There is no `/`, `^` or `#` in the selection to stack at.
    NoDivider,
    /// The selection holds something that cannot go in a stack.
    CannotStack,
}

impl fmt::Display for EditRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BadRange => "that part of the text cannot be edited",
            Self::Protected => "the selection holds formatting that cannot be edited yet",
            Self::SplitsGroup => "the selection splits a formatting group",
            Self::TooLarge => "the text would be too long",
            Self::BadValue => "that is not a usable value",
            Self::TooDeep => "the formatting is nested too deeply",
            Self::NoDivider => "select text such as 1/2, 1^2 or 1#2 to stack",
            Self::CannotStack => "that text cannot be stacked",
        })
    }
}

impl std::error::Error for EditRefusal {}

/// The result of an edit: the new contents and where the caret goes (the
/// byte after what was inserted).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edited {
    pub contents: String,
    pub caret: usize,
}

/// Plain text as contents: `\`, `{` and `}` are escaped, a line break
/// becomes a paragraph break and a carriage return is dropped.
pub fn escape_for_contents(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '\r' => {
                if chars.peek() != Some(&'\n') {
                    out.push_str("\\P");
                }
            }
            '\n' => out.push_str("\\P"),
            _ => out.push(c),
        }
    }
    out
}

/// Check that `range` can be cut out of text parsed as `parsed`.
pub(super) fn check(
    parsed: &ParsedMText,
    contents: &str,
    range: &Range<usize>,
) -> Result<(), EditRefusal> {
    if range.start > range.end
        || range.end > contents.len()
        || !contents.is_char_boundary(range.start)
        || !contents.is_char_boundary(range.end)
    {
        return Err(EditRefusal::BadRange);
    }
    let mut depth: i64 = 0;
    for node in &parsed.nodes {
        let n = &node.source;
        let inside = n.start >= range.start && n.end <= range.end;
        // An end of the range strictly inside anything but plain text would
        // cut a control in two.
        let cut = (n.start < range.start && range.start < n.end)
            || (n.start < range.end && range.end < n.end);
        if cut && node.kind != NodeKind::Text {
            return Err(EditRefusal::BadRange);
        }
        if inside && range.start != range.end {
            match node.kind {
                NodeKind::Opaque(_) => return Err(EditRefusal::Protected),
                NodeKind::Push => depth += 1,
                NodeKind::Pop => depth -= 1,
                _ => {}
            }
        }
    }
    if depth != 0 {
        return Err(EditRefusal::SplitsGroup);
    }
    Ok(())
}

/// The `{`, the controls that open it, or the `}` of every group in `range`
/// whose other end is outside it: what a selection that cuts a group would
/// have to take with it. In order.
pub(super) fn dangling(parsed: &ParsedMText, range: &Range<usize>) -> Vec<Range<usize>> {
    let inside: Vec<&super::parse::Node> = parsed
        .nodes
        .iter()
        .filter(|n| n.source.start >= range.start && n.source.end <= range.end)
        .collect();
    let mut open: Vec<usize> = Vec::new();
    let mut out: Vec<Range<usize>> = Vec::new();
    for (i, node) in inside.iter().enumerate() {
        match node.kind {
            NodeKind::Push => open.push(i),
            NodeKind::Pop if open.pop().is_none() => out.push(node.source.clone()),
            _ => {}
        }
    }
    for i in open {
        // The group's own controls follow its brace.
        let mut end = inside[i].source.end;
        for node in &inside[i + 1..] {
            if node.source.start != end || !matches!(node.kind, NodeKind::Format(_)) {
                break;
            }
            end = node.source.end;
        }
        out.push(inside[i].source.start..end);
    }
    out.sort_by_key(|r| r.start);
    out
}

/// Replace `range` of `contents` with `inserted` (plain text, escaped here).
/// When the range cuts a group (holds one brace but not its partner) the
/// text goes and the brace and the group's controls stay, so the groups
/// still balance.
pub fn replace_text(
    contents: &str,
    range: Range<usize>,
    inserted: &str,
) -> Result<Edited, EditRefusal> {
    let limits = TextLimits::default();
    let parsed = parse_with(contents, limits).map_err(|_| EditRefusal::TooLarge)?;
    let kept = match check(&parsed, contents, &range) {
        Ok(()) => Vec::new(),
        Err(EditRefusal::SplitsGroup) => dangling(&parsed, &range),
        Err(why) => return Err(why),
    };
    let escaped = escape_for_contents(inserted);
    let kept_len: usize = kept.iter().map(|r| r.len()).sum();
    let new_len = contents.len() - range.len() + kept_len + escaped.len();
    if new_len > limits.max_content_bytes {
        return Err(EditRefusal::TooLarge);
    }
    let mut out = String::with_capacity(new_len);
    out.push_str(&contents[..range.start]);
    out.push_str(&escaped);
    for piece in &kept {
        out.push_str(&contents[piece.clone()]);
    }
    out.push_str(&contents[range.end..]);
    // The result must still be readable (node and paragraph limits).
    parse_with(&out, limits).map_err(|_| EditRefusal::TooLarge)?;
    Ok(Edited {
        contents: out,
        caret: range.start + escaped.len(),
    })
}

/// The plain text of `range` of the contents (for the clipboard): what the
/// reader sees, with paragraph breaks as newlines. A range that cuts a
/// control gives what is left of its whole nodes.
pub fn plain_text_of(contents: &str, range: Range<usize>) -> String {
    let Ok(parsed) = parse_with(contents, TextLimits::default()) else {
        return String::new();
    };
    let mut out = String::new();
    for node in &parsed.nodes {
        let n = &node.source;
        let piece = n.start.max(range.start)..n.end.min(range.end);
        if piece.start >= piece.end {
            continue;
        }
        match &node.kind {
            NodeKind::Text | NodeKind::Tab => {
                if let Some(text) = contents.get(piece) {
                    out.push_str(text);
                }
            }
            _ if piece != *n => {}
            NodeKind::EscapedChar => out.extend(contents[n.clone()].chars().nth(1)),
            NodeKind::ParagraphBreak => out.push('\n'),
            NodeKind::NonBreakingSpace => out.push('\u{a0}'),
            NodeKind::Unicode(c) => out.push(*c),
            NodeKind::Stack => {
                let text = &contents[n.clone()];
                out.push_str(text.get(2..text.len().saturating_sub(1)).unwrap_or(""));
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::parse;

    fn replaced(contents: &str, range: Range<usize>, text: &str) -> String {
        replace_text(contents, range, text).unwrap().contents
    }

    #[test]
    fn typed_text_is_escaped_into_contents() {
        assert_eq!(escape_for_contents("a\\b{c}"), "a\\\\b\\{c\\}");
        assert_eq!(
            escape_for_contents("one\ntwo\r\nthree\rfour"),
            "one\\Ptwo\\Pthree\\Pfour"
        );
        assert_eq!(escape_for_contents("tab\there é😀"), "tab\there é😀");
    }

    #[test]
    fn inserting_and_deleting_plain_text() {
        assert_eq!(replaced("abcd", 2..2, "XY"), "abXYcd");
        assert_eq!(replaced("abcd", 1..3, ""), "ad");
        assert_eq!(replaced("abcd", 1..3, "Q"), "aQd");
        assert_eq!(replaced("", 0..0, "hi\nthere"), "hi\\Pthere");
        let edited = replace_text("ab", 1..1, "{x}").unwrap();
        assert_eq!(edited.contents, "a\\{x\\}b");
        assert_eq!(edited.caret, 1 + "\\{x\\}".len());
        // What was typed reads back as typed.
        let p = parse(&edited.contents).unwrap();
        assert_eq!(p.plain_text(&edited.contents), "a{x}b");
    }

    #[test]
    fn edits_keep_the_controls_around_them() {
        let src = "\\LAB\\l cd";
        assert_eq!(replaced(src, 3..4, "X"), "\\LAX\\l cd");
    }

    #[test]
    fn the_ends_of_a_range_may_not_cut_a_control() {
        let src = "a\\H2.5;b";
        // Inside the `\H2.5;` control.
        assert_eq!(
            replace_text(src, 3..3, "x").unwrap_err(),
            EditRefusal::BadRange
        );
        assert_eq!(
            replace_text(src, 0..4, "").unwrap_err(),
            EditRefusal::BadRange
        );
        // At its edges is fine.
        assert!(replace_text(src, 1..1, "x").is_ok());
        assert!(replace_text(src, 8..8, "x").is_ok());
        assert_eq!(
            replace_text(src, 5..99, "").unwrap_err(),
            EditRefusal::BadRange
        );
        assert_eq!(
            replace_text("é", 1..1, "x").unwrap_err(),
            EditRefusal::BadRange
        );
        assert_eq!(
            replace_text("ab", std::ops::Range { start: 2, end: 1 }, "x").unwrap_err(),
            EditRefusal::BadRange
        );
    }

    #[test]
    fn controls_that_are_not_understood_are_protected() {
        let src = "a\\pxqd;b";
        assert_eq!(
            replace_text(src, 0..8, "").unwrap_err(),
            EditRefusal::Protected
        );
        assert_eq!(replace_text(src, 0..1, "").unwrap().contents, "\\pxqd;b");
        // Typing next to it is fine.
        assert!(replace_text(src, 8..8, "!").is_ok());
        assert_eq!(
            replace_text("a\\zb", 0..4, "").unwrap_err(),
            EditRefusal::Protected
        );
    }

    #[test]
    fn a_group_must_be_removed_whole_or_not_at_all() {
        let src = "a{\\L b}c";
        assert_eq!(replace_text(src, 1..7, "").unwrap().contents, "ac");
        // Cutting into it takes the text and leaves the braces, so it still
        // balances.
        assert_eq!(replace_text(src, 0..2, "").unwrap().contents, "{\\L b}c");
        assert_eq!(replace_text(src, 5..8, "").unwrap().contents, "a{\\L }");
        let typed = replace_text(src, 5..8, "xy").unwrap();
        assert_eq!(typed.contents, "a{\\L xy}");
        assert_eq!(typed.caret, 7);
        assert!(
            replace_text(src, 4..6, "").is_ok(),
            "inside the group is fine"
        );
    }

    #[test]
    fn the_size_limit_is_respected() {
        let big = "a".repeat(1 << 20);
        assert!(replace_text(&big, 0..1, "b").is_ok());
        assert_eq!(
            replace_text(&big, 0..0, "b").unwrap_err(),
            EditRefusal::TooLarge
        );
        assert_eq!(
            replace_text(&big, 0..0, "{").unwrap_err(),
            EditRefusal::TooLarge
        );
        let over = "a".repeat((1 << 20) + 1);
        assert_eq!(
            replace_text(&over, 0..0, "b").unwrap_err(),
            EditRefusal::TooLarge
        );
    }

    #[test]
    fn copying_gives_what_the_reader_sees() {
        let src = "ab\\Pc\\~d\\\\e\\U+00E9\\S1/2;\\H2;x";
        let all = plain_text_of(src, 0..src.len());
        assert_eq!(all, "ab\nc\u{a0}d\\e\u{e9}1/2x");
        assert_eq!(plain_text_of(src, 1..4), "b\n");
        assert_eq!(plain_text_of(src, 0..0), "");
        assert_eq!(plain_text_of("a\\pxi;b", 0..7), "ab");
    }
}
