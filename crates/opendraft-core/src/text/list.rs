//! Bulleted, numbered and lettered lists, written the way AutoCAD writes
//! them (`private_material/MText Example 2.dxf`). A list is not a control: each
//! item is a paragraph that starts with its marker as text, `1.`, `a.` or
//! `A.` and a tab, or a bullet, a middle dot in the Symbol font in a group,
//! then a tab. The hanging indent under it is a paragraph control, written
//! on the first item only (see `paragraph_edit`).

use std::ops::Range;

use super::edit::{plain_text_of, EditRefusal};
use super::format::Formatted;
use super::limits::TextLimits;
use super::paragraph::Paragraph;
use super::paragraph_edit::{apply_paragraph_edit, mapped, touched, ParagraphEdit};
use super::parse::{parse_with, Format, Node, NodeKind, ParsedMText};

/// The kinds of list the editor makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKind {
    Bullet,
    Number,
    LowerLetter,
    UpperLetter,
}

/// A bullet as AutoCAD writes it, including the tab after it.
const BULLET: &str = "{\\fSymbol|b0|i0|c2|p18;\u{b7}\t}";

/// A list item's marker: its kind, and the bytes it covers (with its tab).
struct Marker {
    kind: ListKind,
    /// Its place in the list, from 0 (`c.` is 2).
    index: usize,
    range: Range<usize>,
}

fn marker_of(parsed: &ParsedMText, contents: &str, paragraph: &Paragraph) -> Option<Marker> {
    let nodes: Vec<&Node> = parsed.nodes[paragraph.nodes.clone()]
        .iter()
        .filter(|n| !matches!(n.kind, NodeKind::Paragraph(_)))
        .collect();
    let text = |n: &Node| &contents[n.source.clone()];
    if let [push, font, dot, tab, pop, ..] = nodes.as_slice() {
        let symbol =
            matches!(font.kind, NodeKind::Format(Format::Font(_))) && text(font).contains("Symbol");
        if push.kind == NodeKind::Push
            && symbol
            && dot.kind == NodeKind::Text
            && text(dot) == "\u{b7}"
            && tab.kind == NodeKind::Tab
            && pop.kind == NodeKind::Pop
        {
            return Some(Marker {
                kind: ListKind::Bullet,
                index: 0,
                range: push.source.start..pop.source.end,
            });
        }
    }
    if let [item, tab, ..] = nodes.as_slice() {
        if item.kind == NodeKind::Text && tab.kind == NodeKind::Tab {
            let label = text(item).strip_suffix('.')?;
            let mut chars = label.chars();
            let (kind, index) = match (chars.next()?, chars.next()) {
                (c, None) if c.is_ascii_lowercase() => {
                    (ListKind::LowerLetter, usize::from(c as u8 - b'a'))
                }
                (c, None) if c.is_ascii_uppercase() => {
                    (ListKind::UpperLetter, usize::from(c as u8 - b'A'))
                }
                _ if label.bytes().all(|b| b.is_ascii_digit()) => (
                    ListKind::Number,
                    label.parse::<usize>().unwrap_or(1).saturating_sub(1),
                ),
                _ => return None,
            };
            return Some(Marker {
                kind,
                index,
                range: item.source.start..tab.source.end,
            });
        }
    }
    None
}

/// The marker for the `n`th item (from 0) of a list of `kind`.
fn marker_text(kind: ListKind, n: usize) -> String {
    let letter = |base: u8| char::from(base + (n % 26) as u8);
    match kind {
        ListKind::Bullet => BULLET.to_owned(),
        ListKind::Number => format!("{}.\t", n + 1),
        ListKind::LowerLetter => format!("{}.\t", letter(b'a')),
        ListKind::UpperLetter => format!("{}.\t", letter(b'A')),
    }
}

