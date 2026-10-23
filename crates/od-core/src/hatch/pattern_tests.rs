use std::f64::consts::{FRAC_PI_2, PI};

use super::testing::*;

#[test]
fn ansi31_from_the_definition_matches_what_autocad_stores() {
    // HatchTest.dxf: ANSI31, scale 10, at 0 and 45 degrees.
    let flat = catalog_pattern("ANSI31", 10.0, 0.0, p(0.0, 0.0));
    let l = &flat.lines[0];
    assert!((l.angle - PI / 4.0).abs() < 1e-12);
    assert!((l.offset[0] + 0.8838834764831844).abs() < 1e-12);
    assert!((l.offset[1] - 0.8838834764831844).abs() < 1e-12);
    let turned = catalog_pattern("ANSI31", 10.0, PI / 4.0, p(0.0, 0.0));
    let l = &turned.lines[0];
    assert!((l.angle - FRAC_PI_2).abs() < 1e-12);
    assert!((l.offset[0] + 1.25).abs() < 1e-12 && l.offset[1].abs() < 1e-12);
}

#[test]
fn horizontal_lines_over_a_square_add_up_to_area_over_spacing() {
    let pattern = catalog_pattern("LINE", 8.0, 0.0, p(0.0, 0.0));
    let h = patterned(vec![square(0.5, 0.5, 20.0)], pattern);
    let result = eval(&h);
    // Spacing 1; lines at y = 1..=20 across a width of 20.
    assert_eq!(result.strokes.len(), 20);
    assert!((length(&result.strokes) - 400.0).abs() < 1e-9);
}

#[test]
fn a_line_through_a_vertex_or_along_an_edge_is_not_counted_twice() {
    // A diamond whose corners sit exactly on pattern lines (spacing 1).
    let diamond = HatchLoop {
        flags: HatchLoopFlags::default(),
        boundary: HatchBoundary::Polyline(HatchPolyline {
            vertices: vec![p(0.0, 3.0), p(3.0, 0.0), p(6.0, 3.0), p(3.0, 6.0)],
            bulges: Vec::new(),
            closed: true,
        }),
        sources: Vec::new(),
    };
    let pattern = catalog_pattern("LINE", 8.0, 0.0, p(0.0, 0.0));
    let result = eval(&patterned(vec![diamond], pattern));
    // The area is 18; with unit spacing the strokes add up to about it.
    let total = length(&result.strokes);
    assert!((total - 18.0).abs() < 2.0, "{total}");
    // No stroke runs outside the diamond.
    for s in &result.strokes {
        let mid = p((s[0].x + s[1].x) / 2.0, (s[0].y + s[1].y) / 2.0);
        let inside = (mid.x - 3.0).abs() + (mid.y - 3.0).abs() <= 3.0 + 1e-9;
        assert!(inside, "{s:?}");
    }
}

#[test]
fn an_island_cuts_the_lines_and_the_dashes_keep_their_phase() {
    // A dashed horizontal family: 3 on, 1 off, spacing 1.
    let def = PatternCatalog::parse("*D, dashed\n0, 0,0, 0,1, 3,-1\n");
    let pattern = HatchPattern::from_definition(
        def.find("D").unwrap(),
        PatternType::Custom,
        1.0,
        0.0,
        p(0.0, 0.0),
    )
    .unwrap();
    let mut h = patterned(vec![square(0.0, 0.0, 20.0), square(6.5, 6.0, 7.0)], pattern);
    h.style = HatchStyle::Normal;
    let result = eval(&h);
    // On the line y = 10 (through the island) every stroke starts at a
    // multiple of 4 or at the island's edge, and ends at a multiple of 4 plus
    // 3 or at the island's edge.
    let row: Vec<&[Point; 2]> = result
        .strokes
        .iter()
        .filter(|s| (s[0].y - 10.0).abs() < 1e-9)
        .collect();
    assert!(!row.is_empty());
    for s in &row {
        let (a, b) = (s[0].x.min(s[1].x), s[0].x.max(s[1].x));
        let starts_ok = a.rem_euclid(4.0) < 1e-9 || (a - 13.5).abs() < 1e-9 || a.abs() < 1e-9;
        let ends_ok =
            (b - 3.0).rem_euclid(4.0) < 1e-9 || (b - 6.5).abs() < 1e-9 || (b - 20.0).abs() < 1e-9;
        assert!(starts_ok && ends_ok, "{s:?}");
        // Nothing inside the island.
        assert!(!(a > 6.5 + 1e-9 && b < 13.5 - 1e-9), "{s:?}");
    }
}

#[test]
fn the_pattern_stays_put_when_the_boundary_does_not() {
    // The same pattern over two squares: lines meet across the gap.
    let pattern = catalog_pattern("LINE", 8.0, 0.0, p(0.0, 0.0));
    let left = eval(&patterned(vec![square(0.0, 0.25, 10.0)], pattern.clone()));
    let right = eval(&patterned(vec![square(20.0, 0.25, 10.0)], pattern));
    let rows = |r: &EvaluatedHatch| {
        let mut ys: Vec<i64> = r
            .strokes
            .iter()
            .map(|s| (s[0].y * 1e6).round() as i64)
            .collect();
        ys.sort_unstable();
        ys
    };
    assert_eq!(rows(&left), rows(&right));
}

