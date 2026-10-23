use od_core::hatch::{BoundarySource, HatchBoundary, HatchEdge, HatchEntity, HatchFill};
use od_core::{Entity, Point};

use crate::dxf::read::read;

fn file(entities: &[(i32, String)]) -> Vec<u8> {
    let mut text = String::from("0\nSECTION\n2\nENTITIES\n");
    for (code, value) in entities {
        text.push_str(&format!("{code}\n{value}\n"));
    }
    text.push_str("0\nENDSEC\n0\nEOF\n");
    text.into_bytes()
}

type Groups = Vec<(i32, String)>;

fn g(items: &[(i32, &str)]) -> Groups {
    items.iter().map(|(c, v)| (*c, (*v).to_owned())).collect()
}

/// A HATCH record: the head, the given loops and the tail.
fn hatch(head: &[(i32, &str)], loops: &[(i32, &str)], count: &str, tail: &[(i32, &str)]) -> Groups {
    let mut all = g(&[(0, "HATCH"), (8, "0"), (100, "AcDbHatch")]);
    all.extend(g(head));
    all.extend(g(&[(91, count)]));
    all.extend(g(loops));
    all.extend(g(tail));
    all
}

const HEAD: &[(i32, &str)] = &[
    (10, "0"),
    (20, "0"),
    (30, "0"),
    (210, "0"),
    (220, "0"),
    (230, "1"),
    (2, "SOLID"),
    (70, "1"),
    (71, "0"),
];

const SQUARE: &[(i32, &str)] = &[
    (92, "7"),
    (72, "0"),
    (73, "1"),
    (93, "4"),
    (10, "0"),
    (20, "0"),
    (10, "4"),
    (20, "0"),
    (10, "4"),
    (20, "4"),
    (10, "0"),
    (20, "4"),
    (97, "0"),
];

const TAIL: &[(i32, &str)] = &[(75, "0"), (76, "1"), (47, "0.1"), (98, "0")];

fn read_all(entities: &Groups) -> (Vec<HatchEntity>, Vec<String>) {
    let loaded = read(&file(entities)).unwrap();
    let found = loaded
        .document
        .objects()
        .iter()
        .filter_map(|o| match &o.entity {
            Entity::Hatch(h) => Some((**h).clone()),
            _ => None,
        })
        .collect();
    (found, loaded.warnings)
}

fn area(h: &HatchEntity) -> f64 {
    h.evaluate(1e-4, &Default::default()).unwrap().area
}

#[test]
fn a_solid_square_reads_with_its_loop_flags_and_a_gradient_tail_of_kind_zero() {
    let mut tail = TAIL.to_vec();
    tail.extend([
        (450, "0"),
        (451, "0"),
        (460, "0.0"),
        (461, "0.0"),
        (452, "0"),
        (462, "0.0"),
        (453, "2"),
        (463, "0.0"),
        (63, "5"),
        (421, "255"),
        (463, "1.0"),
        (63, "2"),
        (421, "16776960"),
        (470, "LINEAR"),
    ]);
    let (found, warnings) = read_all(&hatch(HEAD, SQUARE, "1", &tail));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(found[0].fill, HatchFill::Solid);
    assert!((area(&found[0]) - 16.0).abs() < 1e-9);
    assert_eq!(found[0].loops[0].flags.0, 7);
    assert_eq!(found[0].source_pixel_size, Some(0.1));
}

#[test]
fn a_gradient_is_kept_with_its_colours_and_raw_name() {
    let mut tail = TAIL.to_vec();
    tail.extend([
        (450, "1"),
        (451, "0"),
        (460, "0.5"),
        (461, "0.25"),
        (452, "0"),
        (462, "0.5"),
        (453, "2"),
        (463, "0.0"),
        (63, "5"),
        (421, "255"),
        (463, "1.0"),
        (63, "2"),
        (421, "16776960"),
        (470, "CYLINDER"),
    ]);
    let (found, _) = read_all(&hatch(HEAD, SQUARE, "1", &tail));
    let HatchFill::Gradient(grad) = &found[0].fill else {
        panic!("gradient expected");
    };
    assert_eq!(grad.name, "CYLINDER");
    assert_eq!((grad.angle, grad.shift, grad.tint), (0.5, 0.25, 0.5));
    assert_eq!(grad.colors, [(0.0, [0, 0, 255]), (1.0, [255, 255, 0])]);
}

#[test]
fn a_clockwise_arc_edge_has_its_angles_measured_clockwise() {
    // The upper half circle, run clockwise from (-2,0) over the top to
    // (2,0), then the chord back. AutoCAD stores the angles mirrored.
    let loops = [
        (92, "1"),
        (93, "2"),
        (72, "2"),
        (10, "0"),
        (20, "0"),
        (40, "2"),
        (50, "180"),
        (51, "360"),
        (73, "0"),
        (72, "1"),
        (10, "2"),
        (20, "0"),
        (11, "-2"),
        (21, "0"),
        (97, "0"),
    ];
    let (found, warnings) = read_all(&hatch(HEAD, &loops, "1", TAIL));
    assert!(warnings.is_empty(), "{warnings:?}");
    let HatchBoundary::Edges(edges) = &found[0].loops[0].boundary else {
        panic!("edges expected");
    };
    let HatchEdge::CircularArc {
        start_angle,
        end_angle,
        counter_clockwise,
        ..
    } = edges[0]
    else {
        panic!("arc expected");
    };
    assert!(!counter_clockwise);
    assert!((start_angle - std::f64::consts::PI).abs() < 1e-12);
    assert!(end_angle.abs() < 1e-12 || (end_angle - std::f64::consts::TAU).abs() < 1e-12);
    let half_disc = std::f64::consts::PI * 2.0;
    assert!((area(&found[0]) - half_disc).abs() < 1e-3);
}

