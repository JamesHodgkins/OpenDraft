//! Stacked text, `\S1/2;`: a numerator over a denominator.
//!
//! The divider says how: `/` a fraction with a bar, `^` the two stacked with
//! no bar (tolerances), `#` a slanted fraction. A backslash makes the next
//! character literal, so `\S1\/2^3;` is "1/2" over "3".

use std::ops::Range;

use super::edit::{check, escape_for_contents, EditRefusal};
use super::format::Formatted;
use super::limits::TextLimits;
use super::parse::{parse_with, NodeKind};

/// A stack's text taken apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackParts {
    pub numerator: String,
    pub denominator: String,
    /// `None` when there is no divider, so there is nothing to stack.
    pub divider: Option<char>,
}

/// Split the text between `\S` and `;` at its first unescaped divider.
pub fn split_stack(payload: &str) -> StackParts {
    let (mut numerator, mut denominator) = (String::new(), String::new());
    let mut divider = None;
    let mut chars = payload.chars();
    while let Some(c) = chars.next() {
        let (literal, c) = if c == '\\' {
            match chars.next() {
                Some(next) => (true, next),
                None => break,
            }
        } else {
            (false, c)
        };
        let target = if divider.is_some() {
            &mut denominator
        } else {
            &mut numerator
        };
        if !literal && divider.is_none() && matches!(c, '/' | '^' | '#') {
            divider = Some(c);
        } else {
            target.push(c);
        }
    }
    StackParts {
        numerator,
        denominator,
        divider,
    }
}

/// Stack or unstack the text in `range`. Selected plain text with a divider
/// in it (`1/2`) becomes a stack; a selection that is exactly one stack
/// becomes plain text again.
pub fn toggle_stack(contents: &str, range: Range<usize>) -> Result<Formatted, EditRefusal> {
    let limits = TextLimits::default();
    let parsed = parse_with(contents, limits).map_err(|_| EditRefusal::TooLarge)?;
    check(&parsed, contents, &range)?;
    if range.start >= range.end {
        return Err(EditRefusal::NoDivider);
    }
    let inside: Vec<_> = parsed
        .nodes
        .iter()
        .filter(|n| n.source.start < range.end && n.source.end > range.start)
        .collect();
    let replacement = match inside.as_slice() {
        [one] if one.kind == NodeKind::Stack && one.source == range => {
            let text = &contents[range.clone()];
            let parts = split_stack(text.get(2..text.len().saturating_sub(1)).unwrap_or(""));
            let divider = parts.divider.unwrap_or('/');
            escape_for_contents(&format!(
                "{}{divider}{}",
                parts.numerator, parts.denominator
            ))
        }
        nodes if nodes.iter().all(|n| n.kind == NodeKind::Text) => {
            let text = &contents[range.clone()];
            if text.contains(['\\', ';', '{', '}']) {
                return Err(EditRefusal::CannotStack);
            }
            if !text.contains(['/', '^', '#']) {
                return Err(EditRefusal::NoDivider);
            }
            format!("\\S{text};")
        }
        _ => return Err(EditRefusal::CannotStack),
    };
    let mut out = String::with_capacity(contents.len() + replacement.len());
    out.push_str(&contents[..range.start]);
    out.push_str(&replacement);
    out.push_str(&contents[range.end..]);
    if out.len() > limits.max_content_bytes {
        return Err(EditRefusal::TooLarge);
    }
    parse_with(&out, limits).map_err(|_| EditRefusal::TooLarge)?;
    Ok(Formatted {
        contents: out,
        range: range.start..range.start + replacement.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stack_splits_at_its_first_unescaped_divider() {
        let p = split_stack("1/2");
        assert_eq!((p.numerator.as_str(), p.denominator.as_str()), ("1", "2"));
        assert_eq!(p.divider, Some('/'));
        assert_eq!(split_stack("+0.1^-0.2").divider, Some('^'));
        assert_eq!(split_stack("a#b").divider, Some('#'));
        let escaped = split_stack("1\\/2^3");
        assert_eq!(escaped.numerator, "1/2");
        assert_eq!(escaped.denominator, "3");
        assert_eq!(escaped.divider, Some('^'));
        let none = split_stack("abc");
        assert_eq!((none.numerator.as_str(), none.divider), ("abc", None));
        let again = split_stack("1/2/3");
        assert_eq!(again.denominator, "2/3", "only the first divides");
        assert_eq!(split_stack("/2").numerator, "");
        assert_eq!(split_stack("1/").denominator, "");
        assert_eq!(split_stack("").divider, None);
        assert_eq!(
            split_stack("a\\").numerator,
            "a",
            "a trailing backslash is dropped"
        );
    }

    #[test]
    fn selected_text_with_a_divider_becomes_a_stack() {
        let done = toggle_stack("x 1/2 y", 2..5).unwrap();
        assert_eq!(done.contents, "x \\S1/2; y");
        assert_eq!(&done.contents[done.range], "\\S1/2;");
        assert_eq!(toggle_stack("1^2", 0..3).unwrap().contents, "\\S1^2;");
        assert_eq!(toggle_stack("a#b", 0..3).unwrap().contents, "\\Sa#b;");
        assert_eq!(toggle_stack("/2", 0..2).unwrap().contents, "\\S/2;");
    }

    #[test]
    fn a_stack_selected_whole_becomes_plain_text_again() {
        let stacked = toggle_stack("x 1/2 y", 2..5).unwrap();
        let back = toggle_stack(&stacked.contents, stacked.range).unwrap();
        assert_eq!(back.contents, "x 1/2 y");
        assert_eq!(&back.contents[back.range], "1/2");
        // The text of a stack with an escaped divider comes back escaped.
        let odd = toggle_stack("\\S1\\/2^3;", 0..9).unwrap();
        assert_eq!(odd.contents, "1/2^3");
    }

    #[test]
    fn what_cannot_be_stacked_is_refused() {
        assert_eq!(
            toggle_stack("abc", 0..3).unwrap_err(),
            EditRefusal::NoDivider
        );
        assert_eq!(
            toggle_stack("abc", 1..1).unwrap_err(),
            EditRefusal::NoDivider
        );
        assert_eq!(
            toggle_stack("1/2;3", 0..5).unwrap_err(),
            EditRefusal::CannotStack
        );
        assert_eq!(
            toggle_stack("1\\\\/2", 0..5).unwrap_err(),
            EditRefusal::CannotStack
        );
        // A selection with formatting in it.
        assert_eq!(
            toggle_stack("1\\L/2", 0..5).unwrap_err(),
            EditRefusal::CannotStack
        );
        // Cutting a control, or covering one that is not understood.
        assert_eq!(
            toggle_stack("a\\H2;1/2", 0..4).unwrap_err(),
            EditRefusal::BadRange
        );
        assert_eq!(
            toggle_stack("1/2\\pxi;", 0..8).unwrap_err(),
            EditRefusal::Protected
        );
        assert_eq!(
            toggle_stack("1/2", 0..9).unwrap_err(),
            EditRefusal::BadRange
        );
    }
}
