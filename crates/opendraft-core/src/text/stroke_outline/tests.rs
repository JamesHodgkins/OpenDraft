use super::*;
use crate::text::stroke_font::{od_roman, StrokeFont};

fn glyph(text: &str) -> StrokeGlyph {
    let font =
        StrokeFont::parse(&format!("odroman 1\nmetrics\nglyph 0041 adv=700\n{text}\n")).unwrap();
    font.glyph_of('A').unwrap().clone()
}

fn area(c: &[(f64, f64)]) -> f64 {
    let n = c.len();
    (0..n)
        .map(|i| {
            let (a, b) = (c[i], c[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f64>()
        / 2.0
}

fn inside(c: &[(f64, f64)], p: (f64, f64)) -> bool {
    let mut hit = false;
    let n = c.len();
    for i in 0..n {
        let (a, b) = (c[i], c[(i + 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            hit = !hit;
        }
    }
    hit
}

#[test]
fn a_straight_stroke_is_a_capsule() {
    let g = glyph("stroke M 0 0 L 1000 0");
    let contours = stroke_contours(&g, 0.1, 1000.0, 0.5);
    assert_eq!(contours.len(), 1);
    let expected = 1000.0 * 100.0 + PI * 50.0 * 50.0;
    assert!((area(&contours[0]) - expected).abs() / expected < 0.01);
}

#[test]
fn every_contour_winds_counter_clockwise() {
    let font = od_roman().unwrap();
    for ch in ('!'..='~').filter_map(|c| font.glyph_of(c)) {
        for contour in stroke_contours(ch, 0.1, 1000.0, 2.0) {
            assert!(area(&contour) > 0.0, "{} winds the wrong way", ch.ch);
        }
    }
}

#[test]
fn a_dot_is_a_disc_the_width_of_the_pen() {
    let g = glyph("dot 100 100");
    let contours = stroke_contours(&g, 0.2, 1000.0, 0.5);
    assert_eq!(contours.len(), 1);
    let expected = PI * 100.0 * 100.0;
    assert!((area(&contours[0]) - expected).abs() / expected < 0.01);
    assert!(inside(&contours[0], (100.0, 100.0)));
}

#[test]
fn a_stroke_weight_scales_the_pen() {
    let thick = stroke_contours(&glyph("stroke M 0 0 L 1000 0"), 0.1, 1000.0, 0.5);
    let fine = stroke_contours(&glyph("stroke w=0.5 M 0 0 L 1000 0"), 0.1, 1000.0, 0.5);
    let ratio = area(&thick[0]) / area(&fine[0]);
    assert!(ratio > 2.0 && ratio < 2.2, "{ratio}");
}

#[test]
fn the_weight_is_clamped() {
    let g = glyph("stroke M 0 0 L 1000 0");
    assert_eq!(
        stroke_contours(&g, 9.0, 1000.0, 1.0),
        stroke_contours(&g, MAX_WEIGHT, 1000.0, 1.0)
    );
    assert_eq!(
        stroke_contours(&g, 0.0, 1000.0, 1.0),
        stroke_contours(&g, MIN_WEIGHT, 1000.0, 1.0)
    );
}

#[test]
fn a_coarser_tolerance_makes_fewer_points() {
    let g = glyph("stroke M 0 0 C 0 500 500 500 500 0");
    let count = |t| {
        stroke_contours(&g, 0.1, 1000.0, t)
            .iter()
            .map(Vec::len)
            .sum::<usize>()
    };
    assert!(count(0.2) > count(20.0));
}

#[test]
fn the_outline_covers_the_centre_line() {
    let font = od_roman().unwrap();
    for g in ('!'..='~').filter_map(|c| font.glyph_of(c)) {
        let contours = stroke_contours(g, 0.1, 1000.0, 1.0);
        for s in &g.strokes {
            assert!(
                contours.iter().any(|c| inside(c, s.start)),
                "{}: stroke start not covered",
                g.ch
            );
            if let Some(Segment::Line(p) | Segment::Cubic(_, _, p)) = s.segments.last() {
                assert!(
                    contours.iter().any(|c| inside(c, *p)),
                    "{}: end not covered",
                    g.ch
                );
            }
        }
    }
}

#[test]
fn degenerate_strokes_make_nothing_and_nothing_is_not_finite() {
    let g = glyph("stroke M 5 5 L 5 5 L 5 5");
    assert!(stroke_contours(&g, 0.1, 1000.0, 1.0).is_empty());
    let font = od_roman().unwrap();
    for g in ('!'..='~').filter_map(|c| font.glyph_of(c)) {
        for c in stroke_contours(g, 0.4, 1000.0, 0.01) {
            assert!(c.iter().all(|p| p.0.is_finite() && p.1.is_finite()));
        }
    }
}