#[test]
fn a_spline_edge_reads_with_or_without_a_fit_count_before_the_sources() {
    let edge = |with_fit_count: bool| {
        let mut loops = g(&[
            (92, "5"),
            (93, "1"),
            (72, "4"),
            (94, "1"),
            (73, "0"),
            (74, "0"),
            (95, "5"),
            (96, "3"),
            (40, "0"),
            (40, "0"),
            (40, "1"),
            (40, "2"),
            (40, "2"),
            (10, "0"),
            (20, "0"),
            (10, "4"),
            (20, "0"),
            (10, "0"),
            (20, "0"),
        ]);
        if with_fit_count {
            loops.extend(g(&[(97, "0")]));
        }
        loops.extend(g(&[(97, "1"), (330, "27E")]));
        loops
    };
    for with in [true, false] {
        let loops = edge(with);
        let loops: Vec<(i32, &str)> = loops.iter().map(|(c, v)| (*c, v.as_str())).collect();
        let (found, warnings) = read_all(&hatch(HEAD, &loops, "1", TAIL));
        // A degenerate spline (a line there and back) reads; it just has no
        // area. What matters is that the sources are found.
        assert!(
            warnings.is_empty() || found.is_empty(),
            "{with}: {warnings:?}"
        );
        if let Some(h) = found.first() {
            assert_eq!(
                h.loops[0].sources,
                [BoundarySource::External {
                    scheme: "acad-handle".into(),
                    value: "27E".into()
                }],
                "{with}"
            );
        }
    }
}

#[test]
fn sources_are_kept_as_unresolved_handles() {
    let mut loops = SQUARE.to_vec();
    loops.pop();
    loops.extend([(97, "2"), (330, "A1"), (330, "B2")]);
    let (found, _) = read_all(&hatch(HEAD, &loops, "1", TAIL));
    assert_eq!(found[0].loops[0].sources.len(), 2);
}

#[test]
fn a_hatch_facing_down_or_tilted_is_skipped_by_name() {
    for normal in [("210", "0"), ("230", "-1")] {
        let head: Vec<(i32, &str)> = HEAD
            .iter()
            .map(|&(c, v)| {
                if c.to_string() == normal.0 {
                    (c, normal.1)
                } else {
                    (c, v)
                }
            })
            .collect();
        let (found, warnings) = read_all(&hatch(&head, SQUARE, "1", TAIL));
        if normal.0 == "230" {
            assert!(found.is_empty());
            assert_eq!(
                warnings,
                ["Skipped 1 unsupported object: 1 HATCH (not facing up in the drawing plane)."]
            );
        } else {
            assert_eq!(found.len(), 1);
        }
    }
}

#[test]
fn counts_that_lie_are_damaged_or_too_large_and_never_panic() {
    // More vertices than there are groups.
    let mut loops = SQUARE.to_vec();
    loops[3] = (93, "400000");
    let (found, warnings) = read_all(&hatch(HEAD, &loops, "1", TAIL));
    assert!(found.is_empty());
    assert_eq!(warnings, ["Skipped 1 damaged objects."]);
    // A loop count beyond the limit.
    let (found, warnings) = read_all(&hatch(HEAD, SQUARE, "99999999", TAIL));
    assert!(found.is_empty());
    assert_eq!(
        warnings,
        ["Skipped 1 unsupported object: 1 HATCH (too large)."]
    );
    // A negative count and a count that is not a number.
    for bad in ["-1", "x", ""] {
        let (found, _) = read_all(&hatch(HEAD, SQUARE, bad, TAIL));
        assert!(found.is_empty(), "{bad}");
    }
    // A bad edge type.
    let loops = [(92, "1"), (93, "1"), (72, "9"), (97, "0")];
    let (found, _) = read_all(&hatch(HEAD, &loops, "1", TAIL));
    assert!(found.is_empty());
}

#[test]
fn a_record_cut_short_anywhere_is_never_a_panic() {
    let full = hatch(HEAD, SQUARE, "1", TAIL);
    for keep in 0..=full.len() {
        let cut: Groups = full[..keep].to_vec();
        let _ = read(&file(&cut));
    }
    // Numbers that are not numbers, in every place.
    for at in 3..full.len() {
        let mut broken = full.clone();
        broken[at].1 = "nan".to_owned();
        let _ = read(&file(&broken));
        broken[at].1 = "1e999".to_owned();
        let _ = read(&file(&broken));
    }
}

#[test]
fn a_hatch_inside_a_block_is_skipped_by_name() {
    let mut text = String::from("0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\nB\n70\n0\n10\n0\n20\n0\n");
    for (code, value) in hatch(HEAD, SQUARE, "1", TAIL) {
        text.push_str(&format!("{code}\n{value}\n"));
    }
    text.push_str("0\nENDBLK\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n");
    let loaded = read(text.as_bytes()).unwrap();
    assert!(loaded
        .warnings
        .iter()
        .any(|w| w.contains("1 HATCH (inside a block)")));
    loaded.document.validate().unwrap();
    let _ = Point::new(0.0, 0.0);
}
