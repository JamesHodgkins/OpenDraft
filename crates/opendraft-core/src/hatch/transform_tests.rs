use std::f64::consts::TAU;

use super::testing::*;

fn same_strokes(a: &EvaluatedHatch, b: &EvaluatedHatch) -> bool {
    let key = |r: &EvaluatedHatch| {
        let mut v: Vec<[i64; 4]> = r
            .strokes
            .iter()
            .map(|s| {
                let mut ends = [
                    [(s[0].x * 1e5).round() as i64, (s[0].y * 1e5).round() as i64],
                    [(s[1].x * 1e5).round() as i64, (s[1].y * 1e5).round() as i64],
                ];
                ends.sort_unstable();
                [ends[0][0], ends[0][1], ends[1][0], ends[1][1]]
            })
            .collect();
        v.sort_unstable();
        v
    };
    key(a) == key(b)
}

#[test]
fn every_transform_carries_the_pattern_with_it() {
    let pattern = catalog_pattern("ANSI32", 1.0, 0.0, p(0.0, 0.0));
    let h = patterned(nested(2), pattern);
    let before = eval(&h);
    // Move: the strokes are the old ones moved.
    let moved = eval(&h.translated(7.0, -3.0));
    let shifted = EvaluatedHatch {
        strokes: before
            .strokes
            .iter()
            .map(|s| [s[0].translated(7.0, -3.0), s[1].translated(7.0, -3.0)])
            .collect(),
        ..before.clone()
    };
    assert!(same_strokes(&moved, &shifted));
    // Turn and turn back; mirror and mirror back.
    let there = h.rotated(p(4.0, 1.0), 0.7);
    assert!(same_strokes(
        &eval(&there.rotated(p(4.0, 1.0), -0.7)),
        &before
    ));
    let (a, b) = (p(1.0, 2.0), p(5.0, 3.0));
    let flipped = h.mirrored(a, b);
    assert!(same_strokes(&eval(&flipped.mirrored(a, b)), &before));
    // Scale up and back down.
    let big = h.scaled(p(2.0, 2.0), 2.0);
    assert!((eval(&big).area - 4.0 * before.area).abs() < 1e-6);
    assert!(same_strokes(&eval(&big.scaled(p(2.0, 2.0), 0.5)), &before));
    // Uneven scale is refused.
    assert!(h.transformed(&crate::Affine::scaling(2.0, 1.0)).is_err());
}

#[test]
fn mirroring_keeps_arcs_and_ellipses_on_the_same_curve() {
    let loops = vec![
        HatchLoop {
            flags: HatchLoopFlags::default(),
            boundary: HatchBoundary::Edges(vec![HatchEdge::EllipticArc {
                center: p(0.0, 0.0),
                major_axis: p(6.0, 2.0),
                minor_major_ratio: 0.4,
                start_param: 0.3,
                end_param: 0.3 + TAU - 1e-9,
                counter_clockwise: true,
            }]),
            sources: Vec::new(),
        },
        HatchLoop {
            flags: HatchLoopFlags::default(),
            boundary: HatchBoundary::Polyline(HatchPolyline {
                vertices: vec![p(10.0, 0.0), p(14.0, 0.0), p(14.0, 4.0)],
                bulges: vec![0.0, 0.5, 0.0],
                closed: true,
            }),
            sources: Vec::new(),
        },
    ];
    let h = solid(loops, HatchStyle::Normal);
    let before = eval(&h);
    let (a, b) = (p(-3.0, 8.0), p(7.0, -5.0));
    let image = h.mirrored(a, b);
    let after = eval(&image);
    assert!(
        (before.area - after.area).abs() < 1e-3,
        "{} {}",
        before.area,
        after.area
    );
    // Every flattened point of the original lands on the image's outline.
    for c in &before.fill {
        for q in c {
            let r = q.mirrored(a, b);
            let near = after
                .fill
                .iter()
                .flat_map(|c| c.iter())
                .map(|w| (w.x - r.x).hypot(w.y - r.y))
                .fold(f64::INFINITY, f64::min);
            assert!(near < 0.1, "{near}");
        }
    }
}
