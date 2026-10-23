//! Breaking shaped clusters into lines and placing them.

use std::ops::Range;

use super::items::{CharState, Elem};
use super::{
    CaretStop, Context, Decoration, DecorationKind, LaidOutLine, PositionedGlyph, TextLayout,
    DEFAULT_TAB, LINE_PITCH,
};
use crate::mtext::LineSpacingStyle;
use crate::text::paragraph::{ParagraphAlignment, ParagraphStyle};

const EPS: f64 = 1e-9;
/// Slants beyond this are clamped, since the shear grows without bound.
const MAX_SLANT_DEGREES: f64 = 85.0;

/// One line: the elements on it, and where it ends if a hard break ended it.
struct Span {
    elems: Range<usize>,
    /// The paragraph break that ended it, if one did.
    hard: Option<Range<usize>>,
    /// Where the line starts: the paragraph's left indent, plus its first
    /// line indent on the first line.
    x0: f64,
    /// The paragraph's custom tab stops, ascending.
    tabs: Vec<f64>,
    align: ParagraphAlignment,
    /// The line ends its paragraph (a break, or the end of the text), so
    /// justifying leaves it alone.
    ends_paragraph: bool,
}

fn next_stop(x: f64, tab: f64) -> f64 {
    ((x / tab + EPS).floor() + 1.0) * tab
}

/// The tab after `x`: the paragraph's own stops first, then the default grid.
fn tab_after(x: f64, custom: &[f64], default: f64) -> f64 {
    custom
        .iter()
        .copied()
        .find(|t| *t > x + EPS)
        .unwrap_or_else(|| next_stop(x, default))
}

/// What the paragraph starting at `from` says, wherever its control sits;
/// with none of its own it keeps what the paragraph before it had.
fn paragraph_at<'a>(
    elems: &'a [Elem],
    from: usize,
    before: Option<&'a ParagraphStyle>,
) -> Option<&'a ParagraphStyle> {
    elems[from..]
        .iter()
        .take_while(|e| !matches!(e, Elem::Hard { .. }))
        .filter_map(|e| match e {
            Elem::Para { style, .. } => Some(style),
            _ => None,
        })
        .last()
        .or(before)
}

/// The left edge of a line of a paragraph.
fn line_start(style: Option<&ParagraphStyle>, first: bool) -> f64 {
    style.map_or(0.0, |s| {
        let indent = s.left_indent + if first { s.first_indent } else { 0.0 };
        indent.max(0.0)
    })
}

fn alignment_of(style: Option<&ParagraphStyle>) -> ParagraphAlignment {
    style.map_or(ParagraphAlignment::Default, |s| s.alignment)
}

fn tabs_of(style: Option<&ParagraphStyle>) -> Vec<f64> {
    let mut tabs: Vec<f64> =
        style.map_or_else(Vec::new, |s| s.tabs.iter().map(|t| t.offset).collect());
    tabs.sort_by(f64::total_cmp);
    tabs
}

fn break_lines(elems: &[Elem], wrap: Option<f64>, tab: f64) -> Vec<Span> {
    let mut spans = Vec::new();
    let (mut start, mut i) = (0, 0);
    let mut style = paragraph_at(elems, 0, None);
    let mut first = true;
    let mut tabs = tabs_of(style);
    let mut x = line_start(style, first);
    let mut x0 = x;
    let mut last_break: Option<usize> = None;
    while i < elems.len() {
        match &elems[i] {
            Elem::Hard { source } => {
                spans.push(Span {
                    elems: start..i,
                    hard: Some(source.clone()),
                    x0,
                    tabs: tabs.clone(),
                    align: alignment_of(style),
                    ends_paragraph: true,
                });
                style = paragraph_at(elems, i + 1, style);
                first = true;
                tabs = tabs_of(style);
                (start, i, last_break) = (i + 1, i + 1, None);
                x = line_start(style, first);
                x0 = x;
            }
            Elem::Para { .. } => i += 1,
            Elem::Tab { .. } => {
                x = tab_after(x, &tabs, tab);
                i += 1;
            }
            Elem::Cluster(c) => {
                let after = x + c.advance;
                let too_wide = wrap.is_some_and(|w| after > w + EPS);
                if let (true, false, Some(b)) = (too_wide, c.space, last_break) {
                    // End the line at the last break and look again from
                    // there. With none, the word runs past the width, as in
                    // AutoCAD, until a break comes.
                    let end = b + 1;
                    spans.push(Span {
                        elems: start..end,
                        hard: None,
                        x0,
                        tabs: tabs.clone(),
                        align: alignment_of(style),
                        ends_paragraph: false,
                    });
                    first = false;
                    (start, i, last_break) = (end, end, None);
                    x = line_start(style, first);
                    x0 = x;
                    continue;
                }
                x = after;
                if c.break_after {
                    last_break = Some(i);
                }
                i += 1;
            }
        }
    }
    spans.push(Span {
        elems: start..elems.len(),
        hard: None,
        x0,
        tabs,
        align: alignment_of(style),
        ends_paragraph: true,
    });
    spans
}

