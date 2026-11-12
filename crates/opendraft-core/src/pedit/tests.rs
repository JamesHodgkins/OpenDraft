use super::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn poly(points: &[(f64, f64)]) -> PolyData {
    PolyData {
        points: points.iter().map(|&(x, y)| p(x, y)).collect(),
        closed: false,
        bulges: Vec::new(),
        widths: Vec::new(),
    }
}

#[test]
fn lines_arcs_and_rectangles_become_polylines() {
    let line = Entity::Line {
        start: p(0.0, 0.0),
        end: p(3.0, 0.0),
    };
    assert_eq!(PolyData::of(&line).unwrap().points.len(), 2);
    let arc = Entity::Arc {
        center: p(0.0, 0.0),
        radius: 5.0,
        start: 0.0,
        end: std::f64::consts::FRAC_PI_2,
    };
    let from_arc = PolyData::of(&arc).unwrap();
    assert!((from_arc.bulges[0] - (std::f64::consts::PI / 8.0).tan()).abs() < 1e-9);
    let rect = Entity::Rectangle {
        min: p(0.0, 0.0),
        max: p(2.0, 1.0),
    };
    let from_rect = PolyData::of(&rect).unwrap();
    assert!(from_rect.closed && from_rect.points.len() == 4);
    let circle = Entity::Circle {
        center: p(0.0, 0.0),
        radius: 1.0,
    };
    assert!(PolyData::of(&circle).is_none());
}

#[test]
fn closing_and_opening_keep_the_shape_and_clear_the_unused_bulge() {
    let mut open = poly(&[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0)]);
    open.bulges = vec![0.0, 0.5, 0.7];
    let closed = open.with_closed(true);
    assert!(closed.closed);
    assert_eq!(closed.bulges, vec![0.0, 0.5, 0.0]);
    assert_eq!(closed.with_closed(false).points, open.points);
}

#[test]
fn width_sets_every_segment_and_zero_drops_the_list() {
    let wide = poly(&[(0.0, 0.0), (4.0, 0.0)]).with_width(2.0);
    assert_eq!(wide.widths, vec![(2.0, 2.0); 2]);
    let Entity::Polyline { widths, .. } = wide.with_width(0.0).into_entity() else {
        panic!("a polyline");
    };
    assert!(widths.is_empty());
}

#[test]
fn reversing_twice_gives_the_original() {
    let mut shape = poly(&[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0)]);
    shape.bulges = vec![0.5, 0.0, 0.0];
    shape.widths = vec![(1.0, 2.0), (3.0, 3.0), (0.0, 0.0)];
    let back = shape.reversed();
    assert_eq!(back.points[0], p(4.0, 3.0));
    assert_eq!(back.bulges, vec![0.0, -0.5, 0.0]);
    assert_eq!(back.widths, vec![(3.0, 3.0), (2.0, 1.0), (0.0, 0.0)]);
    assert_eq!(back.reversed(), shape);
}

#[test]
fn reversing_a_curve_turns_it_the_other_way() {
    let mut shape = poly(&[(0.0, 0.0), (2.0, 0.0)]);
    shape.bulges = vec![0.5, 0.0];
    shape.widths = vec![(1.0, 3.0), (0.0, 0.0)];
    let back = shape.reversed();
    assert_eq!(back.bulges, vec![-0.5, 0.0]);
    assert_eq!(back.widths[0], (3.0, 1.0));
}

#[test]
fn a_closed_polyline_reverses_with_its_closing_segment() {
    let mut shape = poly(&[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0)]);
    shape.closed = true;
    shape.bulges = vec![0.1, 0.2, 0.3];
    let back = shape.reversed();
    assert_eq!(back.points, vec![p(4.0, 3.0), p(4.0, 0.0), p(0.0, 0.0)]);
    // Closing segment (4,3)->(0,0) was 0.3; now it runs (0,0)->(4,3)
    // as the last one, turning the other way.
    assert_eq!(back.bulges, vec![-0.2, -0.1, -0.3]);
    assert_eq!(back.reversed(), shape);
}

#[test]
fn decurve_straightens_every_segment() {
    let mut shape = poly(&[(0.0, 0.0), (2.0, 0.0)]);
    shape.bulges = vec![0.5, 0.0];
    assert!(shape.decurved().bulges.is_empty());
}

