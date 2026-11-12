//! Moving a caret about a laid-out text, and the shapes of a selection.
//!
//! A caret is an index into [`TextLayout::carets`]. Indexes are only good for
//! one layout: an editor keeps a byte offset in the contents (plus which side
//! of a wrap it is on) and finds the index again after every edit.

use super::layout::TextLayout;

/// The box a selection paints on one line, in layout coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionRect {
    pub x0: f64,
    pub x1: f64,
    pub bottom: f64,
    pub top: f64,
}

/// How far below the baseline and above it (in text heights) a caret reaches.
const CARET_BELOW: f64 = 0.28;
const CARET_ABOVE: f64 = 1.1;
/// How wide (in text heights) the mark for a paragraph break at a line's end is.
const BREAK_MARK: f64 = 0.3;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Space,
    Word,
    Other,
    LineEdge,
}

fn class(c: Option<char>) -> Class {
    match c {
        None => Class::LineEdge,
        Some(c) if c.is_whitespace() => Class::Space,
        Some(c) if c.is_alphanumeric() || c == '_' => Class::Word,
        Some(_) => Class::Other,
    }
}

impl TextLayout {
    /// The caret at byte `source` of the contents. Where a wrap puts two
    /// there, `upstream` picks the end of the earlier line. A byte with no
    /// caret of its own (inside a control) gets the one it stands next to:
    /// after the text before it, else before the text after it.
    pub fn caret_at(&self, source: usize, upstream: bool) -> Option<usize> {
        let mut found = self
            .carets
            .iter()
            .enumerate()
            .filter(|(_, c)| c.source == source)
            .map(|(i, _)| i);
        let first = found.next();
        if let Some(first) = first {
            return Some(if upstream {
                first
            } else {
                found.next_back().unwrap_or(first)
            });
        }
        let before = self
            .carets
            .iter()
            .enumerate()
            .filter(|(_, c)| c.source < source)
            .max_by_key(|(i, c)| (c.source, *i))
            .map(|(i, c)| {
                // Past the end of what that stop covers (a closing brace or
                // a control after it), the caret is the one after it, not
                // the one before it.
                if c.end <= source && i + 1 < self.carets.len() {
                    i + 1
                } else {
                    i
                }
            });
        before.or(if self.carets.is_empty() {
            None
        } else {
            Some(0)
        })
    }

    /// Whether caret `index` is the end of a wrapped line (so, upstream of the
    /// start of the next one at the same byte).
    pub fn is_upstream(&self, index: usize) -> bool {
        let Some(stop) = self.carets.get(index) else {
            return false;
        };
        self.carets
            .get(index + 1)
            .is_some_and(|next| next.source == stop.source && next.line != stop.line)
    }

    /// Where caret `index` stands: `(x, bottom, top)` in layout coordinates.
    pub fn caret_rect(&self, index: usize) -> Option<(f64, f64, f64)> {
        let stop = self.carets.get(index)?;
        let line = self.lines.get(stop.line)?;
        Some((
            stop.x,
            line.baseline - CARET_BELOW * line.height,
            line.baseline + CARET_ABOVE * line.height,
        ))
    }

