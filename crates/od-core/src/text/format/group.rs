//! Where a character edit lands: the group it reuses, and what it replaces.
//!
//! A selection that is exactly a group, braces and all (what Select All
//! gives once the text is in one), is the group's inside; wrapping it again
//! would nest a new group on every edit until the ninth is unreadable. And
//! controls an edit overrides at the start of a reused group are dropped, so
//! dragging a value does not stack one control per frame.

use std::ops::Range;

use super::CharEdit;
use crate::text::parse::{Format, NodeKind, ParsedMText};

/// `range` with the braces of every group it exactly encloses taken off,
/// and the controls that open the innermost: `{{\L x}}` selected whole is
/// the `x`, as a selection made by hand would be.
pub(super) fn inside_groups(parsed: &ParsedMText, range: &Range<usize>) -> Range<usize> {
    let mut inner = range.clone();
    let mut peeled = false;
    while let Some(next) = one_group(parsed, &inner) {
        inner = next;
        peeled = true;
    }
    if !peeled {
        return inner;
    }
    while let Some(node) = parsed.nodes.iter().find(|n| {
        n.source.start == inner.start
            && n.source.end <= inner.end
            && matches!(n.kind, NodeKind::Format(_))
    }) {
        inner.start = node.source.end;
    }
    inner
}

/// The inside of the group that `range` is exactly, if it is one.
fn one_group(parsed: &ParsedMText, range: &Range<usize>) -> Option<Range<usize>> {
    let open = parsed
        .nodes
        .iter()
        .position(|n| n.source.start == range.start && n.kind == NodeKind::Push)?;
    let mut depth = 0i64;
    for node in &parsed.nodes[open..] {
        match node.kind {
            NodeKind::Push => depth += 1,
            NodeKind::Pop => {
                depth -= 1;
                if depth == 0 {
                    return (node.source.end == range.end)
                        .then(|| parsed.nodes[open].source.end..node.source.start);
                }
            }
            _ => {}
        }
    }
    None
}

/// The stretches of `range` an edit applies to when `range` cuts groups:
/// what is left of it between the braces (and the controls that open the
/// groups) in `cut`, leaving out stretches with no text in them.
pub(super) fn pieces(
    parsed: &ParsedMText,
    range: &Range<usize>,
    cut: &[Range<usize>],
) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut from = range.start;
    for gap in cut.iter().map(Some).chain([None]) {
        let to = gap.map_or(range.end, |g| g.start);
        if from < to && has_text(parsed, &(from..to)) {
            out.push(from..to);
        }
        if let Some(g) = gap {
            from = g.end;
        }
    }
    out
}

/// Whether anything is shown in `range` (text can be cut part way through).
fn has_text(parsed: &ParsedMText, range: &Range<usize>) -> bool {
    parsed.nodes.iter().any(|n| {
        n.source.start < range.end
            && n.source.end > range.start
            && matches!(
                n.kind,
                NodeKind::Text
                    | NodeKind::Tab
                    | NodeKind::EscapedChar
                    | NodeKind::NonBreakingSpace
                    | NodeKind::Unicode(_)
                    | NodeKind::Stack
                    | NodeKind::Field
            )
    })
}

/// Whether `edit` sets what `format` sets, so the control can go.
pub(super) fn superseded(edit: &CharEdit, format: &Format) -> bool {
    match format {
        Format::Height(_) => edit.height.is_some(),
        Format::Width(_) => edit.width.is_some(),
        Format::Tracking(_) => edit.tracking.is_some(),
        Format::Oblique(_) => edit.oblique.is_some(),
        Format::Aci(_) | Format::TrueColor(_) => edit.color.is_some(),
        Format::Underline(_) => edit.underline.is_some(),
        Format::Overline(_) => edit.overline.is_some(),
        Format::Strike(_) => edit.strike.is_some(),
        Format::Font(_) => edit.bold.is_some() || edit.italic.is_some() || edit.weight.is_some(),
        Format::Align(_) => false,
    }
}
