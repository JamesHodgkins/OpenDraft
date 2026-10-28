use super::dash::{dash_path, fit, Mark, Piece};
use super::StrokeElement::{Dash, Dot, Gap};
use crate::Point;

const DASHED: [super::StrokeElement; 2] = [Dash(12.7), Gap(6.35)];

fn dashes(marks: &[Mark]) -> Vec<(f64, f64)> {
    marks
        .iter()
        .filter_map(|m| match m {
            Mark::Dash { from, to } => Some((*from, *to)),
            Mark::Dot(_) => None,
        })
        .collect()
}

#[test]
fn an_open_line_starts_and_ends_with_a_dash_and_keeps_inner_lengths() {
    let marks = fit(100.0, false, &DASHED, 1.0).unwrap();
    let d = dashes(&marks);
    assert_eq!(d.len(), 5);
    assert_eq!(d[0].0, 0.0);
    assert_eq!(d[4].1, 100.0);
    // 4 repeats and a closing dash use 88.9; the 11.1 left over is shared by
    // the two end dashes.
    assert!((d[0].1 - (12.7 + 5.55)).abs() < 1e-9);
    assert!((d[4].1 - d[4].0 - (12.7 + 5.55)).abs() < 1e-9);
    for w in d[1..4].iter() {
        assert!((w.1 - w.0 - 12.7).abs() < 1e-9);
    }
    for pair in d.windows(2) {
        assert!((pair[1].0 - pair[0].1 - 6.35).abs() < 1e-9 || pair[0].0 == 0.0);
    }
}

#[test]
fn a_path_too_short_for_one_repeat_plus_a_dash_is_continuous() {
    assert_eq!(fit(15.0, false, &DASHED, 1.0), None);
    assert_eq!(fit(19.05, false, &DASHED, 1.0), None);
    assert!(fit(31.75, false, &DASHED, 1.0).is_some());
}

#[test]
fn scale_multiplies_every_length() {
    let one = fit(200.0, false, &DASHED, 1.0).unwrap();
    let two = fit(200.0, false, &DASHED, 2.0).unwrap();
    assert!(dashes(&two).len() < dashes(&one).len());
    let inner = dashes(&two)[1];
    assert!((inner.1 - inner.0 - 25.4).abs() < 1e-9);
}

#[test]
fn bad_scales_lengths_and_empty_patterns_are_continuous() {
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(fit(100.0, false, &DASHED, scale), None);
    }
    assert_eq!(fit(f64::NAN, false, &DASHED, 1.0), None);
    assert_eq!(fit(0.0, false, &DASHED, 1.0), None);
    assert_eq!(fit(100.0, false, &[], 1.0), None);
}

#[test]
fn a_dot_first_pattern_puts_a_dot_at_both_ends() {
    let marks = fit(20.0, false, &[Dot, Gap(6.35)], 1.0).unwrap();
    let at: Vec<f64> = marks
        .iter()
        .map(|m| match m {
            Mark::Dot(s) => *s,
            Mark::Dash { .. } => panic!("no dashes expected"),
        })
        .collect();
    assert_eq!(at.len(), 4);
    assert_eq!(at[0], 0.0);
    assert!((at[3] - 20.0).abs() < 1e-9);
    let gap = at[1] - at[0];
    assert!(gap > 6.35 && (at[2] - at[1] - gap).abs() < 1e-9);
}

#[test]
fn dashes_and_dots_mix_in_dashdot() {
    let pattern = [Dash(12.7), Gap(6.35), Dot, Gap(6.35)];
    let marks = fit(100.0, false, &pattern, 1.0).unwrap();
    assert!(marks.iter().any(|m| matches!(m, Mark::Dot(_))));
    assert!(matches!(marks[0], Mark::Dash { from, .. } if from == 0.0));
    assert!(matches!(marks.last(), Some(Mark::Dash { to, .. }) if *to == 100.0));
}

#[test]
fn a_closed_path_is_stretched_to_whole_repeats_with_no_overlap() {
    let marks = fit(100.0, true, &DASHED, 1.0).unwrap();
    let d = dashes(&marks);
    assert_eq!(d.len(), 5);
    assert_eq!(d[0].0, 0.0);
    let stretch = 100.0 / (5.0 * 19.05);
    assert!((d[0].1 - 12.7 * stretch).abs() < 1e-9);
    // The last gap ends exactly where the path closes.
    assert!((d[4].1 + 6.35 * stretch - 100.0).abs() < 1e-9);
    assert_eq!(fit(10.0, true, &DASHED, 1.0), None);
}

#[test]
fn touching_dashes_are_joined() {
    let pattern = [Dash(2.0), Gap(2.0), Dash(2.0)];
    let marks = fit(100.0, false, &pattern, 1.0).unwrap();
    let d = dashes(&marks);
    for pair in d.windows(2) {
        assert!(pair[1].0 > pair[0].1, "no two dashes touch");
    }
}

