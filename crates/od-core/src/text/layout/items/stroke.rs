//! Shaping in OD Roman: one cluster per character, its advance read from the
//! font (never from the pen weight, so weight does not reflow text). A
//! character the font lacks is drawn in the bundled font instead, and
//! reported, so nothing is dropped without a word.

use std::ops::Range;

use super::{is_space, Builder, CharState, Cluster, Context, Elem, ShapedGlyph};
use crate::text::font::{self, FontFace};
use crate::text::layout::LayoutIssue;
use crate::text::stroke_font::{od_roman, StrokeFont};
use crate::text::{MAX_WEIGHT, MIN_WEIGHT};

/// How a run in OD Roman is drawn.
pub(super) struct Look {
    pub font: &'static StrokeFont,
    /// Pen width in thousandths of the cap height.
    pub weight: u16,
    /// Slant added by italic, in degrees.
    pub slant: f64,
}

/// The stroke look of `state`, or `None` when it is drawn in the bundled
/// font (or the stroke font is unavailable).
pub(super) fn look(cx: &Context, state: &CharState) -> Option<Look> {
    if !state.stroke {
        return None;
    }
    let font = od_roman()?;
    let mut weight = state
        .weight
        .map_or(cx.params.weight, |w| f64::from(w) / 1000.0);
    if state.bold {
        weight *= font.metrics.bold_factor;
    }
    Some(Look {
        font,
        weight: (weight.clamp(MIN_WEIGHT, MAX_WEIGHT) * 1000.0).round() as u16,
        slant: if state.italic {
            font.metrics.italic_degrees
        } else {
            0.0
        },
    })
}

impl Builder<'_> {
    /// Shape `text` as OD Roman. Sources are as for `segment`.
    pub(super) fn segment_stroke(
        &mut self,
        look: &Look,
        text: &str,
        source: Range<usize>,
        literal: bool,
        state: usize,
    ) {
        let s = self.states[state];
        let scale = s.height / look.font.metrics.cap_height;
        let across = scale * self.cx.params.width_factor * s.width;
        for (start, ch) in text.char_indices() {
            let end = start + ch.len_utf8();
            let mut cluster = Cluster {
                glyphs: Vec::new(),
                advance: 0.0,
                space: is_space(ch),
                first: ch,
                last: ch,
                break_after: false,
                state,
                source: source.clone(),
                at_start: true,
                face: FontFace::OdRoman,
                scale_y: scale,
                bars: Vec::new(),
                unit: false,
                weight: look.weight,
                slant: look.slant,
            };
            if literal {
                cluster.source = source.start + start..source.start + end;
            } else {
                cluster.at_start = start == 0;
            }
            let wanted = if ch == '\u{a0}' { ' ' } else { ch };
            match look
                .font
                .slot_of(wanted)
                .and_then(|i| Some((i, look.font.glyph(i)?)))
            {
                Some((slot, glyph)) => {
                    cluster.advance = glyph.advance * across * s.tracking;
                    if !glyph.strokes.is_empty() {
                        cluster.glyphs.push(ShapedGlyph {
                            id: slot,
                            size: 1.0,
                            dx: 0.0,
                            dy: 0.0,
                        });
                    }
                }
                None => self.fallback(&mut cluster, ch, &s),
            }
            self.elems.push(Elem::Cluster(cluster));
        }
    }

    /// `ch` in the bundled font, for a character OD Roman has no glyph for.
    fn fallback(&mut self, cluster: &mut Cluster, ch: char, s: &CharState) {
        let face = FontFace::new(s.bold, s.italic);
        let (Some(hb), Some(metrics)) = (font::face_of(face), font::metrics_of(face)) else {
            return;
        };
        let glyph = hb.glyph_index(ch);
        if glyph.is_none() {
            self.issue(cluster.source.clone(), LayoutIssue::MissingGlyph);
        }
        let id = glyph.map_or(0, |g| g.0);
        let scale = s.height / metrics.cap_height;
        let across = scale * self.cx.params.width_factor * s.width;
        cluster.face = face;
        cluster.scale_y = scale;
        cluster.weight = 0;
        cluster.slant = 0.0;
        cluster.advance = f64::from(
            hb.glyph_hor_advance(rustybuzz::ttf_parser::GlyphId(id))
                .unwrap_or(0),
        ) * across
            * s.tracking;
        if !cluster.space {
            cluster.glyphs.push(ShapedGlyph {
                id,
                size: 1.0,
                dx: 0.0,
                dy: 0.0,
            });
        }
    }

    /// OD Roman `text` as one run at `size` times the text height, for the
    /// parts of a stack: glyphs with pen offsets and the run's advance. A
    /// character the font lacks is left out.
    pub(super) fn shape_flat_stroke(
        &self,
        look: &Look,
        text: &str,
        state: usize,
        size: f64,
    ) -> (Vec<ShapedGlyph>, f64) {
        let s = self.states[state];
        let scale = s.height / look.font.metrics.cap_height;
        let across = scale * self.cx.params.width_factor * s.width * size;
        let mut pen = 0.0;
        let mut glyphs = Vec::new();
        for ch in text.chars() {
            let Some((slot, glyph)) = look
                .font
                .slot_of(ch)
                .and_then(|i| Some((i, look.font.glyph(i)?)))
            else {
                continue;
            };
            if !glyph.strokes.is_empty() {
                glyphs.push(ShapedGlyph {
                    id: slot,
                    size,
                    dx: pen,
                    dy: 0.0,
                });
            }
            pen += glyph.advance * across * s.tracking;
        }
        (glyphs, pen)
    }
}
