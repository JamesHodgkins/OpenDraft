//! From parsed nodes to shaped clusters: the character state at each point
//! of the contents, and the glyphs each run of text shapes to.

use std::ops::Range;

use rustybuzz::UnicodeBuffer;

use super::{Context, LayoutDiagnostic, LayoutIssue};
use crate::text::font::{self, FontFace};
use crate::text::fontctl::font_parts;
use crate::text::paragraph::ParagraphStyle;
use crate::text::parse::{Format, NodeKind, ParsedMText};
use crate::text::stack::split_stack;
use crate::text_style::OD_ROMAN_FAMILY;

pub(super) use crate::text::charstate::CharState;

mod stroke;

pub(super) struct ShapedGlyph {
    pub id: u16,
    /// A multiple of the run's size: 1, or the smaller size of a stack.
    pub size: f64,
    /// Offsets from the pen, in drawing units.
    pub dx: f64,
    pub dy: f64,
}

pub(super) struct Cluster {
    pub glyphs: Vec<ShapedGlyph>,
    pub advance: f64,
    /// Only white space, which may hang past the end of a line.
    pub space: bool,
    pub first: char,
    pub last: char,
    pub break_after: bool,
    pub state: usize,
    pub source: Range<usize>,
    /// Whether a caret may sit before it: always for literal text, and for
    /// text made from a control (a stack) only at the control's start.
    pub at_start: bool,
    /// The face it was shaped in, and drawing units per font unit in it.
    pub face: FontFace,
    pub scale_y: f64,
    /// Lines drawn with it (a fraction bar), relative to its origin.
    pub bars: Vec<Bar>,
    /// Stacked text: one unit that is never broken or split.
    pub unit: bool,
    /// For a stroke face, the pen width in thousandths of the cap height
    /// and the slant italic adds in degrees; 0 for an outline face.
    pub weight: u16,
    pub slant: f64,
}

/// A bar across a stack, in drawing units from the cluster's origin.
pub(super) struct Bar {
    pub x0: f64,
    pub x1: f64,
    pub y: f64,
    pub thickness: f64,
}

/// The size of stacked text, as a multiple of the text height.
const STACK_SIZE: f64 = 0.7;

pub(super) enum Elem {
    Cluster(Cluster),
    Tab {
        state: usize,
        source: Range<usize>,
    },
    Hard {
        source: Range<usize>,
    },
    /// A paragraph control: no width, but it sets the paragraph's indents and tabs.
    Para {
        style: ParagraphStyle,
        source: Range<usize>,
    },
}

pub(super) struct Built {
    pub elems: Vec<Elem>,
    pub states: Vec<CharState>,
    pub diagnostics: Vec<LayoutDiagnostic>,
}

fn is_space(c: char) -> bool {
    c.is_whitespace() && c != '\u{a0}'
}

fn is_cjk(c: char) -> bool {
    matches!(u32::from(c),
        0x2E80..=0xD7AF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF | 0x20000..=0x3FFFF)
}

struct Builder<'a> {
    cx: &'a Context,
    elems: Vec<Elem>,
    states: Vec<CharState>,
    diagnostics: Vec<LayoutDiagnostic>,
    stack: Vec<usize>,
}

pub(super) fn build(parsed: &ParsedMText, source: &str, cx: &Context) -> Built {
    let base = CharState {
        bold: cx.params.bold,
        italic: cx.params.italic,
        stroke: cx.params.stroke,
        ..CharState::plain(cx.params.height)
    };
    let mut b = Builder {
        cx,
        elems: Vec::new(),
        states: vec![base],
        diagnostics: Vec::new(),
        stack: Vec::new(),
    };
    let mut state = 0;
    for node in &parsed.nodes {
        let range = node.source.clone();
        let text = &source[range.clone()];
        match &node.kind {
            NodeKind::Text => b.segment(text, range, true, state),
            NodeKind::EscapedChar => b.segment(&text[1..], range, false, state),
            NodeKind::NonBreakingSpace => b.segment("\u{a0}", range, false, state),
            NodeKind::Unicode(c) => b.segment(c.encode_utf8(&mut [0; 4]), range, false, state),
            NodeKind::Tab => b.elems.push(Elem::Tab {
                state,
                source: range,
            }),
            NodeKind::ParagraphBreak => b.elems.push(Elem::Hard { source: range }),
            NodeKind::Paragraph(style) => b.elems.push(Elem::Para {
                style: style.clone(),
                source: range,
            }),
            NodeKind::Stack => b.stack(text, range, state),
            NodeKind::Field => b.issue(range, LayoutIssue::FieldNotEvaluated),
            NodeKind::Opaque(reason) => b.issue(range, LayoutIssue::Skipped(*reason)),
            NodeKind::Push => b.stack.push(state),
            NodeKind::Pop => state = b.stack.pop().unwrap_or(state),
            NodeKind::Format(format) => {
                let family = font_parts(text).family;
                let ours = family.eq_ignore_ascii_case("DejaVu Sans")
                    || family.eq_ignore_ascii_case(OD_ROMAN_FAMILY);
                if matches!(format, Format::Font(_)) && !ours {
                    b.issue(range.clone(), LayoutIssue::FontSubstituted);
                }
                if let Some(mut next) = b.states[state].after(*format) {
                    if matches!(format, Format::Font(_)) && ours {
                        next.stroke = family.eq_ignore_ascii_case(OD_ROMAN_FAMILY);
                    }
                    b.states.push(next);
                    state = b.states.len() - 1;
                }
            }
        }
    }
    b.mark_breaks();
    Built {
        elems: b.elems,
        states: b.states,
        diagnostics: b.diagnostics,
    }
}

