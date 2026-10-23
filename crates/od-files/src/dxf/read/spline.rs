//! SPLINE records, in ENTITIES and in BLOCKS alike.
//!
//! Control points (10/20/30) and fit points (11/21/31) are world coordinates
//! whatever the normal (210/220/230), so a normal along +Z or -Z changes
//! nothing; a tilted one, or points at different heights, is skipped as
//! non-planar (a common height is dropped, as for every other DXF object).
//! Repeated codes are gathered by code, so their order does not matter, and
//! the declared counts (72, 73, 74) must match what is there. Weights (41)
//! go with the rational flag (4). Periodic splines are read in the full form
//! (`control points + degree + 1` knots); AutoCAD's shorter periodic form is
//! skipped by name rather than guessed at. A spline with fit points and no
//! control points is fitted as a cubic (chord knots; natural ends, or smooth
//! where it closes); with end tangents or of another degree it is skipped. Flag bits other than closed,
//! periodic, rational, planar and linear are ignored.

use od_core::{Entity, KnotParameterization, Point, Spline, SplineFitData};

use super::{Importer, Record};
use crate::native::spline::MAX_FILE_SPLINE_POINTS;

/// How much a normal may lean off the Z axis, or points differ in height,
/// and still count as flat.
const FLAT_TOLERANCE: f64 = 1e-9;
/// Most numbers all the splines of one file may bring in.
pub(super) const MAX_FILE_SPLINE_SCALARS: usize = 2_000_000;
/// Codes a spline may give at most once.
const ONCE: [i32; 17] = [
    70, 71, 72, 73, 74, 42, 43, 44, 12, 22, 32, 13, 23, 33, 210, 220, 230,
];

enum Reading {
    Spline(Box<Spline>),
    NonPlanar,
    PeriodicShortForm,
    FitOnly,
    TooLarge,
    Damaged,
}

/// Every value of `code`, as numbers; `None` if any is not one.
fn numbers(record: &Record, code: i32) -> Option<Vec<f64>> {
    record
        .groups
        .iter()
        .filter(|g| g.code == code)
        .map(|g| g.value.trim().parse::<f64>().ok().filter(|v| v.is_finite()))
        .collect()
}

fn count(record: &Record, code: i32) -> usize {
    record.groups.iter().filter(|g| g.code == code).count()
}

/// Points from codes `x`, `x + 10` and (optionally) `x + 20`, which must
/// come `expected` times each, all at the same height.
fn points(record: &Record, x: i32, expected: usize) -> Result<(Vec<Point>, Option<f64>), Reading> {
    let (Some(xs), Some(ys), Some(zs)) = (
        numbers(record, x),
        numbers(record, x + 10),
        numbers(record, x + 20),
    ) else {
        return Err(Reading::Damaged);
    };
    if xs.len() != expected || ys.len() != expected || !(zs.is_empty() || zs.len() == expected) {
        return Err(Reading::Damaged);
    }
    let height = zs.first().copied();
    if let Some(h) = height {
        let size = xs
            .iter()
            .chain(&ys)
            .chain(&zs)
            .fold(1.0f64, |m, v| m.max(v.abs()));
        if zs.iter().any(|z| (z - h).abs() > FLAT_TOLERANCE * size) {
            return Err(Reading::NonPlanar);
        }
    }
    let found = xs
        .iter()
        .zip(&ys)
        .map(|(x, y)| Point::new(*x, *y))
        .collect();
    Ok((found, height))
}

/// An optional end tangent (codes `x`, `x + 10`, `x + 20`), which must lie
/// in the drawing plane.
fn tangent(record: &Record, x: i32) -> Result<Option<Point>, Reading> {
    let values = [x, x + 10, x + 20].map(|c| record.text(c).map(|_| record.number(c)));
    match values {
        [None, None, None] => Ok(None),
        [Some(Some(tx)), Some(Some(ty)), z] => {
            let tz = match z {
                None => 0.0,
                Some(Some(tz)) => tz,
                Some(None) => return Err(Reading::Damaged),
            };
            if tz.abs() > FLAT_TOLERANCE * tx.hypot(ty).max(1.0) {
                return Err(Reading::NonPlanar);
            }
            Ok(Some(Point::new(tx, ty)))
        }
        _ => Err(Reading::Damaged),
    }
}

