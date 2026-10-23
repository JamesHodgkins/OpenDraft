//! Polylines from DXF: lightweight (`LWPOLYLINE`) and old-style (`POLYLINE`
//! with `VERTEX` records), with their arc bulges and segment widths.

use od_core::{bulge, width, Entity, Point};

use super::{Importer, Record};

/// A polyline vertex and the segment that starts there.
pub(super) struct Vertex {
    at: Point,
    bulge: f64,
    /// Width at the start and end of the segment, `None` for the polyline's
    /// default.
    start: Option<f64>,
    end: Option<f64>,
}

/// The default widths of an old-style polyline, from its header record.
pub(super) fn header_widths(header: &Record) -> (f64, f64) {
    (
        header.number(40).unwrap_or(0.0),
        header.number(41).unwrap_or(0.0),
    )
}

/// A VERTEX record's vertex, or `None` for a point that is not on the curve.
pub(super) fn vertex(record: &Record) -> Option<Vertex> {
    Some(Vertex {
        at: Importer::point(record, 10, 20)?,
        bulge: record.number(42).unwrap_or(0.0),
        start: record.number(40),
        end: record.number(41),
    })
}

/// Vertices from a lightweight polyline's groups. Code 43 is a constant
/// width for every segment that gives none of its own.
pub(super) fn lw_vertices(record: &Record) -> (Vec<Vertex>, f64) {
    let mut vertices: Vec<Vertex> = Vec::new();
    for group in &record.groups {
        let value = group.value.parse::<f64>().ok().filter(|v| v.is_finite());
        match (group.code, value) {
            (10, Some(x)) => vertices.push(Vertex {
                at: Point::new(x, 0.0),
                bulge: 0.0,
                start: None,
                end: None,
            }),
            (20, Some(y)) => {
                if let Some(last) = vertices.last_mut() {
                    last.at.y = y;
                }
            }
            (40, Some(w)) => {
                if let Some(last) = vertices.last_mut() {
                    last.start = Some(w);
                }
            }
            (41, Some(w)) => {
                if let Some(last) = vertices.last_mut() {
                    last.end = Some(w);
                }
            }
            (42, Some(bulge)) => {
                if let Some(last) = vertices.last_mut() {
                    last.bulge = bulge;
                }
            }
            _ => {}
        }
    }
    (vertices, record.number(43).unwrap_or(0.0))
}

impl Importer {
    /// Add a polyline from its vertices. A bulge makes the segment from that
    /// vertex to the next an arc; `default` is the width of a segment that
    /// gives none.
    pub(super) fn polyline(
        &mut self,
        record: &Record,
        vertices: &[Vertex],
        default: (f64, f64),
        closed: bool,
    ) {
        if vertices.len() < 2 {
            self.damaged += 1;
            return;
        }
        let points: Vec<Point> = vertices.iter().map(|v| v.at).collect();
        // A bulge on the last vertex of an open polyline leads nowhere.
        let mut bulges: Vec<f64> = vertices
            .iter()
            .map(|v| if v.bulge.is_finite() { v.bulge } else { 0.0 })
            .collect();
        let mut widths: Vec<(f64, f64)> = vertices
            .iter()
            .map(|v| {
                let wide = |w: Option<f64>, default: f64| w.unwrap_or(default).max(0.0);
                (wide(v.start, default.0), wide(v.end, default.1))
            })
            .collect();
        if !closed {
            if let Some(last) = bulges.last_mut() {
                *last = 0.0;
            }
            if let Some(last) = widths.last_mut() {
                *last = (0.0, 0.0);
            }
        }
        if !bulge::has_arcs(&bulges) {
            bulges.clear();
        }
        if !width::has_widths(&widths) {
            widths.clear();
        }
        let entity = if points.len() == 2 && !closed && bulges.is_empty() && widths.is_empty() {
            Entity::Line {
                start: points[0],
                end: points[1],
            }
        } else {
            Entity::Polyline {
                points,
                closed,
                bulges,
                widths,
            }
        };
        self.add(record, entity);
    }
}