impl Builder<'_> {
    fn issue(&mut self, range: Range<usize>, issue: LayoutIssue) {
        self.diagnostics.push(LayoutDiagnostic { range, issue });
    }

    /// Shape `text` and add its clusters. For literal text, byte `i` of it
    /// is byte `source.start + i` of the contents; otherwise a cluster's
    /// source is the whole node.
    fn segment(&mut self, text: &str, source: Range<usize>, literal: bool, state: usize) {
        if text.is_empty() {
            return;
        }
        let cx = self.cx;
        let s = self.states[state];
        if let Some(look) = stroke::look(cx, &s) {
            self.segment_stroke(&look, text, source, literal, state);
            return;
        }
        let face = FontFace::new(s.bold, s.italic);
        let (Some(hb), Some(metrics)) = (font::face_of(face), font::metrics_of(face)) else {
            return;
        };
        let scale = s.height / metrics.cap_height;
        let across = scale * cx.params.width_factor * s.width;
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let shaped = rustybuzz::shape(hb, &[], buffer);
        let (infos, positions) = (shaped.glyph_infos(), shaped.glyph_positions());

        let mut starts: Vec<usize> = infos.iter().map(|i| i.cluster as usize).collect();
        starts.sort_unstable();
        starts.dedup();
        let mut i = 0;
        while i < infos.len() {
            let start = infos[i].cluster as usize;
            let mut cluster = Cluster {
                glyphs: Vec::new(),
                advance: 0.0,
                space: false,
                first: ' ',
                last: ' ',
                break_after: false,
                state,
                source: source.clone(),
                at_start: true,
                face,
                scale_y: scale,
                bars: Vec::new(),
                unit: false,
                weight: 0,
                slant: 0.0,
            };
            while i < infos.len() && infos[i].cluster as usize == start {
                let (info, pos) = (&infos[i], &positions[i]);
                if info.glyph_id == 0 {
                    self.issue(source.clone(), LayoutIssue::MissingGlyph);
                }
                cluster.glyphs.push(ShapedGlyph {
                    id: u16::try_from(info.glyph_id).unwrap_or(0),
                    size: 1.0,
                    dx: f64::from(pos.x_offset) * across,
                    dy: f64::from(pos.y_offset) * scale,
                });
                cluster.advance += f64::from(pos.x_advance) * across;
                i += 1;
            }
            // Tracking adds its space once, however many glyphs make the cluster.
            cluster.advance = (cluster.advance + s.tracking_extra()).max(0.0);
            let at = starts.partition_point(|&v| v <= start);
            let end = starts.get(at).copied().unwrap_or(text.len());
            let piece = text.get(start..end).unwrap_or("");
            cluster.first = piece.chars().next().unwrap_or(' ');
            cluster.last = piece.chars().next_back().unwrap_or(' ');
            cluster.space = !piece.is_empty() && piece.chars().all(is_space);
            if literal {
                cluster.source = source.start + start..source.start + end;
            } else {
                cluster.at_start = start == 0;
            }
            self.elems.push(Elem::Cluster(cluster));
        }
    }

    /// Shape `text` as one run at `size` times the text height: its glyphs
    /// with their pen offsets, and the run's advance.
    fn shape_flat(&self, text: &str, state: usize, size: f64) -> (Vec<ShapedGlyph>, f64) {
        let s = self.states[state];
        if let Some(look) = stroke::look(self.cx, &s) {
            return self.shape_flat_stroke(&look, text, state, size);
        }
        let face = FontFace::new(s.bold, s.italic);
        let (Some(hb), Some(metrics)) = (font::face_of(face), font::metrics_of(face)) else {
            return (Vec::new(), 0.0);
        };
        if text.is_empty() {
            return (Vec::new(), 0.0);
        }
        let scale = s.height / metrics.cap_height;
        let across = scale * self.cx.params.width_factor * s.width * size;
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let shaped = rustybuzz::shape(hb, &[], buffer);
        let mut pen = 0.0;
        let mut glyphs = Vec::new();
        for (info, pos) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
            glyphs.push(ShapedGlyph {
                id: u16::try_from(info.glyph_id).unwrap_or(0),
                size,
                dx: pen + f64::from(pos.x_offset) * across,
                dy: f64::from(pos.y_offset) * scale * size,
            });
            pen += (f64::from(pos.x_advance) * across + s.tracking_extra() * size).max(0.0);
        }
        (glyphs, pen)
    }

    /// A stack: the numerator over the denominator at a smaller size, with
    /// a bar for `/`, none for `^`, and slanted for `#`. It is one cluster,
    /// so it wraps, is selected and is deleted as a unit.
    fn stack(&mut self, text: &str, range: Range<usize>, state: usize) {
        let payload = text.get(2..text.len().saturating_sub(1)).unwrap_or("");
        let parts = split_stack(payload);
        let Some(divider) = parts.divider else {
            self.segment(&parts.numerator, range, false, state);
            return;
        };
        let s = self.states[state];
        let look = stroke::look(self.cx, &s);
        let face = look
            .as_ref()
            .map_or(FontFace::new(s.bold, s.italic), |_| FontFace::OdRoman);
        let cap_height = match &look {
            Some(l) => l.font.metrics.cap_height,
            None => match font::metrics_of(face) {
                Some(m) => m.cap_height,
                None => return,
            },
        };
        let h = s.height;
        let (num, nw) = self.shape_flat(&parts.numerator, state, STACK_SIZE);
        let (den, dw) = self.shape_flat(&parts.denominator, state, STACK_SIZE);
        let (mut glyphs, mut bars) = (Vec::new(), Vec::new());
        let mut place = |run: Vec<ShapedGlyph>, dx: f64, dy: f64| {
            glyphs.extend(run.into_iter().map(|g| ShapedGlyph {
                dx: g.dx + dx,
                dy: g.dy + dy,
                ..g
            }));
        };
        let width = if divider == '#' {
            let (slash, sw) = self.shape_flat("/", state, 1.0);
            place(num, 0.0, 0.35 * h);
            place(slash, nw, 0.0);
            place(den, nw + sw, -0.25 * h);
            nw + sw + dw
        } else {
            let width = nw.max(dw) + 0.16 * h;
            let (up, down) = if divider == '/' {
                (0.40 * h, -0.45 * h)
            } else {
                (0.45 * h, -0.35 * h)
            };
            place(num, (width - nw) / 2.0, up);
            place(den, (width - dw) / 2.0, down);
            if divider == '/' {
                bars.push(Bar {
                    x0: 0.0,
                    x1: width,
                    y: 0.32 * h,
                    thickness: 0.05 * h,
                });
            }
            width
        };
        self.elems.push(Elem::Cluster(Cluster {
            glyphs,
            advance: width,
            space: false,
            first: parts.numerator.chars().next().unwrap_or(' '),
            last: parts.denominator.chars().next_back().unwrap_or(' '),
            break_after: false,
            state,
            source: range,
            at_start: true,
            face,
            scale_y: h / cap_height,
            bars,
            unit: true,
            weight: look.as_ref().map_or(0, |l| l.weight),
            slant: look.as_ref().map_or(0.0, |l| l.slant),
        }));
    }

    /// Where a line may end: after white space and hyphens, and around
    /// ideographs.
    fn mark_breaks(&mut self) {
        let firsts: Vec<Option<char>> = self
            .elems
            .iter()
            .skip(1)
            .map(|e| match e {
                Elem::Cluster(c) => Some(c.first),
                _ => None,
            })
            .chain(std::iter::once(None))
            .collect();
        for (elem, next) in self.elems.iter_mut().zip(firsts) {
            if let Elem::Cluster(c) = elem {
                c.break_after = !c.unit
                    && (is_space(c.last)
                        || c.last == '-'
                        || is_cjk(c.last)
                        || next.is_some_and(is_cjk));
            }
        }
    }
}
