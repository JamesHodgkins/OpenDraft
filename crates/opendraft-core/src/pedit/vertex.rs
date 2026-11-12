//! Vertex edits.

use super::PolyData;
use crate::{bulge, Point};

impl PolyData {
    /// A vertex at `at` after vertex `i`. On a straight segment the vertex
    /// goes exactly where given; on an arc it goes to the nearest point of
    /// the arc, which splits into two arcs of the same circle. The width
    /// tapers across the two pieces. `None` when it would land on an end of
    /// an arc.
    pub fn with_inserted(&self, i: usize, at: Point) -> Option<PolyData> {
        let n = self.points.len();
        let (a, b) = (self.points[i], self.points[(i + 1) % n]);
        let (mut bulges, mut widths) = (self.full_bulges(), self.full_widths());
        let (near, t) = bulge::nearest(a, b, bulges[i], at);
        let curved = bulges[i] != 0.0;
        if curved && (t <= 0.0 || t >= 1.0) {
            return None;
        }
        let (first, second) = (
            bulge::part(bulges[i], 0.0, t),
            bulge::part(bulges[i], t, 1.0),
        );
        bulges[i] = first;
        bulges.insert(i + 1, second);
        let (w0, w1) = widths[i];
        let mid = w0 + (w1 - w0) * t;
        widths[i] = (w0, mid);
        widths.insert(i + 1, (mid, w1));
        let mut out = self.clone();
        out.points.insert(i + 1, if curved { near } else { at });
        out.bulges = bulges;
        out.widths = widths;
        Some(out)
    }

    /// Vertex `i` moved to `at`.
    pub fn with_moved(&self, i: usize, at: Point) -> PolyData {
        let mut out = self.clone();
        out.points[i] = at;
        out
    }

    /// The vertices between `i` and `j` removed, leaving one straight
    /// segment from `i` to `j`; `None` unless `i < j`.
    pub fn straightened(&self, i: usize, j: usize) -> Option<PolyData> {
        let n = self.points.len();
        if i >= j || j >= n {
            return None;
        }
        let (mut bulges, mut widths) = (self.full_bulges(), self.full_widths());
        let ends = (widths[i].0, widths[j - 1].1);
        bulges[i] = 0.0;
        widths[i] = ends;
        let mut out = self.clone();
        out.points.drain(i + 1..j);
        bulges.drain(i + 1..j);
        widths.drain(i + 1..j);
        out.bulges = bulges;
        out.widths = widths;
        Some(out)
    }

    /// The open stretch from vertex `from` to `to` (inclusive, in order).
    fn slice(&self, from: usize, to: usize) -> PolyData {
        let (bulges, widths) = (self.full_bulges(), self.full_widths());
        let mut piece = PolyData {
            points: self.points[from..=to].to_vec(),
            closed: false,
            bulges: bulges[from..=to].to_vec(),
            widths: widths[from..=to].to_vec(),
        };
        let last = to - from;
        piece.bulges[last] = 0.0;
        piece.widths[last] = (0.0, 0.0);
        piece
    }

    /// A closed polyline as an open one that starts and ends at vertex `at`.
    fn unrolled(&self, at: usize) -> PolyData {
        let n = self.points.len();
        let (bulges, widths) = (self.full_bulges(), self.full_widths());
        let order: Vec<usize> = (0..=n).map(|k| (at + k) % n).collect();
        let last = n;
        let mut piece = PolyData {
            points: order.iter().map(|&k| self.points[k]).collect(),
            closed: false,
            bulges: order.iter().map(|&k| bulges[k]).collect(),
            widths: order.iter().map(|&k| widths[k]).collect(),
        };
        piece.bulges[last] = 0.0;
        piece.widths[last] = (0.0, 0.0);
        piece
    }

    /// What is left when the segments between vertices `a` and `b` are
    /// taken out: up to two open polylines (one when this is closed). The
    /// same vertex twice just cuts there. `None` when nothing would change
    /// (a cut at an end of an open polyline).
    pub fn broken(&self, a: usize, b: usize) -> Option<Vec<PolyData>> {
        let n = self.points.len();
        let (i, j) = (a.min(b), a.max(b));
        if j >= n {
            return None;
        }
        let pieces: Vec<PolyData> = if self.closed {
            let rolled = self.unrolled(j);
            vec![rolled.slice(0, i + n - j)]
        } else {
            let mut kept = Vec::new();
            if i >= 1 {
                kept.push(self.slice(0, i));
            }
            if j + 1 < n {
                kept.push(self.slice(j, n - 1));
            }
            if i == j && kept.len() < 2 {
                return None;
            }
            kept
        };
        Some(pieces)
    }
}
