//! ELLIPSE records, in ENTITIES and in BLOCKS alike.
//!
//! The centre (10/20) and the major axis (11/21) are world coordinates even
//! though an ellipse has an extrusion (210/220/230), and the parameters
//! (41/42) run counter-clockwise seen down that extrusion. Along +Z that is
//! the drawing's own sense; along -Z the curve runs the other way, so the arc
//! from `s` to `e` is the drawing's arc from `-e` to `-s`. A tilted extrusion,
//! or a major axis that leaves the drawing plane, is skipped as non-planar.
//! Elevation is dropped, as for every other DXF object.

use std::f64::consts::TAU;

use od_core::{Ellipse, Entity};

use super::{Importer, Record};

/// How much the extrusion may differ from the Z axis and still count as
/// along it (as in the DWG reader).
const AXIS_TOLERANCE: f64 = 1e-9;

/// Codes an ellipse must have exactly once.
const REQUIRED: [i32; 5] = [10, 20, 11, 21, 40];
/// Codes it may leave out (the parameters of a full ellipse) but not repeat.
const OPTIONAL: [i32; 5] = [41, 42, 210, 220, 230];

enum Reading {
    Ellipse(Ellipse),
    NonPlanar,
    Damaged,
}

fn count(record: &Record, code: i32) -> usize {
    record.groups.iter().filter(|g| g.code == code).count()
}

fn read(record: &Record) -> Reading {
    let once = REQUIRED.iter().all(|c| count(record, *c) == 1);
    if !once || OPTIONAL.iter().any(|c| count(record, *c) > 1) {
        return Reading::Damaged;
    }
    let (Some(center), Some(axis), Some(ratio)) = (
        Importer::point(record, 10, 20),
        Importer::point(record, 11, 21),
        record.number(40),
    ) else {
        return Reading::Damaged;
    };
    let number = |code: i32, default: f64| match record.text(code) {
        Some(_) => record.number(code),
        None => Some(default),
    };
    let (Some(start), Some(end), Some(axis_z)) =
        (number(41, 0.0), number(42, TAU), number(31, 0.0))
    else {
        return Reading::Damaged;
    };
    let normal = [number(210, 0.0), number(220, 0.0), number(230, 1.0)];
    let [Some(nx), Some(ny), Some(nz)] = normal else {
        return Reading::Damaged;
    };
    let length = (nx * nx + ny * ny + nz * nz).sqrt();
    if !length.is_finite() || length == 0.0 {
        return Reading::Damaged;
    }
    let size = axis.x.hypot(axis.y);
    let flat = axis_z.abs() <= AXIS_TOLERANCE * size.max(1.0);
    if (nx / length).abs() > AXIS_TOLERANCE || (ny / length).abs() > AXIS_TOLERANCE || !flat {
        return Reading::NonPlanar;
    }
    let (start, end) = if nz > 0.0 {
        (start, end)
    } else {
        (-end, -start)
    };
    match Ellipse::new(center, axis, ratio, start, end) {
        Ok(e) => Reading::Ellipse(e),
        Err(_) => Reading::Damaged,
    }
}

impl Importer {
    pub(super) fn ellipse(&mut self, record: &Record) {
        match read(record) {
            Reading::Ellipse(e) => self.add(record, Entity::Ellipse(e)),
            Reading::NonPlanar => self.skip("ELLIPSE (not flat in the drawing plane)"),
            Reading::Damaged => self.damaged += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use od_core::{Ellipse, Entity, Point};

    /// A DXF with one ELLIPSE made of `groups`, and what reading it warned.
    fn read(groups: &[(i32, &str)]) -> (Vec<Ellipse>, Vec<String>) {
        let mut text = String::from("0\nSECTION\n2\nENTITIES\n0\nELLIPSE\n8\n0\n");
        for (code, value) in groups {
            text.push_str(&format!("{code}\n{value}\n"));
        }
        text.push_str("0\nENDSEC\n0\nEOF\n");
        let loaded = super::super::read(text.as_bytes()).unwrap();
        let found = loaded
            .document
            .objects()
            .iter()
            .filter_map(|o| match o.entity {
                Entity::Ellipse(e) => Some(e),
                _ => None,
            })
            .collect();
        (found, loaded.warnings)
    }

    const FULL: &[(i32, &str)] = &[
        (10, "1"),
        (20, "2"),
        (30, "0"),
        (11, "4"),
        (21, "0"),
        (31, "0"),
        (40, "0.5"),
    ];

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).hypot(a.y - b.y) < 1e-9
    }

    #[test]
    fn a_full_ellipse_needs_no_parameters() {
        let (found, warnings) = read(FULL);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(found.len(), 1);
        let e = found[0];
        assert!(e.is_full());
        assert_eq!(
            (e.center, e.major_axis, e.radius_ratio),
            (Point::new(1.0, 2.0), Point::new(4.0, 0.0), 0.5)
        );
    }

    #[test]
    fn an_arc_keeps_its_parameters_in_any_group_order() {
        let mut groups = vec![(42, "3.14159265358979"), (41, "1.5707963267949")];
        groups.extend_from_slice(FULL);
        let (found, _) = read(&groups);
        let e = found[0];
        assert!((e.start - FRAC_PI_2).abs() < 1e-12 && (e.end - PI).abs() < 1e-12);
        assert!(near(e.start_point(), Point::new(1.0, 4.0)));
    }

    #[test]
    fn a_downward_extrusion_runs_the_other_way_round() {
        let mut groups = FULL.to_vec();
        groups.extend([
            (41, "0"),
            (42, "1.5707963267949"),
            (210, "0"),
            (220, "0"),
            (230, "-1"),
        ]);
        let (found, _) = read(&groups);
        let e = found[0];
        // Seen from below, the quarter from the major end turns clockwise.
        assert!(
            near(e.start_point(), Point::new(1.0, 0.0)),
            "{:?}",
            e.start_point()
        );
        assert!(near(e.end_point(), Point::new(5.0, 2.0)));
        assert!((e.sweep() - FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn damaged_and_tilted_ellipses_are_counted_not_loaded() {
        let tilted: Vec<(i32, &str)> = FULL
            .iter()
            .copied()
            .chain([(210, "0"), (220, "1"), (230, "1")])
            .collect();
        let (found, warnings) = read(&tilted);
        assert!(found.is_empty());
        assert!(warnings[0].contains("ELLIPSE (not flat"), "{warnings:?}");

        let bad = |change: (i32, &'static str)| -> Vec<(i32, &'static str)> {
            FULL.iter()
                .map(|&(c, v)| if c == change.0 { change } else { (c, v) })
                .collect()
        };
        for groups in [
            bad((40, "1.5")),
            bad((40, "0")),
            bad((11, "0"))
                .into_iter()
                .map(|g| if g.0 == 21 { (21, "0") } else { g })
                .collect(),
            bad((10, "nan")),
            FULL[1..].to_vec(),
            FULL.iter().copied().chain([(40, "0.25")]).collect(),
            FULL.iter().copied().chain([(41, "1"), (42, "1")]).collect(),
        ] {
            let (found, warnings) = read(&groups);
            assert!(found.is_empty(), "{groups:?}");
            assert!(
                warnings.iter().any(|w| w.contains("damaged")),
                "{warnings:?}"
            );
        }
        let full_turn: Vec<(i32, &str)> = FULL
            .iter()
            .copied()
            .chain([(41, "0"), (42, "6.283185307179586")])
            .collect();
        assert!(read(&full_turn).0[0].is_full());
    }
}