#[test]
fn join_chains_pieces_at_either_end_in_either_direction() {
    let base = poly(&[(0.0, 0.0), (5.0, 0.0)]);
    let others = [
        poly(&[(5.0, 5.0), (5.0, 0.0)]),
        poly(&[(-3.0, 0.0), (0.0, 0.0)]),
        poly(&[(50.0, 50.0), (60.0, 60.0)]),
    ];
    let (joined, used) = base.joined(&others, 0.0, JoinType::Add).unwrap();
    assert_eq!(used, vec![0, 1]);
    assert_eq!(
        joined.points,
        vec![p(-3.0, 0.0), p(0.0, 0.0), p(5.0, 0.0), p(5.0, 5.0)]
    );
}

#[test]
fn join_keeps_the_widths_and_curves_of_what_it_adds() {
    let base = poly(&[(0.0, 0.0), (5.0, 0.0)]);
    let mut other = poly(&[(5.0, 0.0), (5.0, 4.0)]);
    other.bulges = vec![0.4, 0.0];
    other.widths = vec![(1.0, 2.0), (0.0, 0.0)];
    let (joined, _) = base.joined(&[other], 0.0, JoinType::Add).unwrap();
    assert_eq!(joined.bulges, vec![0.0, 0.4, 0.0]);
    assert_eq!(joined.widths, vec![(0.0, 0.0), (1.0, 2.0), (0.0, 0.0)]);
}

#[test]
fn a_gap_within_the_fuzz_is_bridged_and_a_wider_one_is_not() {
    let base = poly(&[(0.0, 0.0), (5.0, 0.0)]);
    let other = poly(&[(5.5, 0.0), (9.0, 0.0)]);
    assert!(base
        .joined(std::slice::from_ref(&other), 0.0, JoinType::Add)
        .is_none());
    let (joined, _) = base.joined(&[other], 1.0, JoinType::Add).unwrap();
    assert_eq!(joined.points.len(), 4, "the gap is a segment of its own");
}

#[test]
fn closed_polylines_neither_join_nor_get_joined() {
    let mut closed = poly(&[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0)]);
    closed.closed = true;
    assert!(closed
        .joined(&[poly(&[(5.0, 5.0), (9.0, 9.0)])], 0.0, JoinType::Add)
        .is_none());
    let open = poly(&[(5.0, 5.0), (9.0, 9.0)]);
    assert!(open.joined(&[closed], 0.0, JoinType::Add).is_none());
}

#[test]
fn insert_on_an_arc_splits_it_into_two_arcs_of_the_same_circle() {
    let mut shape = poly(&[(0.0, 0.0), (4.0, 0.0)]);
    shape.bulges = vec![0.5, 0.0];
    shape.widths = vec![(2.0, 4.0), (0.0, 0.0)];
    let out = shape.with_inserted(0, p(2.0, 1.0)).unwrap();
    assert_eq!(out.points.len(), 3);
    assert!((out.points[1].x - 2.0).abs() < 1e-9);
    assert!(out.bulges[0] != 0.0 && out.bulges[1] != 0.0);
    assert!((out.widths[0].1 - 3.0).abs() < 1e-9);
    assert!((out.widths[1].0 - 3.0).abs() < 1e-9);
}

#[test]
fn insert_on_a_straight_segment_goes_where_given_and_tapers_by_distance() {
    let mut shape = poly(&[(0.0, 0.0), (4.0, 0.0)]);
    shape.widths = vec![(0.0, 4.0), (0.0, 0.0)];
    let out = shape.with_inserted(0, p(1.0, 2.0)).unwrap();
    assert_eq!(out.points[1], p(1.0, 2.0));
    assert_eq!(out.bulges, vec![0.0, 0.0, 0.0]);
    assert_eq!(out.widths[..2], [(0.0, 1.0), (1.0, 4.0)]);
}

#[test]
fn join_prefers_the_nearer_end() {
    let base = poly(&[(0.0, 0.0), (5.0, 0.0)]);
    let far = poly(&[(5.4, 0.0), (9.0, 0.0)]);
    let near = poly(&[(5.1, 0.0), (5.1, 7.0)]);
    let (joined, used) = base.joined(&[far, near], 1.0, JoinType::Add).unwrap();
    assert_eq!(used, vec![1], "the far one is out of reach of the new end");
    assert_eq!(joined.points[2], p(5.1, 0.0));
}

#[test]
fn straighten_replaces_the_vertices_between_with_one_segment() {
    let mut shape = poly(&[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0), (3.0, 1.0), (4.0, 0.0)]);
    shape.bulges = vec![0.3, 0.3, 0.3, 0.3, 0.0];
    let out = shape.straightened(1, 3).unwrap();
    assert_eq!(out.points.len(), 4);
    assert_eq!(out.points[2], p(3.0, 1.0));
    assert_eq!(out.bulges, vec![0.3, 0.0, 0.3, 0.0]);
    assert!(shape.straightened(3, 1).is_none());
}

