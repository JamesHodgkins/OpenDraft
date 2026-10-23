//! From an entity's stored geometry to flat drawing-plane shapes, with the
//! validation and the report entries that go with it (Open Design
//! Specification §9 of the DWG support specification: coordinates and
//! numerical validation).

use crate::drawing::DwgColor;
use crate::entities::{
    ArcData, CircleData, EllipseData, InsertData, LineData, LwPolylineData, Point3, Polyline2dData,
    Vertex2dData,
};
use crate::geometry::{elevation_matters, NotFlat, Orientation};
use crate::handles::DwgHandle;
use crate::report::{Degradation, ImportReport, Normalization, Skipped};

/// The largest coordinate, radius or width accepted. Anything beyond it would
/// only poison bounds and rendering.
pub const MAX_ABS_COORDINATE: f64 = 1e12;

/// A flat shape, in the drawing plane. Angles are counter-clockwise radians
/// as stored (not normalised); a bulge is the tangent of a quarter of the
/// segment's arc angle, positive counter-clockwise.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Line {
        start: [f64; 2],
        end: [f64; 2],
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
    Arc {
        center: [f64; 2],
        radius: f64,
        start: f64,
        end: f64,
    },
    /// An ellipse or elliptical arc: the centre, the vector to the end of the
    /// major axis, minor over major radius, and counter-clockwise parameters
    /// (eccentric anomaly, not angles; 0 to 2π for a full ellipse).
    Ellipse {
        center: [f64; 2],
        major_axis: [f64; 2],
        ratio: f64,
        start: f64,
        end: f64,
    },
    /// A spline by its control frame: the degree, control points, the full
    /// knot vector, one weight per control point when rational (else none),
    /// and the flags and tolerances as stored. Not checked beyond finite
    /// values: the knots and weights are the application's to judge.
    Spline {
        degree: u32,
        control_points: Vec<[f64; 2]>,
        knots: Vec<f64>,
        weights: Vec<f64>,
        closed: bool,
        periodic: bool,
        knot_tolerance: f64,
        control_tolerance: f64,
    },
    /// A spline stored by its fit points alone, for the reader to fit (as
    /// AutoCAD does on loading): the degree, the points, the fit tolerance,
    /// end tangent directions when given, and the knot spacing (0 chord,
    /// 1 square root, 2 uniform) when the file says (R2013 and later).
    FitSpline {
        degree: u32,
        points: Vec<[f64; 2]>,
        tolerance: f64,
        start_tangent: Option<[f64; 2]>,
        end_tangent: Option<[f64; 2]>,
        knots: Option<u32>,
        closed: bool,
    },
    Polyline {
        points: Vec<[f64; 2]>,
        closed: bool,
        /// Empty, or one per point.
        bulges: Vec<f64>,
        /// Empty, or one (start, end) per point.
        widths: Vec<[f64; 2]>,
    },
    /// A block reference: the block's base point goes to `insertion`, after
    /// scaling along the block's axes (negative mirrors) and turning by
    /// `rotation`.
    Insert {
        /// One of the drawing's [`DwgBlock`](crate::DwgBlock)s.
        block: DwgHandle,
        insertion: [f64; 2],
        scale: [f64; 2],
        rotation: f64,
    },
    /// A single line of text. `at` is where the alignment codes hang it:
    /// `horizontal` 0 left, 1 centre, 2 right and `vertical` 0 baseline,
    /// 1 bottom, 2 middle, 3 top. `rotation` and `oblique` are radians.
    Text {
        at: [f64; 2],
        rotation: f64,
        height: f64,
        width_factor: f64,
        oblique: f64,
        value: String,
        horizontal: u8,
        vertical: u8,
        backward: bool,
        upside_down: bool,
        style: Option<DwgHandle>,
    },
    /// Multiline text: its contents with their format codes, the box width
    /// (0 for none), attachment 1 to 9 (top left to bottom right), flow
    /// (1 left to right, 3 top to bottom, 5 by style), line spacing as stored
    /// and the mask (its scale, and a colour unless it is the window's).
    MText {
        at: [f64; 2],
        direction: [f64; 2],
        height: f64,
        width: f64,
        attachment: u8,
        flow: u8,
        contents: String,
        spacing_style: u8,
        spacing: f64,
        background: Option<(f64, Option<DwgColor>)>,
        style: Option<DwgHandle>,
    },
}