#[test]
fn a_dense_pattern_is_refused_before_it_is_made() {
    let pattern = catalog_pattern("LINE", 1e-6, 0.0, p(0.0, 0.0));
    let h = patterned(vec![square(0.0, 0.0, 1000.0)], pattern);
    let r = h.evaluate(TOL, &HatchLimits::default());
    assert!(matches!(r, Err(HatchError::TooDense { .. })), "{r:?}");
    // Dashes too short for the length.
    let def = PatternCatalog::parse("*S, short\n0, 0,0, 0,1, 1e-7,-1e-7\n");
    let pattern = HatchPattern::from_definition(
        def.find("S").unwrap(),
        PatternType::Custom,
        1.0,
        0.0,
        p(0.0, 0.0),
    )
    .unwrap();
    let h = patterned(vec![square(0.0, 0.0, 100.0)], pattern);
    let r = h.evaluate(TOL, &HatchLimits::default());
    assert!(matches!(r, Err(HatchError::TooDense { .. })), "{r:?}");
}

#[test]
fn a_pattern_with_no_spacing_is_refused() {
    let def = PatternCatalog::parse("*Z, zero\n0, 0,0, 0,0\n");
    let pattern = HatchPattern::from_definition(
        def.find("Z").unwrap(),
        PatternType::Custom,
        1.0,
        0.0,
        p(0.0, 0.0),
    )
    .unwrap();
    let r = patterned(vec![square(0.0, 0.0, 10.0)], pattern).evaluate(TOL, &HatchLimits::default());
    assert_eq!(r, Err(HatchError::PatternSpacing));
}

#[test]
fn user_defined_lines_and_the_double_flag() {
    let single = HatchPattern::user_defined(2.0, 0.0, false, p(0.0, 0.0)).unwrap();
    let double = HatchPattern::user_defined(2.0, 0.0, true, p(0.0, 0.0)).unwrap();
    assert_eq!((single.lines.len(), double.lines.len()), (1, 2));
    let s = eval(&patterned(vec![square(0.0, 0.0, 10.0)], single));
    let d = eval(&patterned(vec![square(0.0, 0.0, 10.0)], double));
    // Lines at y = 0, 2, ..., 10 (those along the edges included), and a
    // second set across them.
    assert_eq!(s.strokes.len(), 6);
    assert_eq!(d.strokes.len(), 12);
    assert!(HatchPattern::user_defined(0.0, 0.0, false, p(0.0, 0.0)).is_err());
}

#[test]
fn changing_scale_and_angle_keeps_the_origin_and_matches_a_fresh_pattern() {
    let origin = p(3.0, -2.0);
    let base = catalog_pattern("ANSI32", 1.0, 0.0, origin);
    let again = base.with_scale(4.0).unwrap().with_angle(0.5).unwrap();
    let fresh = catalog_pattern("ANSI32", 4.0, 0.5, origin);
    for (a, b) in again.lines.iter().zip(&fresh.lines) {
        assert!((a.angle - b.angle).abs() < 1e-12);
        assert!((a.base.x - b.base.x).abs() < 1e-12 && (a.base.y - b.base.y).abs() < 1e-12);
        assert!((a.offset[0] - b.offset[0]).abs() < 1e-12);
        assert!((a.offset[1] - b.offset[1]).abs() < 1e-12);
    }
    assert!(base.with_scale(0.0).is_err());
    let moved = base.with_origin(p(0.0, 0.0)).unwrap();
    assert_eq!(moved.origin, p(0.0, 0.0));
    assert!((moved.lines[1].base.x - (base.lines[1].base.x - 3.0)).abs() < 1e-12);
}

#[test]
fn a_line_along_an_edge_is_drawn_on_both_sides_and_a_tangent_draws_nothing() {
    // Lines one apart over a square from 0 to 10: the lines along the
    // bottom and the top edge are both drawn (AutoCAD and ezdxf agree).
    let pattern = catalog_pattern("LINE", 8.0, 0.0, p(0.0, 0.0));
    let result = eval(&patterned(vec![square(0.0, 0.0, 10.0)], pattern.clone()));
    assert_eq!(result.strokes.len(), 11);
    assert!((length(&result.strokes) - 110.0).abs() < 1e-9);
    // The same along a vertical edge, with the pattern turned a quarter.
    let turned = catalog_pattern("LINE", 8.0, FRAC_PI_2, p(0.0, 0.0));
    let result = eval(&patterned(vec![square(0.0, 0.0, 10.0)], turned));
    assert_eq!(result.strokes.len(), 11);
    // A circle touches the lines at its top and bottom and draws nothing
    // there: nine lines cross it, not eleven.
    let circle = HatchLoop {
        flags: HatchLoopFlags::default(),
        boundary: HatchBoundary::Edges(vec![HatchEdge::CircularArc {
            center: p(5.0, 5.0),
            radius: 5.0,
            start_angle: 0.0,
            end_angle: TAU_FULL,
            counter_clockwise: true,
        }]),
        sources: Vec::new(),
    };
    let result = eval(&patterned(vec![circle], pattern));
    assert_eq!(result.strokes.len(), 9);
    assert!(result
        .strokes
        .iter()
        .all(|s| (s[0].x - s[1].x).abs() > 1e-6));
}

/// A full turn.
const TAU_FULL: f64 = 2.0 * PI;
