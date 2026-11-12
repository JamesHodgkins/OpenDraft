//! The edits PEDIT makes to a polyline: close and open, join, width, reverse,
//! straighten curves, and the vertex edits (insert, move, straighten, break,
//! segment width). Pure functions on [`PolyData`], the four lists of a
//! polyline; the command only asks.
//!
//! Lines, arcs and rectangles convert to a polyline first, as in AutoCAD.

use std::f64::consts::TAU;

use crate::{arc, bulge, width, Entity, Point};

mod join;
mod vertex;

pub use join::{JoinType, MAX_JOIN_CANDIDATES};

/// A polyline's points, closed flag, bulges and widths (each of the last two
/// empty or one per point; see [`Entity::Polyline`]).
#[derive(Clone, Debug, PartialEq)]
pub struct PolyData {
    pub points: Vec<Point>,
    pub closed: bool,
    pub bulges: Vec<f64>,
    pub widths: Vec<(f64, f64)>,
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

impl PolyData {
    /// The object as polyline data: a polyline itself, a line, an arc (not a
    /// full circle) or a rectangle.
    pub fn of(entity: &Entity) -> Option<PolyData> {
        match entity {
            Entity::Polyline {
                points,
                closed,
                bulges,
                widths,
            } if points.len() >= 2 => Some(PolyData {
                points: points.clone(),
                closed: *closed,
                bulges: bulges.clone(),
                widths: widths.clone(),
            }),
            Entity::Line { start, end } if distance(*start, *end) > 0.0 => Some(PolyData {
                points: vec![*start, *end],
                closed: false,
                bulges: Vec::new(),
                widths: Vec::new(),
            }),
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => {
                let sweep = arc::sweep(*start, *end);
                (sweep < TAU - 1e-9 && *radius > 0.0).then(|| PolyData {
                    points: vec![
                        arc::point_at(*center, *radius, *start),
                        arc::point_at(*center, *radius, *end),
                    ],
                    closed: false,
                    bulges: vec![bulge::from_sweep(sweep, true), 0.0],
                    widths: Vec::new(),
                })
            }
            Entity::Rectangle { min, max } => Some(PolyData {
                points: vec![
                    *min,
                    Point::new(max.x, min.y),
                    *max,
                    Point::new(min.x, max.y),
                ],
                closed: true,
                bulges: Vec::new(),
                widths: Vec::new(),
            }),
            _ => None,
        }
    }

    /// The polyline entity, with the bulge and width lists dropped when
    /// nothing in them is curved or wide.
    pub fn into_entity(mut self) -> Entity {
        if !bulge::has_arcs(&self.bulges) {
            self.bulges.clear();
        }
        if !width::has_widths(&self.widths) {
            self.widths.clear();
        }
        Entity::Polyline {
            points: self.points,
            closed: self.closed,
            bulges: self.bulges,
            widths: self.widths,
        }
    }

    fn full_bulges(&self) -> Vec<f64> {
        (0..self.points.len())
            .map(|i| bulge::at(&self.bulges, i))
            .collect()
    }

    fn full_widths(&self) -> Vec<(f64, f64)> {
        (0..self.points.len())
            .map(|i| width::at(&self.widths, i))
            .collect()
    }

    /// Joined to its first point (or open again). The closing segment starts
    /// straight and hairline.
    pub fn with_closed(&self, closed: bool) -> PolyData {
        let mut out = self.clone();
        if closed == self.closed {
            return out;
        }
        out.closed = closed;
        let n = out.points.len();
        if !out.bulges.is_empty() {
            out.bulges[n - 1] = 0.0;
        }
        if !out.widths.is_empty() {
            out.widths[n - 1] = (0.0, 0.0);
        }
        out
    }

    /// Every segment `w` wide (a hairline for 0).
    pub fn with_width(&self, w: f64) -> PolyData {
        let mut out = self.clone();
        out.widths = vec![(w, w); self.points.len()];
        out
    }

    /// Segment `i` widening from `start` to `end`.
    pub fn with_segment_width(&self, i: usize, start: f64, end: f64) -> PolyData {
        let mut out = self.clone();
        out.widths = self.full_widths();
        if let Some(slot) = out.widths.get_mut(i) {
            *slot = (start, end);
        }
        out
    }

    /// Every segment straight.
    pub fn decurved(&self) -> PolyData {
        PolyData {
            bulges: Vec::new(),
            ..self.clone()
        }
    }

    /// The same path the other way round: curves turn the other way and
    /// widths swap ends.
    pub fn reversed(&self) -> PolyData {
        let n = self.points.len();
        let (bulges, widths) = (self.full_bulges(), self.full_widths());
        let old = |k: usize| (n + n - 2 - k) % n;
        let mut out = PolyData {
            points: self.points.iter().rev().copied().collect(),
            closed: self.closed,
            bulges: (0..n).map(|k| -bulges[old(k)]).collect(),
            widths: (0..n)
                .map(|k| {
                    let (a, b) = widths[old(k)];
                    (b, a)
                })
                .collect(),
        };
        if !self.closed {
            out.bulges[n - 1] = 0.0;
            out.widths[n - 1] = (0.0, 0.0);
        }
        if self.bulges.is_empty() {
            out.bulges.clear();
        }
        if self.widths.is_empty() {
            out.widths.clear();
        }
        out
    }
}

#[cfg(test)]
mod tests;
