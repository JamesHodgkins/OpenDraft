//! Editing paragraph formatting: indents and tab stops, the controls the
//! editor's ruler sets. Each paragraph's own `\p` control is rewritten (or
//! added, or dropped when nothing is left to say) and nothing else is touched.

use std::ops::Range;

use super::edit::EditRefusal;
use super::format::Formatted;
use super::limits::TextLimits;
use super::paragraph::{encode, ParagraphAlignment, ParagraphStyle, TabKind, TabStop};
use super::parse::{parse_with, NodeKind, ParsedMText};

/// Most tab stops a paragraph may carry (the decoder's limit).
const MAX_TABS: usize = 32;
const MAX_DISTANCE: f64 = 1e9;

/// A change to paragraphs: what is `None` is left as it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParagraphEdit {
    pub first_indent: Option<f64>,
    pub left_indent: Option<f64>,
    pub alignment: Option<ParagraphAlignment>,
    /// The tab stops, replacing the paragraph's own.
    pub tabs: Option<Vec<f64>>,
    pub space_before: Option<f64>,
    pub space_after: Option<f64>,
    /// `Some(None)` drops the paragraph's own multiple.
    pub line_spacing: Option<Option<f64>>,
}

impl ParagraphEdit {
    /// An edit that sets everything `style` says.
    pub fn of(style: &ParagraphStyle) -> Self {
        Self {
            first_indent: Some(style.first_indent),
            left_indent: Some(style.left_indent),
            alignment: Some(style.alignment),
            tabs: Some(style.tabs.iter().map(|t| t.offset).collect()),
            space_before: Some(style.space_before),
            space_after: Some(style.space_after),
            line_spacing: Some(style.line_spacing),
        }
    }

    fn applied(&self, old: &ParagraphStyle) -> Result<ParagraphStyle, EditRefusal> {
        let ok = |v: f64| v.is_finite() && v.abs() <= MAX_DISTANCE;
        let mut style = old.clone();
        if let Some(v) = self.first_indent {
            if !ok(v) {
                return Err(EditRefusal::BadValue);
            }
            style.first_indent = v;
        }
        if let Some(v) = self.left_indent {
            if !ok(v) {
                return Err(EditRefusal::BadValue);
            }
            style.left_indent = v;
        }
        if let Some(alignment) = self.alignment {
            style.alignment = alignment;
        }
        for (new, old) in [
            (self.space_before, &mut style.space_before),
            (self.space_after, &mut style.space_after),
        ] {
            if let Some(v) = new {
                if !ok(v) || v < 0.0 {
                    return Err(EditRefusal::BadValue);
                }
                *old = v;
            }
        }
        if let Some(multiple) = self.line_spacing {
            if multiple.is_some_and(|v| !(0.25..=4.0).contains(&v)) {
                return Err(EditRefusal::BadValue);
            }
            style.line_spacing = multiple;
        }
        if let Some(tabs) = &self.tabs {
            if tabs.len() > MAX_TABS || tabs.iter().any(|t| !ok(*t) || *t < 0.0) {
                return Err(EditRefusal::BadValue);
            }
            let mut sorted = tabs.clone();
            sorted.sort_by(f64::total_cmp);
            sorted.dedup();
            style.tabs = sorted
                .into_iter()
                .map(|offset| TabStop {
                    offset,
                    kind: TabKind::Left,
                    decimal: '.',
                })
                .collect();
        }
        Ok(style)
    }
}

/// A paragraph as the ruler shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct ParagraphView {
    pub style: ParagraphStyle,
    /// It holds formatting that cannot be edited yet.
    pub protected: bool,
}

/// The index of the paragraph that byte `at` belongs to. A paragraph owns
/// its closing break, so the start of the next one belongs to that one.
fn paragraph_index(parsed: &ParsedMText, at: usize) -> usize {
    parsed
        .paragraphs
        .iter()
        .position(|p| p.source.start <= at && at < p.source.end)
        .unwrap_or(parsed.paragraphs.len().saturating_sub(1))
}

/// The first and last paragraph `range` touches. A selection ending right
/// after a break does not touch the next paragraph.
pub(super) fn touched(parsed: &ParsedMText, range: &Range<usize>) -> (usize, usize) {
    let first = paragraph_index(parsed, range.start);
    let mut last = paragraph_index(parsed, range.end);
    if range.end > range.start
        && last > first
        && parsed
            .paragraphs
            .get(last)
            .is_some_and(|p| p.source.start == range.end)
    {
        last -= 1;
    }
    (first, last)
}