#[test]
fn break_removes_the_stretch_between_two_vertices() {
    let shape = poly(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0), (4.0, 0.0)]);
    let pieces = shape.broken(3, 1).unwrap();
    assert_eq!(pieces.len(), 2);
    assert_eq!(pieces[0].points, vec![p(0.0, 0.0), p(1.0, 0.0)]);
    assert_eq!(pieces[1].points, vec![p(3.0, 0.0), p(4.0, 0.0)]);
}

#[test]
fn break_at_one_vertex_cuts_there_and_at_an_end_does_nothing() {
    let shape = poly(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]);
    let pieces = shape.broken(1, 1).unwrap();
    assert_eq!(pieces[0].points.len() + pieces[1].points.len(), 4);
    assert!(shape.broken(0, 0).is_none());
    assert!(shape.broken(2, 2).is_none());
    let from_start = shape.broken(0, 1).unwrap();
    assert_eq!(from_start.len(), 1);
    assert_eq!(from_start[0].points, vec![p(1.0, 0.0), p(2.0, 0.0)]);
}

#[test]
fn breaking_a_closed_polyline_opens_it_around_the_gap() {
    let mut square = poly(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
    square.closed = true;
    let pieces = square.broken(0, 1).unwrap();
    assert_eq!(pieces.len(), 1);
    assert_eq!(
        pieces[0].points,
        vec![p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0), p(0.0, 0.0)]
    );
    let same = square.broken(2, 2).unwrap();
    assert_eq!(same[0].points.len(), 5);
    assert_eq!(same[0].points[0], p(4.0, 4.0));
    assert_eq!(same[0].points[4], p(4.0, 4.0));
}

#[test]
fn extend_moves_both_ends_to_where_their_lines_cross() {
    // The corner would be at (5, 0): the first stops short, the second
    // starts below it.
    let base = poly(&[(0.0, 0.0), (4.5, 0.0)]);
    let other = poly(&[(5.0, -0.4), (5.0, 3.0)]);
    let (joined, _) = base.joined(&[other], 1.0, JoinType::Extend).unwrap();
    assert_eq!(joined.points, vec![p(0.0, 0.0), p(5.0, 0.0), p(5.0, 3.0)]);
}

#[test]
fn extend_trims_ends_that_overlap() {
    let base = poly(&[(0.0, 0.0), (5.4, 0.0)]);
    let other = poly(&[(5.0, 0.6), (5.0, 3.0)]);
    let (joined, _) = base.joined(&[other], 1.0, JoinType::Extend).unwrap();
    assert_eq!(joined.points, vec![p(0.0, 0.0), p(5.0, 0.0), p(5.0, 3.0)]);
}

#[test]
fn extend_alone_refuses_parallel_lines_and_both_adds_a_segment() {
    let base = poly(&[(0.0, 0.0), (4.0, 0.0)]);
    let other = poly(&[(4.5, 0.0), (9.0, 0.0)]);
    assert!(base
        .joined(std::slice::from_ref(&other), 1.0, JoinType::Extend)
        .is_none());
    let (joined, _) = base.joined(&[other], 1.0, JoinType::Both).unwrap();
    assert_eq!(joined.points.len(), 4, "a segment crosses the gap");
}

#[test]
fn extend_refuses_a_curved_end_segment() {
    let mut base = poly(&[(0.0, 0.0), (4.5, 0.0)]);
    base.bulges = vec![0.3, 0.0];
    let other = poly(&[(5.0, -0.4), (5.0, 3.0)]);
    assert!(base.joined(&[other], 1.0, JoinType::Extend).is_none());
}

#[test]
fn ends_that_touch_join_whatever_the_type() {
    let base = poly(&[(0.0, 0.0), (4.0, 0.0)]);
    let other = poly(&[(4.0, 0.0), (4.0, 3.0)]);
    for kind in [JoinType::Extend, JoinType::Add, JoinType::Both] {
        let (joined, _) = base
            .joined(std::slice::from_ref(&other), 0.0, kind)
            .unwrap();
        assert_eq!(joined.points.len(), 3);
    }
}

#[test]
fn far_from_the_origin_ends_still_meet() {
    let x = 1.0e9;
    let base = poly(&[(x, 0.0), (x + 4.0, 0.0)]);
    let other = poly(&[(x + 4.0 + 1.0e-7, 0.0), (x + 4.0, 3.0)]);
    assert!(base.joined(&[other], 0.0, JoinType::Add).is_some());
}
