//! Paragraph state, mirroring Autodesk's `TextEditorParagraph` so nothing is
//! collapsed on the way in (`Default` is not `Left`, a wide numbering mode is
//! not its narrow counterpart).
//!
//! Only what AutoCAD-made files have confirmed is decoded from the contents
//! (see [`decode`]): first-line indent, left indent and left tab stops, as in
//! `\pxi-4,l4;` or `\pxt2;`. Autodesk does not publish the rest of the
//! grammar, so a `\p...;` control with anything else in it stays opaque and
//! its paragraph is marked [`Paragraph::protected`].

use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabKind {
    Left,
    Center,
    Right,
    Decimal,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TabStop {
    pub offset: f64,
    pub kind: TabKind,
    pub decimal: char,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ParagraphAlignment {
    #[default]
    Default,
    Left,
    Center,
    Right,
    Justified,
    Distributed,
}

/// Autodesk's `NumberingType`, exactly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NumberingKind {
    #[default]
    Off,
    Bullet,
    Number,
    LetterLower,
    LetterUpper,
    NumberWide,
    LetterLowerWide,
    LetterUpperWide,
    /// A value this build does not know, kept as read.
    Opaque(i32),
}

/// Where a paragraph's list facts came from. Only ever read from a control
/// verified against AutoCAD output; never made up from how paragraphs sit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ListSource {
    #[default]
    None,
    Opaque {
        raw: Range<usize>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParagraphStyle {
    /// Relative to `left_indent`: the first line starts at their sum.
    pub first_indent: f64,
    pub left_indent: f64,
    pub right_indent: f64,
    pub alignment: ParagraphAlignment,
    pub tabs: Vec<TabStop>,
    pub space_before: f64,
    pub space_after: f64,
    pub numbering: NumberingKind,
    pub list_source: ListSource,
}

impl Default for ParagraphStyle {
    fn default() -> Self {
        Self {
            first_indent: 0.0,
            left_indent: 0.0,
            right_indent: 0.0,
            alignment: ParagraphAlignment::Default,
            tabs: Vec::new(),
            space_before: 0.0,
            space_after: 0.0,
            numbering: NumberingKind::Off,
            list_source: ListSource::None,
        }
    }
}

/// One paragraph of the contents: what lies between paragraph breaks.
#[derive(Clone, Debug, PartialEq)]
pub struct Paragraph {
    /// Bytes of the contents, including the `\P` that ends it.
    pub source: Range<usize>,
    /// Indexes into [`super::ParsedMText::nodes`].
    pub nodes: Range<usize>,
    pub style: ParagraphStyle,
    /// Carries a paragraph control this build cannot decode, so edits to its
    /// paragraph formatting are refused rather than guessed.
    pub protected: bool,
}

/// Most tab stops one control may carry.
const MAX_TABS: usize = 32;
/// Largest distance accepted, in drawing units.
const MAX_DISTANCE: f64 = 1e9;

fn distance(text: &str) -> Option<f64> {
    let v: f64 = text.parse().ok()?;
    (v.is_finite() && v.abs() <= MAX_DISTANCE).then_some(v)
}

/// The paragraph a `\p` payload (the text between `\p` and `;`) describes:
/// `x` then comma-separated `i<first indent>`, `l<left indent>` and
/// `q<l|c|r|j>` (alignment) and `t<tab>[,<tab>...]`. `None` for anything
/// else, which stays opaque.
pub(super) fn decode(payload: &str) -> Option<ParagraphStyle> {
    let keys = payload.strip_prefix('x')?;
    let mut style = ParagraphStyle::default();
    if keys.is_empty() {
        return Some(style);
    }
    let (mut seen_i, mut seen_l, mut seen_q, mut in_tabs) = (false, false, false, false);
    for part in keys.split(',') {
        if let Some(rest) = part.strip_prefix('i').filter(|_| !seen_i) {
            (style.first_indent, seen_i, in_tabs) = (distance(rest)?, true, false);
        } else if let Some(rest) = part.strip_prefix('l').filter(|_| !seen_l) {
            (style.left_indent, seen_l, in_tabs) = (distance(rest)?, true, false);
        } else if let Some(rest) = part.strip_prefix('q').filter(|_| !seen_q) {
            style.alignment = match rest {
                "l" => ParagraphAlignment::Left,
                "c" => ParagraphAlignment::Center,
                "r" => ParagraphAlignment::Right,
                "j" => ParagraphAlignment::Justified,
                _ => return None,
            };
            (seen_q, in_tabs) = (true, false);
        } else {
            // `t<n>` opens the tab list and bare numbers after it continue it.
            let number = match part.strip_prefix('t') {
                Some(rest) if style.tabs.is_empty() && !in_tabs => {
                    in_tabs = true;
                    rest
                }
                _ if in_tabs => part,
                _ => return None,
            };
            let offset = distance(number).filter(|v| *v >= 0.0)?;
            if style.tabs.len() >= MAX_TABS {
                return None;
            }
            style.tabs.push(TabStop {
                offset,
                kind: TabKind::Left,
                decimal: '.',
            });
        }
    }
    Some(style)
}

/// The `\p` control for `style`, with only what [`decode`] understands.
pub fn encode(style: &ParagraphStyle) -> String {
    let mut keys = Vec::new();
    if style.first_indent != 0.0 {
        keys.push(format!("i{}", style.first_indent));
    }
    if style.left_indent != 0.0 {
        keys.push(format!("l{}", style.left_indent));
    }
    let alignment = match style.alignment {
        ParagraphAlignment::Left => Some('l'),
        ParagraphAlignment::Center => Some('c'),
        ParagraphAlignment::Right => Some('r'),
        ParagraphAlignment::Justified => Some('j'),
        _ => None,
    };
    keys.extend(alignment.map(|a| format!("q{a}")));
    if !style.tabs.is_empty() {
        let tabs: Vec<String> = style.tabs.iter().map(|t| t.offset.to_string()).collect();
        keys.push(format!("t{}", tabs.join(",")));
    }
    format!("\\px{};", keys.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_controls_seen_in_autocad_files_decode() {
        // From the AutoCAD sample in private_material/MText Example.dxf.
        let tab = decode("xt2").unwrap();
        assert_eq!(tab.tabs.len(), 1);
        assert_eq!(tab.tabs[0].offset, 2.0);
        assert_eq!(decode("xi4").unwrap().first_indent, 4.0);
        let hanging = decode("xi-4,l4").unwrap();
        assert_eq!((hanging.first_indent, hanging.left_indent), (-4.0, 4.0));
        let tabs = decode("xt2,5.5,9").unwrap();
        let offsets: Vec<f64> = tabs.tabs.iter().map(|t| t.offset).collect();
        assert_eq!(offsets, [2.0, 5.5, 9.0]);
        // Alignment, from the second AutoCAD sample (`qr`, `qc`, `qj`).
        assert_eq!(decode("xqr").unwrap().alignment, ParagraphAlignment::Right);
        assert_eq!(decode("xqc").unwrap().alignment, ParagraphAlignment::Center);
        assert_eq!(
            decode("xqj").unwrap().alignment,
            ParagraphAlignment::Justified
        );
        // The list control from the same sample.
        let list = decode("xi-3,l3,t3").unwrap();
        assert_eq!((list.first_indent, list.left_indent), (-3.0, 3.0));
        assert_eq!(list.tabs[0].offset, 3.0);
    }

    #[test]
    fn what_has_not_been_seen_stays_undecoded() {
        for payload in [
            "", "x?", "i4", "xqz", "xi", "xi1,i2", "xt-1", "xinf", "xl4,r2", "xqc,qr",
        ] {
            assert!(decode(payload).is_none(), "{payload:?}");
        }
    }

    #[test]
    fn encoding_reads_back_the_same() {
        for payload in ["i4", "i-4,l4", "t2,5.5", "i1,l2,qc,t3", "qj"] {
            let style = decode(&format!("x{payload}")).unwrap();
            let text = encode(&style);
            assert_eq!(text, format!("\\px{payload};"));
            assert_eq!(decode(&text[2..text.len() - 1]).unwrap(), style);
        }
    }
}