fn sane(v: f64) -> bool {
    v.is_finite() && v.abs() <= MAX_ABS_COORDINATE
}

pub(crate) fn all_sane(values: &[f64]) -> bool {
    values.iter().all(|&v| sane(v))
}

fn orientation(extrusion: Point3) -> Result<Orientation, Skipped> {
    Orientation::from_extrusion(extrusion).map_err(|e| match e {
        NotFlat::Invalid => Skipped::InvalidGeometry,
        NotFlat::Tilted => Skipped::NonPlanar,
    })
}

/// Notes a dropped elevation, if it is big enough to matter.
pub(crate) fn flatten(elevation: f64, scale: f64, report: &mut ImportReport) {
    if elevation_matters(elevation, scale) {
        report.degrade(Degradation::ElevationFlattened);
    }
}

pub(crate) fn line(d: &LineData, report: &mut ImportReport) -> Result<Shape, Skipped> {
    let coords = [
        d.start[0], d.start[1], d.start[2], d.end[0], d.end[1], d.end[2],
    ];
    if !all_sane(&coords) {
        return Err(Skipped::InvalidGeometry);
    }
    let scale = coords.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    // Both ends must be at one height to be flattened without bending the line.
    if elevation_matters(d.end[2] - d.start[2], scale) {
        return Err(Skipped::NonPlanar);
    }
    flatten(d.start[2], scale, report);
    Ok(Shape::Line {
        start: [d.start[0], d.start[1]],
        end: [d.end[0], d.end[1]],
    })
}

pub(crate) fn circle(d: &CircleData, report: &mut ImportReport) -> Result<Shape, Skipped> {
    if !all_sane(&d.center) || !sane(d.radius) || d.radius <= 0.0 {
        return Err(Skipped::InvalidGeometry);
    }
    let o = orientation(d.extrusion)?;
    flatten(
        d.center[2],
        d.center[0].abs().max(d.center[1].abs()).max(d.radius),
        report,
    );
    Ok(Shape::Circle {
        center: o.point([d.center[0], d.center[1]]),
        radius: d.radius,
    })
}

pub(crate) fn arc(d: &ArcData, report: &mut ImportReport) -> Result<Shape, Skipped> {
    if !all_sane(&d.center) || !sane(d.radius) || d.radius <= 0.0 {
        return Err(Skipped::InvalidGeometry);
    }
    if !d.start.is_finite() || !d.end.is_finite() {
        return Err(Skipped::InvalidGeometry);
    }
    let o = orientation(d.extrusion)?;
    flatten(
        d.center[2],
        d.center[0].abs().max(d.center[1].abs()).max(d.radius),
        report,
    );
    let (start, end) = o.arc(d.start, d.end);
    Ok(Shape::Arc {
        center: o.point([d.center[0], d.center[1]]),
        radius: d.radius,
        start,
        end,
    })
}

/// The smallest minor-to-major ratio and parameter span an ellipse may have
/// (AutoCAD's own limits).
const MIN_RATIO: f64 = 1e-6;

