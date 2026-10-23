//! The `AcDbHatch` part of a HATCH entity, in the order AutoCAD writes it.
//!
//! Counts are those of the vectors written, never numbers carried from a
//! file. Source objects (group 330) are not written: this writer does not
//! know the handles the objects will have, so a hatch is written
//! non-associative. Angles are degrees, except a gradient's rotation. A
//! clockwise arc or ellipse edge is written with its angles mirrored, as
//! `read/hatch.rs` reads them.

use std::f64::consts::TAU;

use od_core::hatch::{
    HatchBoundary, HatchEdge, HatchEntity, HatchFill, HatchGradient, HatchLoopFlags, HatchSpline,
};
use od_core::Color;

use crate::dxf::write::{aci_for, finite, Out};
use crate::FileError;

/// Degrees in `[0, 360]` for an angle; a clockwise edge's are mirrored.
fn degrees(angle: f64, clockwise: bool) -> Result<f64, FileError> {
    let a = if clockwise { TAU - angle } else { angle };
    finite(a.to_degrees())
}

fn edge(out: &mut Out, e: &HatchEdge) -> Result<(), FileError> {
    match e {
        HatchEdge::Line { start, end } => {
            out.pair(72, 1);
            out.pair(10, finite(start.x)?);
            out.pair(20, finite(start.y)?);
            out.pair(11, finite(end.x)?);
            out.pair(21, finite(end.y)?);
        }
        HatchEdge::CircularArc {
            center,
            radius,
            start_angle,
            end_angle,
            counter_clockwise,
        } => {
            out.pair(72, 2);
            out.pair(10, finite(center.x)?);
            out.pair(20, finite(center.y)?);
            out.pair(40, finite(*radius)?);
            out.pair(50, degrees(*start_angle, !counter_clockwise)?);
            out.pair(51, degrees(*end_angle, !counter_clockwise)?);
            out.pair(73, i32::from(*counter_clockwise));
        }
        HatchEdge::EllipticArc {
            center,
            major_axis,
            minor_major_ratio,
            start_param,
            end_param,
            counter_clockwise,
        } => {
            out.pair(72, 3);
            out.pair(10, finite(center.x)?);
            out.pair(20, finite(center.y)?);
            out.pair(11, finite(major_axis.x)?);
            out.pair(21, finite(major_axis.y)?);
            out.pair(40, finite(*minor_major_ratio)?);
            out.pair(50, degrees(*start_param, !counter_clockwise)?);
            out.pair(51, degrees(*end_param, !counter_clockwise)?);
            out.pair(73, i32::from(*counter_clockwise));
        }
        HatchEdge::Spline(s) => spline(out, s)?,
    }
    Ok(())
}

fn spline(out: &mut Out, s: &HatchSpline) -> Result<(), FileError> {
    let rational = s.weights.iter().any(|w| *w != 1.0);
    out.pair(72, 4);
    out.pair(94, s.degree);
    out.pair(73, i32::from(rational));
    out.pair(74, i32::from(s.periodic));
    out.pair(95, s.knots.len());
    out.pair(96, s.control_points.len());
    for k in &s.knots {
        out.pair(40, finite(*k)?);
    }
    for (i, q) in s.control_points.iter().enumerate() {
        out.pair(10, finite(q.x)?);
        out.pair(20, finite(q.y)?);
        if rational {
            out.pair(42, finite(s.weights.get(i).copied().unwrap_or(1.0))?);
        }
    }
    out.pair(97, s.fit_points.len());
    for q in &s.fit_points {
        out.pair(11, finite(q.x)?);
        out.pair(21, finite(q.y)?);
    }
    if let Some(t) = s.start_tangent {
        out.pair(12, finite(t.x)?);
        out.pair(22, finite(t.y)?);
    }
    if let Some(t) = s.end_tangent {
        out.pair(13, finite(t.x)?);
        out.pair(23, finite(t.y)?);
    }
    Ok(())
}

