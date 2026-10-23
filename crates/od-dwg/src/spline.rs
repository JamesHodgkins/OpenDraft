//! SPLINE (Open Design Specification §20.4.40): reading it as stored, and
//! turning it into a flat shape.
//!
//! A DWG spline is stored one of two ways. By its control frame ("scenario
//! 1": degree, knots, control points, weights when rational), or by its fit
//! points alone ("scenario 2": fit points, fit tolerance, end tangents), in
//! which case there are no control points in the file and AutoCAD fits the
//! curve when it loads it. From R2013 two more fields come first: flags (bit
//! 1: made through fit points; bit 4: closed) and the knot spacing (0 chord,
//! 1 square root, 2 uniform, 15 custom), which decide the scenario.
//!
//! Points are world coordinates; a spline has no extrusion here. It is flat
//! when every point is at one height (dropped, as for every other entity).

use crate::bit::BitReader;
use crate::entities::{count, Point3};
use crate::error::{DwgError, DwgErrorKind};
use crate::limits::DwgLimits;
use crate::object::ObjectStreams;
use crate::report::{ImportReport, Skipped};
use crate::shapes::{all_sane, flatten, Shape};
use crate::version::DwgVersion;

/// The knot spacing code for "custom", which means a control frame.
const CUSTOM_KNOTS: u32 = 15;

/// A spline as stored.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SplineData {
    Control {
        degree: u32,
        closed: bool,
        periodic: bool,
        knot_tolerance: f64,
        control_tolerance: f64,
        knots: Vec<f64>,
        points: Vec<Point3>,
        /// One per point when the spline is rational.
        weights: Option<Vec<f64>>,
    },
    Fit {
        degree: u32,
        closed: bool,
        tolerance: f64,
        start_tangent: Point3,
        end_tangent: Point3,
        /// The knot spacing code (R2013 and later).
        knots: Option<u32>,
        points: Vec<Point3>,
    },
}

fn bad(d: &BitReader<'_>) -> DwgError {
    d.error(DwgErrorKind::InvalidObject)
}

pub(crate) fn read_spline(
    s: &mut ObjectStreams<'_>,
    version: DwgVersion,
    limits: &DwgLimits,
) -> Result<SplineData, DwgError> {
    let d = &mut s.data;
    let mut scenario = d.bl()?;
    let (mut flags, mut knot_code) = (0, None);
    if version >= DwgVersion::R2013 {
        flags = d.bl()?;
        let code = d.bl()?;
        knot_code = Some(code);
        if flags & 1 != 0 {
            scenario = 2;
        }
        if code == CUSTOM_KNOTS {
            scenario = 1;
        }
    }
    let degree = d.bl()?;
    match scenario {
        1 => {
            let _rational = d.bit()?;
            let closed = d.bit()?;
            let periodic = d.bit()?;
            let knot_tolerance = d.bd()?;
            let control_tolerance = d.bd()?;
            let n_knots = d.bl()?;
            let n_points = d.bl()?;
            let weighted = d.bit()?;
            let n_knots = count(d, n_knots, 2, limits, "spline knots")?;
            let n_points = count(d, n_points, 6, limits, "spline control points")?;
            let knots = (0..n_knots)
                .map(|_| d.bd())
                .collect::<Result<Vec<_>, _>>()?;
            let mut points = Vec::with_capacity(n_points);
            let mut weights = Vec::with_capacity(if weighted { n_points } else { 0 });
            for _ in 0..n_points {
                points.push(d.bd3()?);
                if weighted {
                    weights.push(d.bd()?);
                }
            }
            Ok(SplineData::Control {
                degree,
                closed,
                periodic,
                knot_tolerance,
                control_tolerance,
                knots,
                points,
                weights: weighted.then_some(weights),
            })
        }
        2 => {
            let tolerance = d.bd()?;
            let start_tangent = d.bd3()?;
            let end_tangent = d.bd3()?;
            let n_points = d.bl()?;
            let n_points = count(d, n_points, 6, limits, "spline fit points")?;
            let points = (0..n_points)
                .map(|_| d.bd3())
                .collect::<Result<Vec<_>, _>>()?;
            Ok(SplineData::Fit {
                degree,
                closed: flags & 4 != 0,
                tolerance,
                start_tangent,
                end_tangent,
                knots: knot_code,
                points,
            })
        }
        _ => Err(bad(d)),
    }
}