/// An ellipse is in world coordinates whatever its extrusion; seen from
/// below (a downward extrusion) its parameters run clockwise in the drawing,
/// so the arc from `s` to `e` is the drawing's arc from `-e` to `-s`.
pub(crate) fn ellipse(d: &EllipseData, report: &mut ImportReport) -> Result<Shape, Skipped> {
    let finite = all_sane(&d.center) && all_sane(&d.major_axis);
    if !finite || !d.start.is_finite() || !d.end.is_finite() {
        return Err(Skipped::InvalidGeometry);
    }
    if !(d.ratio.is_finite() && (MIN_RATIO..=1.0 + 1e-9).contains(&d.ratio)) {
        return Err(Skipped::InvalidGeometry);
    }
    let a = d.major_axis[0].hypot(d.major_axis[1]);
    if a == 0.0 || (d.end - d.start).abs() < MIN_RATIO {
        return Err(Skipped::InvalidGeometry);
    }
    let o = orientation(d.extrusion)?;
    if elevation_matters(d.major_axis[2], a) {
        return Err(Skipped::NonPlanar);
    }
    flatten(
        d.center[2],
        d.center[0].abs().max(d.center[1].abs()).max(a),
        report,
    );
    let (start, end) = match o {
        Orientation::Upright => (d.start, d.end),
        Orientation::Mirrored => (-d.end, -d.start),
    };
    Ok(Shape::Ellipse {
        center: [d.center[0], d.center[1]],
        major_axis: [d.major_axis[0], d.major_axis[1]],
        ratio: d.ratio.min(1.0),
        start,
        end,
    })
}

/// A block reference, once its block is known to be readable. Under a
/// downward extrusion the mirror in X moves to the reference itself: the
/// insertion point's X and the rotation change sign and the X scale flips.
pub(crate) fn insert(
    d: &InsertData,
    block: DwgHandle,
    report: &mut ImportReport,
) -> Result<Shape, Skipped> {
    let scale = [d.scale[0], d.scale[1]];
    if !all_sane(&d.insertion) || !all_sane(&scale) || !d.rotation.is_finite() {
        return Err(Skipped::InvalidGeometry);
    }
    if scale.contains(&0.0) {
        return Err(Skipped::InvalidGeometry);
    }
    let o = orientation(d.extrusion)?;
    flatten(
        d.insertion[2],
        d.insertion[0].abs().max(d.insertion[1].abs()),
        report,
    );
    let (scale, rotation) = match o {
        Orientation::Upright => (scale, d.rotation),
        Orientation::Mirrored => ([-scale[0], scale[1]], -d.rotation),
    };
    Ok(Shape::Insert {
        block,
        insertion: o.point([d.insertion[0], d.insertion[1]]),
        scale,
        rotation,
    })
}

/// Turns vertices into the shape's arrays: bulges and widths are left empty
/// when they are all zero.
fn polyline(
    o: Orientation,
    vertices: Vec<([f64; 2], f64, [f64; 2])>,
    closed: bool,
) -> Result<Shape, Skipped> {
    if vertices.len() < 2 {
        return Err(Skipped::DegeneratePolyline);
    }
    let mut points = Vec::with_capacity(vertices.len());
    let mut bulges = Vec::with_capacity(vertices.len());
    let mut widths = Vec::with_capacity(vertices.len());
    for (point, bulge, width) in vertices {
        if !all_sane(&point) || !sane(bulge) || !all_sane(&width) || width.iter().any(|&w| w < 0.0)
        {
            return Err(Skipped::InvalidGeometry);
        }
        points.push(o.point(point));
        bulges.push(o.bulge(bulge));
        widths.push(width);
    }
    if bulges.iter().all(|&b| b == 0.0) {
        bulges.clear();
    }
    if widths.iter().all(|w| w[0] == 0.0 && w[1] == 0.0) {
        widths.clear();
    }
    Ok(Shape::Polyline {
        points,
        closed,
        bulges,
        widths,
    })
}