fn gradient_tail(out: &mut Out, g: &HatchGradient) -> Result<(), FileError> {
    out.pair(450, 1);
    out.pair(451, 0);
    out.pair(460, finite(g.angle)?);
    out.pair(461, finite(g.shift)?);
    out.pair(452, i32::from(g.one_color));
    out.pair(462, finite(g.tint)?);
    out.pair(453, g.colors.len());
    for (position, [r, gr, b]) in &g.colors {
        out.pair(463, finite(*position)?);
        out.pair(63, aci_for(Color::Rgb(*r, *gr, *b)).0);
        out.pair(
            421,
            (u32::from(*r) << 16) | (u32::from(*gr) << 8) | u32::from(*b),
        );
    }
    out.pair(470, &g.name);
    Ok(())
}

/// The record after the common entity groups.
pub(super) fn hatch(out: &mut Out, h: &HatchEntity) -> Result<(), FileError> {
    h.validate()
        .map_err(|why| FileError::Invalid(format!("a hatch is not valid: {why}")))?;
    let (name, solid) = match &h.fill {
        HatchFill::Pattern(p) => (p.name.as_str(), 0),
        HatchFill::Solid => ("SOLID", 1),
        HatchFill::Gradient(_) => ("SOLID", 1),
    };
    out.pair(100, "AcDbHatch");
    out.point3(10, 0.0, 0.0, finite(h.elevation)?);
    out.point3(210, 0.0, 0.0, 1.0);
    out.pair(2, name);
    out.pair(70, solid);
    out.pair(71, 0);
    out.pair(91, h.loops.len());
    for l in &h.loops {
        let polyline = matches!(l.boundary, HatchBoundary::Polyline(_));
        out.pair(92, l.flags.with(HatchLoopFlags::POLYLINE, polyline).0);
        match &l.boundary {
            HatchBoundary::Polyline(p) => {
                out.pair(72, i32::from(!p.bulges.is_empty()));
                out.pair(73, i32::from(p.closed));
                out.pair(93, p.vertices.len());
                for (i, v) in p.vertices.iter().enumerate() {
                    out.pair(10, finite(v.x)?);
                    out.pair(20, finite(v.y)?);
                    if let Some(b) = p.bulges.get(i) {
                        out.pair(42, finite(*b)?);
                    }
                }
            }
            HatchBoundary::Edges(edges) => {
                out.pair(93, edges.len());
                for e in edges {
                    edge(out, e)?;
                }
            }
        }
        out.pair(97, 0);
    }
    out.pair(75, h.style.code());
    let kind = match &h.fill {
        HatchFill::Pattern(p) => p.pattern_type.code(),
        _ => 1,
    };
    out.pair(76, kind);
    if let HatchFill::Pattern(p) = &h.fill {
        out.pair(52, finite(p.angle.to_degrees().rem_euclid(360.0))?);
        out.pair(41, finite(p.scale_or_spacing)?);
        out.pair(77, i32::from(p.double));
        out.pair(78, p.lines.len());
        for l in &p.lines {
            out.pair(53, finite(l.angle.to_degrees().rem_euclid(360.0))?);
            out.pair(43, finite(l.base.x)?);
            out.pair(44, finite(l.base.y)?);
            out.pair(45, finite(l.offset[0])?);
            out.pair(46, finite(l.offset[1])?);
            out.pair(79, l.dashes.len());
            for d in &l.dashes {
                out.pair(49, finite(*d)?);
            }
        }
    }
    if let Some(pixel) = h.source_pixel_size {
        out.pair(47, finite(pixel)?);
    }
    out.pair(98, h.seed_points.len());
    for s in &h.seed_points {
        out.pair(10, finite(s.x)?);
        out.pair(20, finite(s.y)?);
    }
    if let HatchFill::Gradient(g) = &h.fill {
        gradient_tail(out, g)?;
    }
    Ok(())
}