/// What Enter does in a list item, or `None` when the caret is not in one
/// (or the break should be an ordinary one). In an item with text, it ends
/// the item there and starts the next with the next marker; in an empty
/// item it ends the list, as in AutoCAD. The returned range is the caret.
pub fn break_list_item(
    contents: &str,
    range: Range<usize>,
) -> Result<Option<Formatted>, EditRefusal> {
    let limits = TextLimits::default();
    let Ok(parsed) = parse_with(contents, limits) else {
        return Ok(None);
    };
    if range.start > range.end
        || range.end > contents.len()
        || !contents.is_char_boundary(range.start)
        || !contents.is_char_boundary(range.end)
    {
        return Ok(None);
    }
    let (first, last) = touched(&parsed, &range);
    let Some(p) = parsed.paragraphs.get(first).filter(|_| first == last) else {
        return Ok(None);
    };
    let Some(marker) = marker_of(&parsed, contents, p).filter(|_| !p.protected) else {
        return Ok(None);
    };
    // Before the marker, a break is an ordinary one.
    if range.start < marker.range.end {
        return Ok(None);
    }
    let rest = plain_text_of(contents, marker.range.end..p.source.end);
    if range.is_empty() && rest.trim_end_matches('\n').is_empty() {
        return apply_list(contents, range, None, 0.0).map(Some);
    }
    let mut out = String::with_capacity(contents.len() + 32);
    out.push_str(&contents[..range.start]);
    out.push_str("\\P");
    out.push_str(&marker_text(marker.kind, marker.index + 1));
    let caret = out.len();
    out.push_str(&contents[range.end..]);
    if out.len() > limits.max_content_bytes {
        return Err(EditRefusal::TooLarge);
    }
    Ok(Some(Formatted {
        contents: out,
        range: caret..caret,
    }))
}

/// The kind of list every paragraph `range` touches belongs to, if they all
/// belong to the same one.
pub fn list_kind_in(contents: &str, range: Range<usize>) -> Option<ListKind> {
    let parsed = parse_with(contents, TextLimits::default()).ok()?;
    if range.start > range.end || range.end > contents.len() {
        return None;
    }
    let (first, last) = touched(&parsed, &range);
    let mut kinds = parsed
        .paragraphs
        .get(first..=last)?
        .iter()
        .map(|p| marker_of(&parsed, contents, p).map(|m| m.kind));
    let kind = kinds.next()??;
    kinds.all(|k| k == Some(kind)).then_some(kind)
}