fn state_of(elem: &Elem) -> Option<usize> {
    match elem {
        Elem::Cluster(c) => Some(c.state),
        Elem::Tab { state, .. } => Some(*state),
        Elem::Hard { .. } | Elem::Para { .. } => None,
    }
}

fn source_start(elem: &Elem) -> usize {
    match elem {
        Elem::Cluster(c) => c.source.start,
        Elem::Tab { source, .. } | Elem::Hard { source } => source.start,
        Elem::Para { source, .. } => source.start,
    }
}

pub(super) fn place(
    elems: &[Elem],
    states: &[CharState],
    cx: &Context,
    source_len: usize,
    out: &mut TextLayout,
) {
    let p = &cx.params;
    let tab = (DEFAULT_TAB * p.height).max(EPS);
    let spans = break_lines(elems, p.wrap, tab);
    let mut baseline = 0.0;
    let mut lowest = 0.0_f64;
    let mut last_height = p.height;
    let mut line_start_byte = 0;
    let mut aligned: Vec<Aligned> = Vec::new();

    for (k, span) in spans.iter().enumerate() {
        let line = &elems[span.elems.clone()];
        let height = line
            .iter()
            .filter_map(state_of)
            .map(|s| states[s].height)
            .fold(0.0, f64::max);
        let height = if height > 0.0 { height } else { p.height };
        baseline -= if k == 0 {
            height
        } else {
            match p.spacing_style {
                LineSpacingStyle::AtLeast => p.spacing_factor * LINE_PITCH * height,
                LineSpacingStyle::Exact => p.spacing_factor * LINE_PITCH * p.height,
            }
        };
        last_height = height;

        let first_glyph = out.glyphs.len();
        let first_caret = out.carets.len();
        let first_decoration = out.decorations.len();
        let mut gaps: Vec<Gap> = Vec::new();
        let mut first_visible: Option<f64> = None;
        let mut prev: Option<char> = None;
        let (mut x, mut visible) = (span.x0, 0.0);
        for elem in line {
            match elem {
                Elem::Cluster(c) => {
                    if c.at_start {
                        out.carets.push(CaretStop {
                            x,
                            line: k,
                            source: c.source.start,
                            end: c.source.end,
                            prev,
                            next: Some(c.first),
                        });
                    }
                    prev = Some(c.last);
                    let s = &states[c.state];
                    let scale_y = c.scale_y;
                    let scale_x = scale_y * p.width_factor * s.width;
                    let slant = (p.oblique.to_degrees() + s.oblique + c.slant)
                        .clamp(-MAX_SLANT_DEGREES, MAX_SLANT_DEGREES);
                    if !c.space {
                        for g in &c.glyphs {
                            out.glyphs.push(PositionedGlyph {
                                id: g.id,
                                face: c.face,
                                x: x + g.dx,
                                y: baseline + g.dy,
                                scale_x: scale_x * g.size,
                                scale_y: scale_y * g.size,
                                shear: slant.to_radians().tan(),
                                weight: c.weight,
                                color: s.color,
                                source: c.source.clone(),
                            });
                        }
                    }
                    decorate(out, s, x, x + c.advance, baseline);
                    for bar in &c.bars {
                        out.decorations.push(Decoration {
                            kind: DecorationKind::FractionBar,
                            x0: x + bar.x0,
                            x1: x + bar.x1,
                            y: baseline + bar.y,
                            thickness: bar.thickness,
                            color: s.color,
                        });
                    }
                    if c.unit {
                        // A stack hangs below the line further than its text.
                        lowest = lowest.min(baseline - 0.75 * s.height);
                    }
                    if !c.space && first_visible.is_none() {
                        first_visible = Some(x);
                    }
                    x += c.advance;
                    if c.space {
                        gaps.push(Gap {
                            x,
                            glyph: out.glyphs.len(),
                            caret: out.carets.len(),
                        });
                    } else {
                        visible = x;
                    }
                }
                Elem::Tab { source, .. } => {
                    out.carets.push(CaretStop {
                        x,
                        line: k,
                        source: source.start,
                        end: source.end,
                        prev,
                        next: Some('\t'),
                    });
                    prev = Some('\t');
                    x = tab_after(x, &span.tabs, tab);
                    visible = x;
                }
                Elem::Hard { .. } | Elem::Para { .. } => {}
            }
        }

        let next_first = spans.get(k + 1).and_then(|s| elems.get(s.elems.start));
        let end_byte = match (&span.hard, next_first) {
            (Some(hard), _) => hard.end,
            (None, Some(next)) => source_start(next),
            (None, None) => source_len,
        };
        // Right-to-left runs come out in visual order, so a line's end can
        // fall before its start; their ranges are only approximate.
        let end_byte = end_byte.max(line_start_byte);
        // The caret at the end of the line sits before its paragraph break.
        let end_source = span.hard.as_ref().map_or(end_byte, |h| h.start);
        out.carets.push(CaretStop {
            x,
            line: k,
            source: end_source,
            end: end_source,
            prev,
            next: None,
        });
        out.lines.push(LaidOutLine {
            baseline,
            height,
            carets: first_caret..out.carets.len(),
            width: visible,
            glyphs: first_glyph..out.glyphs.len(),
            source: line_start_byte..end_byte,
        });
        line_start_byte = end_byte;
        out.actual_width = out.actual_width.max(visible);
        if span.align != ParagraphAlignment::Default {
            // Only spaces between words stretch a justified line.
            let from = first_visible.unwrap_or(0.0) + EPS;
            gaps.retain(|g| g.x > from && g.x < visible - EPS);
            aligned.push(Aligned {
                line: k,
                align: span.align,
                justify: !span.ends_paragraph,
                visible,
                gaps,
                glyphs: first_glyph..out.glyphs.len(),
                carets: first_caret..out.carets.len(),
                decorations: first_decoration..out.decorations.len(),
            });
        }
    }
    let target = p.wrap.unwrap_or(out.actual_width);
    for a in &aligned {
        align_line(out, a, target);
    }
    let descent = cx.metrics.descent / cx.metrics.cap_height * last_height;
    out.actual_height = (-baseline + descent).max(-lowest);
}