pub(crate) fn lwpolyline(d: &LwPolylineData, report: &mut ImportReport) -> Result<Shape, Skipped> {
    if !sane(d.elevation) {
        return Err(Skipped::InvalidGeometry);
    }
    let o = orientation(d.extrusion)?;
    let scale = d
        .points
        .iter()
        .flatten()
        .fold(0.0f64, |m, v| m.max(v.abs()));
    flatten(d.elevation, scale, report);
    let constant = d.constant_width.unwrap_or(0.0);
    let vertices = d
        .points
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let bulge = d.bulges.get(i).copied().unwrap_or(0.0);
            let width = d.widths.get(i).copied().unwrap_or([constant, constant]);
            (p, bulge, width)
        })
        .collect();
    polyline(o, vertices, d.closed)
}

/// Joins an old-style polyline and its vertices. Spline-frame control points
/// (vertex flag 16) are not on the curve and are dropped, as the vertices the
/// spline fit generated stand for it.
pub(crate) fn polyline2d(
    header: &Polyline2dData,
    vertices: &[&Vertex2dData],
    report: &mut ImportReport,
) -> Result<Shape, Skipped> {
    if !sane(header.elevation) {
        return Err(Skipped::InvalidGeometry);
    }
    let o = orientation(header.extrusion)?;
    let scale = vertices
        .iter()
        .fold(0.0f64, |m, v| m.max(v.point[0].abs()).max(v.point[1].abs()));
    flatten(header.elevation, scale, report);
    let on_curve: Vec<&Vertex2dData> = vertices
        .iter()
        .copied()
        .filter(|v| v.flags & 16 == 0)
        .collect();
    if on_curve.len() != vertices.len() {
        report.degrade(Degradation::SplineFrameVertices);
    }
    let vertices = on_curve
        .iter()
        .map(|v| {
            (
                [v.point[0], v.point[1]],
                v.bulge,
                [v.start_width, v.end_width],
            )
        })
        .collect();
    let shape = polyline(o, vertices, header.flags & 1 != 0)?;
    report.normalize(Normalization::OldPolylineJoined);
    Ok(shape)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::DwgVersion;
    use std::f64::consts::PI;

    const UP: Point3 = [0.0, 0.0, 1.0];
    const DOWN: Point3 = [0.0, 0.0, -1.0];

    fn report() -> ImportReport {
        ImportReport::new(DwgVersion::R2013)
    }

    #[test]
    fn a_flat_line_is_exact() {
        let mut r = report();
        let d = LineData {
            start: [0.0, 0.0, 0.0],
            end: [3.0, 4.0, 0.0],
            extrusion: UP,
        };
        assert_eq!(
            line(&d, &mut r),
            Ok(Shape::Line {
                start: [0.0, 0.0],
                end: [3.0, 4.0]
            })
        );
        assert!(r.degraded.is_empty());
    }

    #[test]
    fn a_line_at_constant_height_is_flattened_with_a_note() {
        let mut r = report();
        let d = LineData {
            start: [0.0, 0.0, 5.0],
            end: [3.0, 4.0, 5.0],
            extrusion: UP,
        };
        assert!(line(&d, &mut r).is_ok());
        assert_eq!(r.degraded.get(&Degradation::ElevationFlattened), Some(&1));
    }

    #[test]
    fn a_sloping_line_is_not_flattened() {
        let d = LineData {
            start: [0.0, 0.0, 0.0],
            end: [3.0, 4.0, 2.0],
            extrusion: UP,
        };
        assert_eq!(line(&d, &mut report()), Err(Skipped::NonPlanar));
    }

    #[test]
    fn bad_numbers_are_refused() {
        let nan = LineData {
            start: [f64::NAN, 0.0, 0.0],
            end: [1.0, 1.0, 0.0],
            extrusion: UP,
        };
        assert_eq!(line(&nan, &mut report()), Err(Skipped::InvalidGeometry));
        let huge = CircleData {
            center: [1e13, 0.0, 0.0],
            radius: 1.0,
            extrusion: UP,
        };
        assert_eq!(circle(&huge, &mut report()), Err(Skipped::InvalidGeometry));
        let flat = CircleData {
            center: [0.0; 3],
            radius: 0.0,
            extrusion: UP,
        };
        assert_eq!(circle(&flat, &mut report()), Err(Skipped::InvalidGeometry));
        let neg = CircleData {
            center: [0.0; 3],
            radius: -1.0,
            extrusion: UP,
        };
        assert_eq!(circle(&neg, &mut report()), Err(Skipped::InvalidGeometry));
        let zero = CircleData {
            center: [0.0; 3],
            radius: 1.0,
            extrusion: [0.0; 3],
        };
        assert_eq!(circle(&zero, &mut report()), Err(Skipped::InvalidGeometry));
        let tilted = CircleData {
            center: [0.0; 3],
            radius: 1.0,
            extrusion: [0.0, 0.6, 0.8],
        };
        assert_eq!(circle(&tilted, &mut report()), Err(Skipped::NonPlanar));
    }

    fn ellipse_data(extrusion: Point3) -> EllipseData {
        EllipseData {
            center: [3.0, 4.0, 0.0],
            major_axis: [2.0, 0.0, 0.0],
            extrusion,
            ratio: 0.5,
            start: 0.25,
            end: 1.0,
        }
    }

    #[test]
    fn an_ellipse_stays_in_world_coordinates_and_a_downward_one_runs_back() {
        let up = ellipse(&ellipse_data(UP), &mut report()).unwrap();
        assert_eq!(
            up,
            Shape::Ellipse {
                center: [3.0, 4.0],
                major_axis: [2.0, 0.0],
                ratio: 0.5,
                start: 0.25,
                end: 1.0,
            }
        );
        // Seen from below, the same parameters run the other way round.
        let Shape::Ellipse {
            center, start, end, ..
        } = ellipse(&ellipse_data(DOWN), &mut report()).unwrap()
        else {
            panic!("an ellipse")
        };
        assert_eq!((center, start, end), ([3.0, 4.0], -1.0, -0.25));
    }

    #[test]
    fn a_bad_or_tilted_ellipse_is_skipped() {
        let tilted = ellipse_data([0.0, 1.0, 1.0]);
        assert_eq!(ellipse(&tilted, &mut report()), Err(Skipped::NonPlanar));
        let rising = EllipseData {
            major_axis: [2.0, 0.0, 1.0],
            ..ellipse_data(UP)
        };
        assert_eq!(ellipse(&rising, &mut report()), Err(Skipped::NonPlanar));
        for bad in [
            EllipseData {
                ratio: 0.0,
                ..ellipse_data(UP)
            },
            EllipseData {
                ratio: 2.0,
                ..ellipse_data(UP)
            },
            EllipseData {
                major_axis: [0.0, 0.0, 0.0],
                ..ellipse_data(UP)
            },
            EllipseData {
                end: 0.25,
                ..ellipse_data(UP)
            },
            EllipseData {
                center: [f64::NAN, 0.0, 0.0],
                ..ellipse_data(UP)
            },
        ] {
            assert_eq!(ellipse(&bad, &mut report()), Err(Skipped::InvalidGeometry));
        }
        let mut r = report();
        let raised = EllipseData {
            center: [3.0, 4.0, 7.0],
            ..ellipse_data(UP)
        };
        assert!(ellipse(&raised, &mut r).is_ok());
        assert_eq!(r.degraded.get(&Degradation::ElevationFlattened), Some(&1));
    }

    #[test]
    fn a_circle_with_a_downward_normal_is_mirrored_in_x() {
        let d = CircleData {
            center: [3.0, 4.0, 0.0],
            radius: 2.0,
            extrusion: DOWN,
        };
        assert_eq!(
            circle(&d, &mut report()),
            Ok(Shape::Circle {
                center: [-3.0, 4.0],
                radius: 2.0
            })
        );
    }

    #[test]
    fn a_mirrored_arc_keeps_the_same_points() {
        // An OCS arc from 0 to 90 degrees about (3, 4) runs from (5, 4) to
        // (3, 6); with the normal down those points are (-5, 4) and (-3, 6),
        // and the drawing-plane arc runs counter-clockwise from the second to
        // the first.
        let d = ArcData {
            center: [3.0, 4.0, 0.0],
            radius: 2.0,
            start: 0.0,
            end: PI / 2.0,
            extrusion: DOWN,
        };
        let Ok(Shape::Arc {
            center,
            radius,
            start,
            end,
        }) = arc(&d, &mut report())
        else {
            panic!("not an arc");
        };
        let at = |a: f64| [center[0] + radius * a.cos(), center[1] + radius * a.sin()];
        // OCS start point (5, 4) maps to (-5, 4); OCS end point (3, 6) to (-3, 6).
        let (p_start, p_end) = (at(start), at(end));
        assert!(
            (p_start[0] + 3.0).abs() < 1e-9 && (p_start[1] - 6.0).abs() < 1e-9,
            "{p_start:?}"
        );
        assert!(
            (p_end[0] + 5.0).abs() < 1e-9 && (p_end[1] - 4.0).abs() < 1e-9,
            "{p_end:?}"
        );
    }

    #[test]
    fn an_elevated_circle_is_flattened_with_a_note() {
        let mut r = report();
        let d = CircleData {
            center: [0.0, 0.0, 12.0],
            radius: 1.0,
            extrusion: UP,
        };
        assert!(circle(&d, &mut r).is_ok());
        assert_eq!(r.degraded.get(&Degradation::ElevationFlattened), Some(&1));
    }

    fn lw(points: &[[f64; 2]], bulges: &[f64], widths: &[[f64; 2]]) -> LwPolylineData {
        LwPolylineData {
            closed: false,
            elevation: 0.0,
            extrusion: UP,
            constant_width: None,
            points: points.to_vec(),
            bulges: bulges.to_vec(),
            widths: widths.to_vec(),
        }
    }

    #[test]
    fn a_lwpolyline_drops_all_zero_bulges_and_widths() {
        let d = lw(
            &[[0.0, 0.0], [1.0, 0.0]],
            &[0.0, 0.0],
            &[[0.0, 0.0], [0.0, 0.0]],
        );
        let s = lwpolyline(&d, &mut report()).unwrap();
        assert_eq!(
            s,
            Shape::Polyline {
                points: vec![[0.0, 0.0], [1.0, 0.0]],
                closed: false,
                bulges: vec![],
                widths: vec![]
            }
        );
    }

    #[test]
    fn a_lwpolyline_keeps_bulges_and_applies_a_constant_width() {
        let mut d = lw(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], &[0.5, 0.0, 0.0], &[]);
        d.constant_width = Some(0.2);
        d.closed = true;
        let Shape::Polyline {
            bulges,
            widths,
            closed,
            ..
        } = lwpolyline(&d, &mut report()).unwrap()
        else {
            panic!();
        };
        assert!(closed);
        assert_eq!(bulges, [0.5, 0.0, 0.0]);
        assert_eq!(widths, [[0.2, 0.2]; 3]);
    }

    #[test]
    fn a_mirrored_lwpolyline_flips_x_and_the_sense_of_its_bulges() {
        let mut d = lw(&[[1.0, 0.0], [2.0, 1.0]], &[0.5, 0.0], &[]);
        d.extrusion = DOWN;
        let Shape::Polyline { points, bulges, .. } = lwpolyline(&d, &mut report()).unwrap() else {
            panic!();
        };
        assert_eq!(points, [[-1.0, 0.0], [-2.0, 1.0]]);
        assert_eq!(bulges, [-0.5, 0.0]);
    }

    #[test]
    fn polylines_with_bad_vertices_are_refused() {
        let one = lw(&[[0.0, 0.0]], &[], &[]);
        assert_eq!(
            lwpolyline(&one, &mut report()),
            Err(Skipped::DegeneratePolyline)
        );
        let nan = lw(&[[0.0, 0.0], [f64::NAN, 1.0]], &[], &[]);
        assert_eq!(
            lwpolyline(&nan, &mut report()),
            Err(Skipped::InvalidGeometry)
        );
        let negative = lw(&[[0.0, 0.0], [1.0, 1.0]], &[], &[[-1.0, 0.0], [0.0, 0.0]]);
        assert_eq!(
            lwpolyline(&negative, &mut report()),
            Err(Skipped::InvalidGeometry)
        );
        let tilted = LwPolylineData {
            extrusion: [1.0, 0.0, 0.0],
            ..lw(&[[0.0, 0.0], [1.0, 1.0]], &[], &[])
        };
        assert_eq!(lwpolyline(&tilted, &mut report()), Err(Skipped::NonPlanar));
    }

    fn insert_data(extrusion: Point3) -> InsertData {
        InsertData {
            insertion: [3.0, 4.0, 0.0],
            scale: [2.0, 1.0, 1.0],
            rotation: 0.5,
            extrusion,
            block: Some(DwgHandle(9)),
            attributes: 0,
            attribute_handles: Vec::new(),
        }
    }

    #[test]
    fn an_insert_and_its_mirror() {
        let s = insert(&insert_data(UP), DwgHandle(9), &mut report());
        assert_eq!(
            s,
            Ok(Shape::Insert {
                block: DwgHandle(9),
                insertion: [3.0, 4.0],
                scale: [2.0, 1.0],
                rotation: 0.5
            })
        );
        let s = insert(&insert_data(DOWN), DwgHandle(9), &mut report());
        assert_eq!(
            s,
            Ok(Shape::Insert {
                block: DwgHandle(9),
                insertion: [-3.0, 4.0],
                scale: [-2.0, 1.0],
                rotation: -0.5
            })
        );
        let flat = InsertData {
            scale: [0.0, 1.0, 1.0],
            ..insert_data(UP)
        };
        assert_eq!(
            insert(&flat, DwgHandle(9), &mut report()),
            Err(Skipped::InvalidGeometry)
        );
    }

    fn vertex(x: f64, y: f64, flags: u8) -> Vertex2dData {
        Vertex2dData {
            flags,
            point: [x, y, 0.0],
            start_width: 0.0,
            end_width: 0.0,
            bulge: 0.0,
        }
    }

    #[test]
    fn an_old_polyline_is_joined_and_counted_as_normalised() {
        let header = Polyline2dData {
            flags: 1,
            curve_type: 0,
            elevation: 0.0,
            extrusion: UP,
            vertices: vec![],
        };
        let (a, b, c) = (
            vertex(0.0, 0.0, 0),
            vertex(2.0, 0.0, 0),
            vertex(2.0, 2.0, 0),
        );
        let mut r = report();
        let s = polyline2d(&header, &[&a, &b, &c], &mut r).unwrap();
        assert!(matches!(s, Shape::Polyline { closed: true, ref points, .. } if points.len() == 3));
        assert_eq!(
            r.normalized.get(&Normalization::OldPolylineJoined),
            Some(&1)
        );
    }

    #[test]
    fn spline_frame_vertices_are_left_out_with_a_note() {
        let header = Polyline2dData {
            flags: 4,
            curve_type: 5,
            elevation: 0.0,
            extrusion: UP,
            vertices: vec![],
        };
        let (a, f, b) = (
            vertex(0.0, 0.0, 0),
            vertex(1.0, 5.0, 16),
            vertex(2.0, 0.0, 0),
        );
        let mut r = report();
        let s = polyline2d(&header, &[&a, &f, &b], &mut r).unwrap();
        assert!(matches!(s, Shape::Polyline { ref points, .. } if points.len() == 2));
        assert_eq!(r.degraded.get(&Degradation::SplineFrameVertices), Some(&1));
    }
}