    /// The caret nearest the layout point `(x, y)`: the line whose middle is
    /// nearest vertically, then the stop nearest across.
    pub fn nearest_caret(&self, x: f64, y: f64) -> Option<usize> {
        let line = self
            .lines
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let d = |l: &super::layout::LaidOutLine| (y - (l.baseline + 0.35 * l.height)).abs();
                d(a).total_cmp(&d(b))
            })
            .map(|(i, _)| i)?;
        self.carets[self.lines[line].carets.clone()]
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (a.x - x).abs().total_cmp(&(b.x - x).abs()))
            .map(|(i, _)| self.lines[line].carets.start + i)
    }

    /// One caret to the right or left, skipping the twin a wrap makes.
    pub fn step(&self, index: usize, forward: bool) -> usize {
        let here = self.carets.get(index).map(|c| c.source);
        let mut at = index;
        loop {
            let next = if forward {
                at + 1
            } else {
                match at.checked_sub(1) {
                    Some(n) => n,
                    None => return index,
                }
            };
            if next >= self.carets.len() {
                return index;
            }
            at = next;
            if Some(self.carets[at].source) != here {
                return at;
            }
        }
    }

    /// To the start of the next word, or the start of the one before.
    pub fn word_step(&self, index: usize, forward: bool) -> usize {
        let last = self.carets.len().saturating_sub(1);
        let mut at = index;
        if forward {
            let first = class(self.carets.get(at).and_then(|c| c.next));
            if first == Class::LineEdge {
                return self.step(index, true);
            }
            while at < last && first != Class::Space && class(self.carets[at].next) == first {
                at += 1;
            }
            while at < last && class(self.carets[at].next) == Class::Space {
                at += 1;
            }
        } else {
            while at > 0 && class(self.carets[at].prev) == Class::Space {
                at -= 1;
            }
            let first = class(self.carets.get(at).and_then(|c| c.prev));
            if first == Class::LineEdge {
                return self.step(index, false);
            }
            while at > 0 && class(self.carets[at].prev) == first {
                at -= 1;
            }
        }
        at
    }

    /// The carets around the word at caret `index`: the run of one kind of
    /// character (letters and digits, white space, or other) on either side.
    /// A caret between two kinds takes the one after it.
    pub fn word_at(&self, index: usize) -> (usize, usize) {
        let Some(stop) = self.carets.get(index) else {
            return (index, index);
        };
        let kind = match class(stop.next) {
            Class::LineEdge => class(stop.prev),
            k => k,
        };
        if kind == Class::LineEdge {
            return (index, index);
        }
        let (mut from, mut to) = (index, index);
        while from > self.line_start(index) && class(self.carets[from].prev) == kind {
            from -= 1;
        }
        while to < self.line_end(index) && class(self.carets[to].next) == kind {
            to += 1;
        }
        (from, to)
    }

    /// The first caret of the line caret `index` is on.
    pub fn line_start(&self, index: usize) -> usize {
        self.carets
            .get(index)
            .map_or(index, |c| self.lines[c.line].carets.start)
    }

    /// The last caret of the line caret `index` is on.
    pub fn line_end(&self, index: usize) -> usize {
        self.carets
            .get(index)
            .map_or(index, |c| self.lines[c.line].carets.end - 1)
    }

    /// The caret on the line above or below, nearest `across`; `None` on the
    /// first or last line.
    pub fn vertical(&self, index: usize, across: f64, up: bool) -> Option<usize> {
        let line = self.carets.get(index)?.line;
        let target = if up {
            line.checked_sub(1)?
        } else {
            let next = line + 1;
            (next < self.lines.len()).then_some(next)?
        };
        let range = self.lines[target].carets.clone();
        self.carets[range.clone()]
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (a.x - across).abs().total_cmp(&(b.x - across).abs()))
            .map(|(i, _)| range.start + i)
    }

    /// The boxes a selection from caret `from` to caret `to` paints, one per
    /// line it touches. A line it runs off the end of gets a little extra, to
    /// show the paragraph break is selected.
    pub fn selection_rects(&self, from: usize, to: usize) -> Vec<SelectionRect> {
        let (from, to) = (from.min(to), from.max(to));
        let mut rects = Vec::new();
        if from == to {
            return rects;
        }
        for (k, line) in self.lines.iter().enumerate() {
            let range = &line.carets;
            let (lo, hi) = (from.max(range.start), to.min(range.end - 1));
            if lo > hi {
                continue;
            }
            let (x0, mut x1) = (self.carets[lo].x, self.carets[hi].x);
            // Only a paragraph break is marked, not a wrap (whose next line starts
            // at the very byte this one ends at).
            let breaks = self.lines.get(k + 1).is_some_and(|next| {
                self.carets[range.end - 1].source != self.carets[next.carets.start].source
            });
            let runs_on = to > range.end - 1 && breaks;
            if runs_on && hi == range.end - 1 {
                x1 += BREAK_MARK * line.height;
            }
            if x1 <= x0 {
                continue;
            }
            rects.push(SelectionRect {
                x0,
                x1,
                bottom: line.baseline - CARET_BELOW * line.height,
                top: line.baseline + CARET_ABOVE * line.height,
            });
        }
        rects
    }
}