/// The paragraph holding byte `at`: what the ruler shows for a caret there.
pub fn paragraph_view_in(contents: &str, at: usize) -> ParagraphView {
    let plain = ParagraphView {
        style: ParagraphStyle::default(),
        protected: false,
    };
    let Ok(parsed) = parse_with(contents, TextLimits::default()) else {
        return ParagraphView {
            protected: true,
            ..plain
        };
    };
    let Some(p) = parsed.paragraphs.get(paragraph_index(&parsed, at)) else {
        return plain;
    };
    ParagraphView {
        style: p.style.clone(),
        protected: p.protected,
    }
}

/// Where a byte offset of the old contents lands after `edits` (each an old
/// range and the length of what replaced it, ascending). Text inserted at an
/// offset goes before it; an offset inside something removed goes to its end.
pub(super) fn mapped(edits: &[(Range<usize>, usize)], at: usize) -> usize {
    let mut delta: i64 = 0;
    for (old, new_len) in edits {
        let change = *new_len as i64 - old.len() as i64;
        if at >= old.end {
            delta += change;
        } else if at > old.start {
            return usize::try_from(old.start as i64 + delta + *new_len as i64).unwrap_or(0);
        } else {
            break;
        }
    }
    usize::try_from(at as i64 + delta).unwrap_or(0)
}