/// A space's end on a line: what lies after it moves when the line is
/// justified.
struct Gap {
    x: f64,
    glyph: usize,
    caret: usize,
}

/// A line that is not left aligned, to be moved once the width is known.
struct Aligned {
    line: usize,
    align: ParagraphAlignment,
    justify: bool,
    visible: f64,
    gaps: Vec<Gap>,
    glyphs: Range<usize>,
    carets: Range<usize>,
    decorations: Range<usize>,
}

/// Move a line to the right or centre of `target`, or spread its words to
/// fill it.
fn align_line(out: &mut TextLayout, a: &Aligned, target: f64) {
    let room = (target - a.visible).max(0.0);
    let (offset, extra) = match a.align {
        ParagraphAlignment::Right => (room, 0.0),
        ParagraphAlignment::Center => (room / 2.0, 0.0),
        ParagraphAlignment::Justified if a.justify && !a.gaps.is_empty() => {
            (0.0, room / a.gaps.len() as f64)
        }
        _ => return,
    };
    let after = |pass: &dyn Fn(&Gap) -> bool| {
        offset + extra * a.gaps.iter().filter(|g| pass(g)).count() as f64
    };
    for i in a.glyphs.clone() {
        let dx = after(&|g| g.glyph <= i);
        out.glyphs[i].x += dx;
    }
    for j in a.carets.clone() {
        let dx = after(&|g| g.caret <= j);
        out.carets[j].x += dx;
    }
    for d in a.decorations.clone() {
        let (dx0, dx1) = (
            after(&|g| g.x <= out.decorations[d].x0 + EPS),
            after(&|g| g.x <= out.decorations[d].x1 + EPS),
        );
        out.decorations[d].x0 += dx0;
        out.decorations[d].x1 += dx1;
    }
    out.lines[a.line].width += offset + extra * a.gaps.len() as f64;
}

/// Add a line under, over or through `x0..x1`, joining it to one just before.
fn decorate(out: &mut TextLayout, s: &CharState, x0: f64, x1: f64, baseline: f64) {
    let h = s.height;
    let wanted = [
        (s.underline, DecorationKind::Underline, baseline - 0.15 * h),
        (s.overline, DecorationKind::Overline, baseline + 1.2 * h),
        (s.strike, DecorationKind::Strike, baseline + 0.4 * h),
    ];
    for (on, kind, y) in wanted {
        if !on || x1 <= x0 {
            continue;
        }
        let thickness = 0.06 * h;
        let joined = out.decorations.iter_mut().rev().take(3).find(|d| {
            d.kind == kind
                && (d.y - y).abs() < EPS
                && (d.thickness - thickness).abs() < EPS
                && d.color == s.color
                && (d.x1 - x0).abs() < EPS
        });
        match joined {
            Some(d) => d.x1 = x1,
            None => out.decorations.push(Decoration {
                kind,
                x0,
                x1,
                y,
                thickness,
                color: s.color,
            }),
        }
    }
}