fn declared(record: &Record, code: i32) -> Option<usize> {
    match record.text(code) {
        None => Some(0),
        Some(_) => usize::try_from(record.int(code)?).ok(),
    }
}

/// A spline given by fit points alone, fitted as AutoCAD does with no end
/// tangents (a cubic with natural ends, knots by chord length).
fn fit_only(record: &Record, fit_count: usize, closed: bool) -> Result<Spline, Reading> {
    if tangent(record, 12)?.is_some() || tangent(record, 13)?.is_some() {
        return Err(Reading::FitOnly);
    }
    let (points, _) = points(record, 11, fit_count)?;
    let how = KnotParameterization::Chord;
    let mut spline = if closed {
        Spline::from_closed_fit_points(points, how)
    } else {
        Spline::from_fit_points(points, how)
    }
    .map_err(|_| Reading::Damaged)?;
    let tolerance = match record.text(44) {
        None => od_core::spline::DEFAULT_FIT_TOLERANCE,
        Some(_) => record
            .number(44)
            .filter(|t| *t >= 0.0)
            .ok_or(Reading::Damaged)?,
    };
    if let Some(fit) = spline.fit.as_mut() {
        fit.tolerance = tolerance;
        // As for every DXF spline, the file does not say how it was fitted.
        fit.parameterization = None;
    }
    Ok(spline)
}

fn read(record: &Record, budget: &mut usize) -> Result<Spline, Reading> {
    if ONCE.iter().any(|c| count(record, *c) > 1) {
        return Err(Reading::Damaged);
    }
    let flags = record.int(70).unwrap_or(0);
    let (closed, periodic, rational) = (flags & 1 != 0, flags & 2 != 0, flags & 4 != 0);
    let degree = record
        .int(71)
        .and_then(|d| u8::try_from(d).ok())
        .ok_or(Reading::Damaged)?;
    let (Some(knot_count), Some(control_count), Some(fit_count)) = (
        declared(record, 72),
        declared(record, 73),
        declared(record, 74),
    ) else {
        return Err(Reading::Damaged);
    };
    // Sizes are checked before anything is gathered.
    if control_count > MAX_FILE_SPLINE_POINTS
        || fit_count > MAX_FILE_SPLINE_POINTS
        || knot_count > MAX_FILE_SPLINE_POINTS + 26
    {
        return Err(Reading::TooLarge);
    }
    let scalars = knot_count + 3 * (control_count + fit_count);
    if scalars > *budget {
        return Err(Reading::TooLarge);
    }
    *budget -= scalars;
    if control_count == 0 && fit_count == 0 {
        return Err(Reading::Damaged);
    }
    // Fit points alone are fitted only as a cubic.
    if control_count == 0 && (degree != 3 || rational) {
        return Err(Reading::FitOnly);
    }
    let normal = [210, 220, 230].map(|c| match record.text(c) {
        None => Some(if c == 230 { 1.0 } else { 0.0 }),
        Some(_) => record.number(c),
    });
    let [Some(nx), Some(ny), Some(nz)] = normal else {
        return Err(Reading::Damaged);
    };
    let length = (nx * nx + ny * ny + nz * nz).sqrt();
    if !(length.is_finite() && length > 0.0) {
        return Err(Reading::Damaged);
    }
    if nx.abs() > FLAT_TOLERANCE * length || ny.abs() > FLAT_TOLERANCE * length {
        return Err(Reading::NonPlanar);
    }
    if control_count == 0 {
        return fit_only(record, fit_count, closed || periodic);
    }
    let knots = numbers(record, 40).ok_or(Reading::Damaged)?;
    if knots.len() != knot_count {
        return Err(Reading::Damaged);
    }
    if periodic && knot_count != control_count + usize::from(degree) + 1 {
        return Err(Reading::PeriodicShortForm);
    }
    let (control_points, height) = points(record, 10, control_count)?;
    let (fit_points, fit_height) = points(record, 11, fit_count)?;
    if let (Some(a), Some(b)) = (height, fit_height) {
        if (a - b).abs() > FLAT_TOLERANCE * a.abs().max(b.abs()).max(1.0) {
            return Err(Reading::NonPlanar);
        }
    }
    let weights = numbers(record, 41).ok_or(Reading::Damaged)?;
    let weights = match (rational, weights.is_empty()) {
        (false, true) => None,
        (true, false) if weights.len() == control_count => {
            weights.iter().any(|w| *w != 1.0).then_some(weights)
        }
        _ => return Err(Reading::Damaged),
    };
    let tolerance = |code: i32, default: f64| match record.text(code) {
        None => Some(default),
        Some(_) => record.number(code).filter(|t| *t >= 0.0),
    };
    let (Some(knot_tolerance), Some(control_point_tolerance), Some(fit_tolerance)) = (
        tolerance(42, od_core::spline::DEFAULT_KNOT_TOLERANCE),
        tolerance(43, od_core::spline::DEFAULT_CONTROL_TOLERANCE),
        tolerance(44, od_core::spline::DEFAULT_FIT_TOLERANCE),
    ) else {
        return Err(Reading::Damaged);
    };
    let (start_tangent, end_tangent) = (tangent(record, 12)?, tangent(record, 13)?);
    let fit = if fit_points.is_empty() {
        if start_tangent.is_some() || end_tangent.is_some() {
            return Err(Reading::Damaged);
        }
        None
    } else {
        Some(SplineFitData {
            points: fit_points,
            tolerance: fit_tolerance,
            start_tangent,
            end_tangent,
            parameterization: None,
        })
    };
    let spline = Spline {
        degree,
        control_points,
        knots,
        weights,
        closed: closed || periodic,
        periodic,
        knot_tolerance,
        control_point_tolerance,
        fit,
    };
    spline.validate().map_err(|_| Reading::Damaged)?;
    Ok(spline)
}