/// Apply `edit` to every paragraph `range` touches. The returned range is
/// `range` in the new contents.
pub fn apply_paragraph_edit(
    contents: &str,
    range: Range<usize>,
    edit: &ParagraphEdit,
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
    let (first, last) = touched(&parsed, &range);
    // A control holds until the next one, so each paragraph's own control is
    // kept only where its style differs from what it would inherit. The
    // paragraphs touched take the change; the ones after them keep the look
    // they had, which may need a control of their own now.
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    let mut inherited = ParagraphStyle::default();
    for (k, p) in parsed.paragraphs.iter().enumerate() {
        let own: Vec<Range<usize>> = parsed.nodes[p.nodes.clone()]
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Paragraph(_)))
            .map(|n| n.source.clone())
            .collect();
        let touched = (first..=last).contains(&k);
        if touched && p.protected {
            return Err(EditRefusal::Protected);
        }
        let style = if touched {
            edit.applied(&p.style)?
        } else {
            p.style.clone()
        };
        if touched || (own.is_empty() && !p.protected) {
            let wanted = (style != inherited).then(|| encode(&style));
            let kept = match (&wanted, own.as_slice()) {
                (None, []) => true,
                (Some(w), [only]) => contents[only.clone()] == *w,
                _ => false,
            };
            if !kept {
                edits.extend(wanted.map(|w| (p.source.start..p.source.start, w)));
                edits.extend(own.into_iter().map(|r| (r, String::new())));
            }
        }
        inherited = style;
    }
    let mut out = String::with_capacity(contents.len() + 16 * edits.len());
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
    parse_with(&out, limits).map_err(|_| EditRefusal::TooLarge)?;
    let lengths: Vec<(Range<usize>, usize)> =
        edits.iter().map(|(r, t)| (r.clone(), t.len())).collect();
    Ok(Formatted {
        contents: out,
        range: mapped(&lengths, range.start)..mapped(&lengths, range.end),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn indent(first: f64, left: f64) -> ParagraphEdit {
        ParagraphEdit {
            first_indent: Some(first),
            left_indent: Some(left),
            ..ParagraphEdit::default()
        }
    }

    #[test]
    fn a_paragraph_gets_a_control_and_loses_it_when_nothing_is_left() {
        let done = apply_paragraph_edit("hello", 2..2, &indent(-4.0, 4.0)).unwrap();
        assert_eq!(done.contents, "\\pxi-4,l4;hello");
        assert_eq!(done.range, 12..12, "the caret stays on the same letter");
        let back = apply_paragraph_edit(&done.contents, done.range, &indent(0.0, 0.0)).unwrap();
        assert_eq!(back.contents, "hello");
        assert_eq!(back.range, 2..2);
    }

    #[test]
    fn an_existing_control_is_rewritten_and_an_unchanged_one_is_left_alone() {
        let src = "\\pxi4;one\\Ptwo";
        let done = apply_paragraph_edit(src, 0..0, &indent(1.0, 0.0)).unwrap();
        assert_eq!(
            done.contents, "\\pxi1;one\\P\\pxi4;two",
            "the next paragraph keeps the look it inherited"
        );
        let same = apply_paragraph_edit(src, 0..0, &indent(4.0, 0.0)).unwrap();
        assert_eq!(same.contents, src);
        assert_eq!(same.range, 0..0);
    }

    #[test]
    fn a_selection_changes_every_paragraph_it_touches() {
        let src = "one\\Ptwo\\Pthree";
        // From inside the first to inside the second.
        let done = apply_paragraph_edit(src, 1..9, &indent(0.0, 2.0)).unwrap();
        assert_eq!(
            done.contents, "\\pxl2;one\\Ptwo\\P\\px;three",
            "two inherits the change; three is put back as it was"
        );
        // Ending right after a break leaves the next paragraph alone.
        let done = apply_paragraph_edit(src, 0..5, &indent(0.0, 2.0)).unwrap();
        assert_eq!(done.contents, "\\pxl2;one\\P\\px;two\\Pthree");
    }

    #[test]
    fn a_list_from_the_second_autocad_sample_edits_as_one_run() {
        // `\pxi-3,l3,t3;` on the first item holds for the items after it.
        let src = "LIST\\P\\pxi-3,l3,t3;1.\tA\\P2.\tB\\P3.\tC";
        let view = paragraph_view_in(src, src.len());
        assert_eq!(view.style.left_indent, 3.0);
        assert!(!view.protected);
        assert_eq!(paragraph_view_in(src, 0).style.left_indent, 0.0);
        // Changing the middle item pins the last one to the old look.
        let at = src.find("2.").unwrap();
        let done = apply_paragraph_edit(src, at..at, &indent(0.0, 0.0)).unwrap();
        assert_eq!(
            done.contents,
            "LIST\\P\\pxi-3,l3,t3;1.\tA\\P\\pxt3;2.\tB\\P\\pxi-3,l3,t3;3.\tC"
        );
    }

    #[test]
    fn alignment_is_set_and_inherited_like_the_other_controls() {
        let centre = ParagraphEdit {
            alignment: Some(ParagraphAlignment::Center),
            ..ParagraphEdit::default()
        };
        let done = apply_paragraph_edit("a\\Pb", 0..0, &centre).unwrap();
        assert_eq!(done.contents, "\\pxqc;a\\P\\px;b");
        let view = paragraph_view_in("\\pxqj;a\\Pb", 8);
        assert_eq!(view.style.alignment, ParagraphAlignment::Justified);
    }

    #[test]
    fn tab_stops_are_sorted_and_replaced() {
        let edit = ParagraphEdit {
            tabs: Some(vec![6.0, 2.0, 2.0]),
            ..ParagraphEdit::default()
        };
        let done = apply_paragraph_edit("a", 0..0, &edit).unwrap();
        assert_eq!(done.contents, "\\pxt2,6;a");
        let none = ParagraphEdit {
            tabs: Some(Vec::new()),
            ..ParagraphEdit::default()
        };
        assert_eq!(
            apply_paragraph_edit(&done.contents, 0..0, &none)
                .unwrap()
                .contents,
            "a"
        );
    }

    #[test]
    fn protected_paragraphs_and_bad_values_are_refused() {
        assert_eq!(
            apply_paragraph_edit("\\pxqd;a", 0..0, &indent(1.0, 0.0)).unwrap_err(),
            EditRefusal::Protected
        );
        assert_eq!(
            apply_paragraph_edit("a", 0..0, &indent(f64::NAN, 0.0)).unwrap_err(),
            EditRefusal::BadValue
        );
        let negative = ParagraphEdit {
            tabs: Some(vec![-1.0]),
            ..ParagraphEdit::default()
        };
        assert_eq!(
            apply_paragraph_edit("a", 0..0, &negative).unwrap_err(),
            EditRefusal::BadValue
        );
        assert_eq!(
            apply_paragraph_edit("a", 0..5, &indent(1.0, 0.0)).unwrap_err(),
            EditRefusal::BadRange
        );
    }

    #[test]
    fn the_view_is_the_paragraph_holding_the_byte() {
        let src = "\\pxl3;one\\Ptwo";
        assert_eq!(paragraph_view_in(src, 7).style.left_indent, 3.0);
        assert_eq!(
            paragraph_view_in(src, 12).style.left_indent,
            3.0,
            "the control holds for the paragraphs after it"
        );
        assert!(paragraph_view_in("\\pxqd;a", 0).protected);
        assert!(!paragraph_view_in("", 0).protected);
    }
}