/// Make the paragraphs `range` touches a list of `kind` (numbered from 1), or
/// take them out of any list with `None`. A list gets a hanging indent of
/// `indent` with a tab stop at it; leaving one removes the indent. The
/// returned range is `range` in the new contents.
pub fn apply_list(
    contents: &str,
    range: Range<usize>,
    kind: Option<ListKind>,
    indent: f64,
) -> Result<Formatted, EditRefusal> {
    let limits = TextLimits::default();
    let parsed = parse_with(contents, limits).map_err(|_| EditRefusal::TooLarge)?;
    if range.start > range.end
        || range.end > contents.len()
        || !contents.is_char_boundary(range.start)
        || !contents.is_char_boundary(range.end)
    {
        return Err(EditRefusal::BadRange);
    }
    if !indent.is_finite() || indent < 0.0 {
        return Err(EditRefusal::BadValue);
    }
    let (first, last) = touched(&parsed, &range);
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    for (n, p) in parsed.paragraphs[first..=last].iter().enumerate() {
        if p.protected {
            return Err(EditRefusal::Protected);
        }
        let old = marker_of(&parsed, contents, p);
        let new = kind.map(|k| marker_text(k, n));
        match (old, new) {
            (Some(m), new) => edits.push((m.range, new.unwrap_or_default())),
            (None, Some(new)) => {
                let at = parsed.nodes[p.nodes.clone()]
                    .iter()
                    .find(|n| !matches!(n.kind, NodeKind::Paragraph(_)))
                    .map_or(p.source.end, |n| n.source.start);
                edits.push((at..at, new));
            }
            (None, None) => {}
        }
    }
    let mut out = String::with_capacity(contents.len() + 24 * edits.len());
    let mut at = 0;
    for (old, text) in &edits {
        out.push_str(&contents[at..old.start]);
        out.push_str(text);
        at = old.end;
    }
    out.push_str(&contents[at..]);
    if out.len() > limits.max_content_bytes {
        return Err(EditRefusal::TooLarge);
    }
    let lengths: Vec<(Range<usize>, usize)> =
        edits.iter().map(|(r, t)| (r.clone(), t.len())).collect();
    // A selection that starts where a marker is added keeps it inside; a
    // bare caret goes after it.
    let before: Vec<(Range<usize>, usize)> = lengths
        .iter()
        .filter(|(r, _)| range.is_empty() || !(r.is_empty() && r.start == range.start))
        .cloned()
        .collect();
    let moved = mapped(&before, range.start)..mapped(&lengths, range.end);
    let hang = match kind {
        Some(_) => ParagraphEdit {
            first_indent: Some(-indent),
            left_indent: Some(indent),
            tabs: Some(vec![indent]),
            ..ParagraphEdit::default()
        },
        None => ParagraphEdit {
            first_indent: Some(0.0),
            left_indent: Some(0.0),
            tabs: Some(Vec::new()),
            ..ParagraphEdit::default()
        },
    };
    apply_paragraph_edit(&out, moved, &hang)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: &str = "LIST\\PONE\\PTWO\\PTHREE";

    fn items() -> Range<usize> {
        PLAIN.find("ONE").unwrap()..PLAIN.len()
    }

    #[test]
    fn a_numbered_list_comes_out_as_autocad_wrote_it() {
        // The first sample list, with a real tab where the file has `^I`.
        let done = apply_list(PLAIN, items(), Some(ListKind::Number), 3.0).unwrap();
        assert_eq!(
            done.contents,
            "LIST\\P\\pxi-3,l3,t3;1.\tONE\\P2.\tTWO\\P3.\tTHREE"
        );
        assert_eq!(
            &done.contents[done.range.clone()],
            "1.\tONE\\P2.\tTWO\\P3.\tTHREE"
        );
    }

    #[test]
    fn bullets_and_letters_match_the_samples_too() {
        let bullets = apply_list(PLAIN, items(), Some(ListKind::Bullet), 3.0).unwrap();
        let dot = "{\\fSymbol|b0|i0|c2|p18;\u{b7}\t}";
        assert_eq!(
            bullets.contents,
            format!("LIST\\P\\pxi-3,l3,t3;{dot}ONE\\P{dot}TWO\\P{dot}THREE")
        );
        let lower = apply_list(PLAIN, items(), Some(ListKind::LowerLetter), 3.0).unwrap();
        assert_eq!(
            lower.contents,
            "LIST\\P\\pxi-3,l3,t3;a.\tONE\\Pb.\tTWO\\Pc.\tTHREE"
        );
        let upper = apply_list(PLAIN, items(), Some(ListKind::UpperLetter), 3.0).unwrap();
        assert_eq!(
            upper.contents,
            "LIST\\P\\pxi-3,l3,t3;A.\tONE\\PB.\tTWO\\PC.\tTHREE"
        );
    }

    #[test]
    fn the_samples_are_recognised_as_lists() {
        for (src, kind) in [
            ("L\\P\\pxi-3,l3,t3;1.\tA\\P2.\tB", ListKind::Number),
            ("L\\P\\pxi-3,l3,t3;a.\tA\\Pb.\tB", ListKind::LowerLetter),
            ("L\\P\\pxi-3,l3,t3;A.\tA\\PB.\tB", ListKind::UpperLetter),
            (
                "L\\P\\pxi-3,l3,t3;{\\fSymbol|b0|i0|c2|p18;\u{b7}\t}A\\P{\\fSymbol|b0|i0|c2|p18;\u{b7}\t}B",
                ListKind::Bullet,
            ),
        ] {
            let at = src.find("\\P").unwrap() + 2;
            assert_eq!(list_kind_in(src, at..src.len()), Some(kind), "{src}");
            assert_eq!(list_kind_in(src, 0..0), None, "the heading is not an item");
        }
        assert_eq!(list_kind_in("1.plain", 0..0), None, "no tab, no marker");
        assert_eq!(list_kind_in("12.\tx", 0..0), Some(ListKind::Number));
        assert_eq!(list_kind_in("ab.\tx", 0..0), None);
    }

    #[test]
    fn switching_kind_replaces_the_markers_and_off_removes_them_and_the_indent() {
        let numbered = apply_list(PLAIN, items(), Some(ListKind::Number), 3.0).unwrap();
        let lettered = apply_list(
            &numbered.contents,
            numbered.range.clone(),
            Some(ListKind::UpperLetter),
            3.0,
        )
        .unwrap();
        assert_eq!(
            lettered.contents,
            "LIST\\P\\pxi-3,l3,t3;A.\tONE\\PB.\tTWO\\PC.\tTHREE"
        );
        let off = apply_list(&lettered.contents, lettered.range, None, 3.0).unwrap();
        assert_eq!(off.contents, PLAIN);
        assert_eq!(&off.contents[off.range], "ONE\\PTWO\\PTHREE");
    }

    #[test]
    fn a_caret_in_one_item_changes_only_that_item() {
        let at = PLAIN.find("TWO").unwrap() + 1;
        let done = apply_list(PLAIN, at..at, Some(ListKind::Number), 3.0).unwrap();
        assert_eq!(
            done.contents,
            "LIST\\PONE\\P\\pxi-3,l3,t3;1.\tTWO\\P\\px;THREE"
        );
        // The caret stays on the same letter.
        assert_eq!(&done.contents[done.range.start..done.range.start + 2], "WO");
    }

    #[test]
    fn empty_paragraphs_and_the_end_of_the_text_take_a_marker() {
        let done = apply_list("", 0..0, Some(ListKind::Bullet), 3.0).unwrap();
        assert!(done.contents.ends_with("\u{b7}\t}"));
        let blank = apply_list("a\\P\\Pb", 3..3, Some(ListKind::Number), 2.0).unwrap();
        assert_eq!(blank.contents, "a\\P\\pxi-2,l2,t2;1.\t\\P\\px;b");
    }

    #[test]
    fn enter_in_an_item_starts_the_next_and_in_an_empty_one_ends_the_list() {
        let list = "L\\P\\pxi-3,l3,t3;1.\tONE\\P2.\tTWO";
        let end = list.len();
        let next = break_list_item(list, end..end).unwrap().unwrap();
        assert_eq!(next.contents, format!("{list}\\P3.\t"));
        assert_eq!(next.range, next.contents.len()..next.contents.len());
        // Mid-item: the rest of the text moves to the new item.
        let mid = list.find("WO").unwrap();
        let split = break_list_item(list, mid..mid).unwrap().unwrap();
        assert_eq!(split.contents, "L\\P\\pxi-3,l3,t3;1.\tONE\\P2.\tT\\P3.\tWO");
        // Empty item: the marker and the indent go.
        let empty = next.contents.clone();
        let done = break_list_item(&empty, next.range).unwrap().unwrap();
        assert!(!done.contents.ends_with("3.\t"));
        assert_eq!(list_kind_in(&done.contents, done.range), None);
        // Letters and bullets continue too; plain text and the marker itself do not.
        let letters = "\\pxi-3,l3,t3;a.\tx";
        let l = break_list_item(letters, 17..17).unwrap().unwrap();
        assert!(l.contents.ends_with("\\Pb.\t"));
        let bullet = apply_list("x", 0..1, Some(ListKind::Bullet), 3.0).unwrap();
        let end = bullet.contents.len();
        let b = break_list_item(&bullet.contents, end..end)
            .unwrap()
            .unwrap();
        assert!(b.contents.ends_with(&format!("\\P{BULLET}")));
        assert!(break_list_item("plain", 5..5).unwrap().is_none());
        assert!(break_list_item(list, 0..0).unwrap().is_none());
        let in_marker = list.find("1.").unwrap() + 1;
        assert!(break_list_item(list, in_marker..in_marker)
            .unwrap()
            .is_none());
    }

    #[test]
    fn protected_paragraphs_and_bad_indents_are_refused() {
        assert_eq!(
            apply_list("\\pxqd;a", 0..0, Some(ListKind::Number), 3.0).unwrap_err(),
            EditRefusal::Protected
        );
        assert_eq!(
            apply_list("a", 0..0, Some(ListKind::Number), f64::NAN).unwrap_err(),
            EditRefusal::BadValue
        );
        assert_eq!(
            apply_list("a", 0..9, Some(ListKind::Number), 3.0).unwrap_err(),
            EditRefusal::BadRange
        );
    }
}