#[test]
fn an_enormous_number_of_repeats_is_refused() {
    assert_eq!(fit(1.0e9, false, &DASHED, 1.0), None);
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

#[test]
fn a_line_becomes_dashes_along_it() {
    let pieces = dash_path(&[pt(0.0, 0.0), pt(100.0, 0.0)], false, &DASHED, 1.0).unwrap();
    assert_eq!(pieces.len(), 5);
    let Piece::Dash(first) = &pieces[0] else {
        panic!("a dash")
    };
    assert_eq!(first[0], pt(0.0, 0.0));
    assert!((first[1].x - 18.25).abs() < 1e-9 && first[1].y == 0.0);
}

#[test]
fn a_dash_keeps_the_corner_it_crosses() {
    // An L with legs of 100 and dashes at 0..60, 70..130, 140..200: the middle
    // one runs over the corner, so it has three points.
    let path = [pt(0.0, 0.0), pt(100.0, 0.0), pt(100.0, 100.0)];
    let pieces = dash_path(&path, false, &[Dash(60.0), Gap(10.0)], 1.0).unwrap();
    assert_eq!(pieces.len(), 3);
    let Piece::Dash(middle) = &pieces[1] else {
        panic!("a dash")
    };
    assert_eq!(
        middle,
        &vec![pt(70.0, 0.0), pt(100.0, 0.0), pt(100.0, 30.0)]
    );
}

#[test]
fn dashes_cover_the_expected_length_on_a_polyline() {
    let path = [pt(0.0, 0.0), pt(40.0, 0.0), pt(40.0, 40.0), pt(0.0, 40.0)];
    let pieces = dash_path(&path, true, &DASHED, 0.5).unwrap();
    let drawn: f64 = pieces
        .iter()
        .map(|p| match p {
            Piece::Dash(run) => run
                .windows(2)
                .map(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y))
                .sum::<f64>(),
            Piece::Dot { .. } => 0.0,
        })
        .sum();
    // Two thirds of each repeat is dash, whatever the stretch.
    assert!((drawn - 160.0 * 12.7 / 19.05).abs() < 1e-6, "{drawn}");
}

#[test]
fn a_dot_knows_the_direction_of_the_path() {
    let path = [pt(0.0, 0.0), pt(0.0, 50.0)];
    let pieces = dash_path(&path, false, &[Dot, Gap(5.0)], 1.0).unwrap();
    let Piece::Dot { at, along } = pieces[3] else {
        panic!("a dot")
    };
    assert!(at.x == 0.0 && at.y > 0.0);
    assert!(along.x.abs() < 1e-12 && (along.y - 1.0).abs() < 1e-12);
}

#[test]
fn degenerate_paths_are_continuous() {
    assert_eq!(dash_path(&[pt(1.0, 1.0)], false, &DASHED, 1.0), None);
    assert_eq!(
        dash_path(&[pt(1.0, 1.0), pt(1.0, 1.0)], false, &DASHED, 1.0),
        None
    );
}

fn square() -> crate::Entity {
    crate::Entity::Polyline {
        points: vec![
            pt(0.0, 0.0),
            pt(100.0, 0.0),
            pt(100.0, 100.0),
            pt(0.0, 100.0),
        ],
        closed: true,
        bulges: Vec::new(),
        widths: Vec::new(),
    }
}

#[test]
fn a_polyline_comes_back_one_path_per_segment_or_whole() {
    use super::paths::curve_outlines;
    let sides = curve_outlines(&square(), 0.1, true).unwrap().unwrap();
    assert_eq!(sides.len(), 4);
    assert!(sides
        .iter()
        .all(|(points, closed)| points.len() == 2 && !closed));
    assert_eq!(sides[3].0, vec![pt(0.0, 100.0), pt(0.0, 0.0)]);
    let whole = curve_outlines(&square(), 0.1, false).unwrap().unwrap();
    assert_eq!(whole.len(), 1);
    assert!(whole[0].1);
}

#[test]
fn a_rectangle_and_an_arc_segment_are_split_the_same_way() {
    use super::paths::curve_outlines;
    let rectangle = crate::Entity::Rectangle {
        min: pt(0.0, 0.0),
        max: pt(10.0, 5.0),
    };
    assert_eq!(
        curve_outlines(&rectangle, 0.1, true)
            .unwrap()
            .unwrap()
            .len(),
        4
    );
    let bent = crate::Entity::Polyline {
        points: vec![pt(0.0, 0.0), pt(10.0, 0.0), pt(10.0, 10.0)],
        closed: false,
        bulges: vec![0.0, 1.0, 0.0],
        widths: Vec::new(),
    };
    let parts = curve_outlines(&bent, 0.01, true).unwrap().unwrap();
    assert_eq!(parts.len(), 2);
    assert!(parts[1].0.len() > 8, "the arc segment is flattened");
}

#[test]
fn an_object_scale_multiplies_ltscale_and_a_bad_ltscale_is_ignored() {
    use crate::{Attr, Attributes, Document, LayerId};
    let mut doc = Document::default();
    let attrs = Attributes {
        linetype: Attr::Set(crate::LinetypeId::DASHED),
        linetype_scale: 2.0,
        ..Attributes::on_layer(LayerId::DEFAULT)
    };
    let id = doc.add_with(
        crate::Entity::Line {
            start: pt(0.0, 0.0),
            end: pt(1.0, 0.0),
        },
        attrs,
    );
    assert_eq!(doc.set_linetype_scale(3.0), 1.0);
    let object = doc.object(id).unwrap();
    assert_eq!(doc.dash_pattern(object).unwrap().1, 6.0);
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        doc.set_linetype_scale(bad);
        assert_eq!(doc.linetype_scale(), 3.0);
    }
}