/// The points' common height, or `NonPlanar` when they have none; the size
/// of their coordinates comes with it, to judge the height against.
fn height(points: &[Point3]) -> Result<(f64, f64), Skipped> {
    let size = points.iter().flatten().fold(1.0f64, |m, v| m.max(v.abs()));
    let z = points.first().map_or(0.0, |p| p[2]);
    if points.iter().any(|p| (p[2] - z).abs() > 1e-9 * size) {
        return Err(Skipped::NonPlanar);
    }
    Ok((z, size))
}

fn flat(points: &[Point3]) -> Vec<[f64; 2]> {
    points.iter().map(|p| [p[0], p[1]]).collect()
}

/// A tangent in the drawing plane, `None` when none was given (a zero
/// vector); `NonPlanar` when it leaves the plane.
fn tangent(v: Point3) -> Result<Option<[f64; 2]>, Skipped> {
    let length = v[0].hypot(v[1]);
    if v[2].abs() > 1e-9 * length.max(1.0) {
        return Err(Skipped::NonPlanar);
    }
    Ok((length > 0.0).then_some([v[0], v[1]]))
}

/// The flat shape of a spline. Only finiteness, sizes and flatness are
/// checked here; the application checks the rest (knots, weights) when it
/// builds its own spline.
pub(crate) fn spline(d: &SplineData, report: &mut ImportReport) -> Result<Shape, Skipped> {
    let (points, scalars): (&[Point3], Vec<f64>) = match d {
        SplineData::Control {
            points,
            knots,
            weights,
            knot_tolerance,
            control_tolerance,
            ..
        } => {
            let mut all = knots.clone();
            all.extend(weights.iter().flatten());
            all.extend([*knot_tolerance, *control_tolerance]);
            (points, all)
        }
        SplineData::Fit {
            points,
            tolerance,
            start_tangent,
            end_tangent,
            ..
        } => {
            let mut all = vec![*tolerance];
            all.extend(start_tangent.iter().chain(end_tangent));
            (points, all)
        }
    };
    let coordinates: Vec<f64> = points.iter().flatten().copied().collect();
    if points.is_empty() || !all_sane(&coordinates) || !scalars.iter().all(|v| v.is_finite()) {
        return Err(Skipped::InvalidGeometry);
    }
    let (z, size) = height(points)?;
    let shape = match d {
        SplineData::Control {
            degree,
            closed,
            periodic,
            knot_tolerance,
            control_tolerance,
            knots,
            weights,
            ..
        } => Shape::Spline {
            degree: *degree,
            control_points: flat(points),
            knots: knots.clone(),
            weights: weights.clone().unwrap_or_default(),
            closed: *closed,
            periodic: *periodic,
            knot_tolerance: *knot_tolerance,
            control_tolerance: *control_tolerance,
        },
        SplineData::Fit {
            degree,
            closed,
            tolerance,
            start_tangent,
            end_tangent,
            knots,
            ..
        } => Shape::FitSpline {
            degree: *degree,
            points: flat(points),
            tolerance: *tolerance,
            start_tangent: tangent(*start_tangent)?,
            end_tangent: tangent(*end_tangent)?,
            knots: knots.filter(|k| *k != CUSTOM_KNOTS),
            closed: *closed,
        },
    };
    flatten(z, size, report);
    Ok(shape)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Degradation;
    use crate::testkit::{BitWriter, Buffers};

    fn with_data(w: &BitWriter) -> Buffers {
        Buffers::new(w, &BitWriter::new(), &BitWriter::new())
    }

    fn report() -> ImportReport {
        ImportReport::new(DwgVersion::R2018)
    }

    /// A rational quadratic by its control frame, as R2010 stores it.
    fn control_frame(w: &mut BitWriter) {
        w.bl(1).bl(2);
        w.bit(true)
            .bit(false)
            .bit(false)
            .bd(1e-7)
            .bd(1e-7)
            .bl(6)
            .bl(3)
            .bit(true);
        for k in [0.0, 0.0, 0.0, 1.0, 1.0, 1.0] {
            w.bd(k);
        }
        for (p, wt) in [
            ([1.0, 0.0, 2.0], 1.0),
            ([1.0, 1.0, 2.0], 0.5),
            ([0.0, 1.0, 2.0], 1.0),
        ] {
            w.bd3(p).bd(wt);
        }
    }

    #[test]
    fn a_control_frame_reads_with_its_weights() {
        let mut w = BitWriter::new();
        control_frame(&mut w);
        let b = with_data(&w);
        let d = read_spline(&mut b.streams(), DwgVersion::R2010, &DwgLimits::default()).unwrap();
        let SplineData::Control {
            degree,
            knots,
            points,
            weights,
            ..
        } = &d
        else {
            panic!("a control frame, got {d:?}")
        };
        assert_eq!((*degree, knots.len(), points.len()), (2, 6, 3));
        assert_eq!(weights.as_deref(), Some(&[1.0, 0.5, 1.0][..]));
        let mut r = report();
        let Shape::Spline {
            control_points,
            weights,
            ..
        } = spline(&d, &mut r).unwrap()
        else {
            panic!("a spline")
        };
        assert_eq!(control_points, [[1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        assert_eq!(weights, [1.0, 0.5, 1.0]);
        assert_eq!(r.degraded.get(&Degradation::ElevationFlattened), Some(&1));
    }

    #[test]
    fn from_r2013_the_flags_say_fit_points_and_the_knot_spacing() {
        let mut w = BitWriter::new();
        // Scenario says 1, but the flags say fit points: it is scenario 2.
        w.bl(1).bl(1 | 4).bl(1).bl(3);
        w.bd(0.0).bd3([0.0, 0.0, 0.0]).bd3([0.0, 1.0, 0.0]).bl(3);
        for p in [[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0]] {
            w.bd3(p);
        }
        let b = with_data(&w);
        let d = read_spline(&mut b.streams(), DwgVersion::R2018, &DwgLimits::default()).unwrap();
        let shape = spline(&d, &mut report()).unwrap();
        assert_eq!(
            shape,
            Shape::FitSpline {
                degree: 3,
                points: vec![[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]],
                tolerance: 0.0,
                start_tangent: None,
                end_tangent: Some([0.0, 1.0]),
                knots: Some(1),
                closed: true,
            }
        );
        // Custom knots mean a control frame whatever the scenario says.
        let mut w = BitWriter::new();
        w.bl(2).bl(0).bl(CUSTOM_KNOTS).bl(2);
        w.bit(true)
            .bit(false)
            .bit(false)
            .bd(1e-7)
            .bd(1e-7)
            .bl(6)
            .bl(3)
            .bit(false);
        for k in [0.0, 0.0, 0.0, 1.0, 1.0, 1.0] {
            w.bd(k);
        }
        for p in [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]] {
            w.bd3(p);
        }
        let b = with_data(&w);
        let d = read_spline(&mut b.streams(), DwgVersion::R2018, &DwgLimits::default()).unwrap();
        assert!(matches!(d, SplineData::Control { .. }));
    }

    #[test]
    fn a_huge_count_an_unknown_scenario_and_uneven_heights_are_refused() {
        let mut w = BitWriter::new();
        w.bl(1)
            .bl(3)
            .bit(false)
            .bit(false)
            .bit(false)
            .bd(0.0)
            .bd(0.0);
        w.bl(4_000_000_000).bl(4).bit(false);
        let b = with_data(&w);
        let limits = DwgLimits::default();
        assert!(read_spline(&mut b.streams(), DwgVersion::R2010, &limits).is_err());
        let mut w = BitWriter::new();
        w.bl(7).bl(3);
        let b = with_data(&w);
        assert!(read_spline(&mut b.streams(), DwgVersion::R2010, &limits).is_err());
        let fit = |start_tangent: Point3, points: Vec<Point3>| SplineData::Fit {
            degree: 3,
            closed: false,
            tolerance: 0.0,
            start_tangent,
            end_tangent: [0.0; 3],
            knots: None,
            points,
        };
        let uneven = fit([0.0; 3], vec![[0.0, 0.0, 0.0], [1.0, 1.0, 5.0]]);
        assert_eq!(spline(&uneven, &mut report()), Err(Skipped::NonPlanar));
        let rising = fit([0.0, 1.0, 1.0], vec![[0.0, 0.0, 0.0], [1.0, 1.0, 0.0]]);
        assert_eq!(spline(&rising, &mut report()), Err(Skipped::NonPlanar));
        let nan = fit([0.0; 3], vec![[f64::NAN, 0.0, 0.0], [1.0, 1.0, 0.0]]);
        assert_eq!(spline(&nan, &mut report()), Err(Skipped::InvalidGeometry));
    }
}