impl Importer {
    pub(super) fn spline(&mut self, record: &Record) {
        let reading = read(record, &mut self.spline_budget)
            .map_or_else(|why| why, |s| Reading::Spline(Box::new(s)));
        match reading {
            Reading::Spline(s) => self.add(record, Entity::Spline(s)),
            Reading::NonPlanar => self.skip("SPLINE (not flat in the drawing plane)"),
            Reading::PeriodicShortForm => self.skip("SPLINE (periodic, short knot form)"),
            Reading::FitOnly => self.skip("SPLINE (fit points without control points)"),
            Reading::TooLarge => self.skip("SPLINE (too large)"),
            Reading::Damaged => self.damaged += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Entity, Point, Spline};

    /// A DXF with the records given (each a list of groups) in ENTITIES,
    /// and the splines and warnings reading it gives.
    fn read_records(records: &[&[(i32, String)]]) -> (Vec<Spline>, Vec<String>) {
        let mut text = String::from("0\nSECTION\n2\nENTITIES\n");
        for groups in records {
            text.push_str("0\nSPLINE\n8\n0\n");
            for (code, value) in *groups {
                text.push_str(&format!("{code}\n{value}\n"));
            }
        }
        text.push_str("0\nENDSEC\n0\nEOF\n");
        let loaded = super::super::read(text.as_bytes()).unwrap();
        let found = loaded
            .document
            .objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::Spline(s) => Some((**s).clone()),
                _ => None,
            })
            .collect();
        (found, loaded.warnings)
    }

    fn read(groups: &[(i32, String)]) -> (Vec<Spline>, Vec<String>) {
        read_records(&[groups])
    }

    /// A quadratic with three control points, in AutoCAD's order.
    const PLAIN: &[(i32, &str)] = &[
        (100, "AcDbSpline"),
        (210, "0"),
        (220, "0"),
        (230, "1"),
        (70, "8"),
        (71, "2"),
        (72, "6"),
        (73, "3"),
        (74, "0"),
        (40, "0"),
        (40, "0"),
        (40, "0"),
        (40, "1"),
        (40, "1"),
        (40, "1"),
        (10, "0"),
        (20, "0"),
        (30, "0"),
        (10, "1"),
        (20, "2"),
        (30, "0"),
        (10, "3"),
        (20, "0"),
        (30, "0"),
    ];

    /// `PLAIN` without the codes in `dropped`, with `added` after it.
    fn with(added: &[(i32, &str)], dropped: &[i32]) -> Vec<(i32, String)> {
        PLAIN
            .iter()
            .filter(|(c, _)| !dropped.contains(c))
            .chain(added)
            .map(|(c, v)| (*c, (*v).to_owned()))
            .collect()
    }

    const DAMAGED: [&str; 1] = ["Skipped 1 damaged objects."];

    #[test]
    fn a_spline_reads_exactly_in_any_group_order_with_default_tolerances() {
        let (found, warnings) = read(&with(&[], &[]));
        assert!(warnings.is_empty(), "{warnings:?}");
        let s = &found[0];
        assert_eq!(s.degree, 2);
        assert_eq!(s.knots, [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        let p = Point::new;
        assert_eq!(s.control_points, [p(0.0, 0.0), p(1.0, 2.0), p(3.0, 0.0)]);
        assert_eq!(
            (s.weights.clone(), s.closed, s.fit.clone()),
            (None, false, None)
        );
        assert_eq!(s.knot_tolerance, 1e-7);
        // Grouped by code (every x, then every y...), the values of each
        // code in their order: the points still pair up.
        let mut sorted = with(&[], &[]);
        sorted.sort_by_key(|(code, _)| std::cmp::Reverse(*code));
        let (again, w) = read(&sorted);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(again[0], found[0]);
        // Without a normal or heights it is still flat.
        let (bare, warnings) = read(&with(&[], &[30, 210, 220, 230]));
        assert!(warnings.is_empty() && bare.len() == 1, "{warnings:?}");
    }

    #[test]
    fn weights_go_with_the_rational_flag() {
        let weights = [(70, "12"), (41, "1"), (41, "0.5"), (41, "1")];
        let (found, warnings) = read(&with(&weights, &[70]));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(found[0].weights, Some(vec![1.0, 0.5, 1.0]));
        // Rational, all weights 1: a plain spline.
        let ones = [(70, "12"), (41, "1"), (41, "1"), (41, "1")];
        assert_eq!(read(&with(&ones, &[70])).0[0].weights, None);
        for bad in [
            vec![(70, "12")],
            vec![(41, "1"), (41, "0.5"), (41, "1")],
            vec![(70, "12"), (41, "1"), (41, "0.5")],
            vec![(70, "12"), (41, "1"), (41, "-1"), (41, "1")],
        ] {
            let dropped = if bad[0].0 == 70 { vec![70] } else { vec![] };
            let (found, warnings) = read(&with(&bad, &dropped));
            assert!(found.is_empty(), "{bad:?}");
            assert_eq!(warnings, DAMAGED, "{bad:?}");
        }
    }

    #[test]
    fn counts_that_do_not_match_and_bad_values_are_damaged() {
        for (added, dropped) in [
            (vec![(72, "7")], vec![72]),
            (vec![(73, "4")], vec![73]),
            (vec![(74, "1")], vec![74]),
            (vec![(40, "0.5")], vec![]),
            (vec![(10, "x")], vec![]),
            (vec![(71, "2")], vec![]),
            (vec![], vec![71]),
            (vec![(40, "nan")], vec![]),
            (vec![(12, "1"), (22, "0")], vec![]),
            (vec![(42, "-1")], vec![]),
        ] {
            let (found, warnings) = read(&with(&added, &dropped));
            assert!(found.is_empty(), "{added:?}");
            assert_eq!(warnings, DAMAGED, "{added:?}");
        }
    }

    #[test]
    fn a_huge_declared_count_is_refused_before_anything_is_gathered() {
        let (found, warnings) = read(&with(&[(73, "100000000")], &[73]));
        assert!(found.is_empty());
        assert_eq!(
            warnings,
            ["Skipped 1 unsupported object: 1 SPLINE (too large)."]
        );
    }

    #[test]
    fn a_tilted_or_uneven_spline_is_skipped_and_one_seen_from_below_is_kept() {
        for (added, dropped) in [
            (vec![(210, "0.5"), (230, "0.8")], vec![210, 230]),
            (vec![(30, "5")], vec![30]),
        ] {
            let mut groups = with(&added, &dropped);
            if dropped == [30] {
                // Heights 5, 0, 0: not one plane.
                groups.extend([(30, "0".to_owned()), (30, "0".to_owned())]);
            }
            let (found, warnings) = read(&groups);
            assert!(found.is_empty());
            assert_eq!(
                warnings,
                ["Skipped 1 unsupported object: 1 SPLINE (not flat in the drawing plane)."]
            );
        }
        // Points are world coordinates, so -Z changes nothing.
        let (found, warnings) = read(&with(&[(230, "-1")], &[230]));
        assert!(warnings.is_empty());
        assert_eq!(found[0], read(&with(&[], &[])).0[0]);
    }

    #[test]
    fn fit_data_and_tangents_come_through_and_stray_tangents_are_damaged() {
        let fit = [
            (74, "2"),
            (44, "0.5"),
            (11, "0"),
            (21, "0"),
            (31, "0"),
            (11, "3"),
            (21, "0"),
            (31, "0"),
            (12, "1"),
            (22, "1"),
            (32, "0"),
            (13, "0"),
            (23, "-1"),
        ];
        let (found, warnings) = read(&with(&fit, &[74]));
        assert!(warnings.is_empty(), "{warnings:?}");
        let data = found[0].fit.clone().unwrap();
        let p = Point::new;
        assert_eq!(data.points, [p(0.0, 0.0), p(3.0, 0.0)]);
        assert_eq!(data.tolerance, 0.5);
        assert_eq!(
            (data.start_tangent, data.end_tangent),
            (Some(p(1.0, 1.0)), Some(p(0.0, -1.0)))
        );
        assert_eq!(data.parameterization, None);
        let (found, warnings) = read(&with(&[(12, "1"), (22, "1")], &[]));
        assert!(found.is_empty());
        assert_eq!(warnings, DAMAGED);
    }

    #[test]
    fn short_periodic_knots_and_fit_points_alone_are_skipped_by_name() {
        // Periodic with as many knots as control points plus one.
        let short = [
            (70, "11"),
            (72, "4"),
            (40, "0"),
            (40, "1"),
            (40, "2"),
            (40, "3"),
        ];
        let (found, warnings) = read(&with(&short, &[70, 72, 40]));
        assert!(found.is_empty());
        assert_eq!(
            warnings,
            ["Skipped 1 unsupported object: 1 SPLINE (periodic, short knot form)."]
        );
        let alone = [
            (71, "3"),
            (73, "0"),
            (72, "0"),
            (74, "2"),
            (11, "0"),
            (21, "0"),
            (11, "1"),
            (21, "1"),
        ];
        // Fit points alone are fitted, through the points.
        let (found, warnings) = read(&with(&alone, &[71, 73, 72, 74, 40, 10, 20, 30]));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(found.len(), 1);
        assert!(found[0].distance_to(Point::new(1.0, 1.0)) < 1e-9);
        assert!(found[0].fit.is_some());
        // With an end tangent they are not.
        let mut tangent = alone.to_vec();
        tangent.extend([(12, "1"), (22, "0"), (32, "0")]);
        let (found, warnings) = read(&with(&tangent, &[71, 73, 72, 74, 40, 10, 20, 30]));
        assert!(found.is_empty());
        assert_eq!(
            warnings,
            ["Skipped 1 unsupported object: 1 SPLINE (fit points without control points)."]
        );
    }

    #[test]
    fn the_file_wide_budget_stops_reading_splines_past_it() {
        // Each record declares 100,000 control points but has three: the
        // first six fit the budget (and are damaged), the last two do not.
        let big = with(&[(73, "100000")], &[73]);
        let records: Vec<&[(i32, String)]> = (0..8).map(|_| big.as_slice()).collect();
        let (found, warnings) = read_records(&records);
        assert!(found.is_empty());
        assert!(
            warnings.iter().any(|w| w.contains("2 SPLINE (too large)")),
            "{warnings:?}"
        );
    }
}
