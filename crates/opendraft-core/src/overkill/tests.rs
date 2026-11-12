use std::f64::consts::PI;

use super::*;
use crate::Point;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> Entity {
    Entity::Line {
        start: p(x1, y1),
        end: p(x2, y2),
    }
}

fn arc_of(start: f64, end: f64) -> Entity {
    Entity::Arc {
        center: p(0.0, 0.0),
        radius: 5.0,
        start,
        end,
    }
}

fn polyline(points: &[(f64, f64)], closed: bool) -> Entity {
    Entity::Polyline {
        points: points.iter().map(|p| Point::new(p.0, p.1)).collect(),
        closed,
        bulges: Vec::new(),
        widths: Vec::new(),
    }
}

#[test]
fn straight_runs_and_repeated_vertices_come_out_of_a_polyline() {
    let messy = polyline(
        &[(0.0, 0.0), (2.0, 0.0), (2.0, 0.0), (5.0, 0.0), (5.0, 4.0)],
        false,
    );
    let out = run(&[messy], &Settings::default());
    assert_eq!(
        out.changed,
        vec![(0, polyline(&[(0.0, 0.0), (5.0, 0.0), (5.0, 4.0)], false))]
    );
    let square = polyline(
        &[(0.0, 0.0), (3.0, 0.0), (6.0, 0.0), (6.0, 6.0), (0.0, 6.0)],
        true,
    );
    let out = run(&[square], &Settings::default());
    assert!(matches!(&out.changed[0].1, Entity::Polyline { points, .. } if points.len() == 4));
}

#[test]
fn a_polyline_keeps_its_corners_curves_and_widths_and_can_be_left_alone() {
    let bent = polyline(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0)], false);
    let curved = Entity::Polyline {
        points: vec![
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(4.0, 0.0),
        ],
        closed: false,
        bulges: vec![0.0, 0.5, 0.0],
        widths: Vec::new(),
    };
    let wide = Entity::Polyline {
        points: vec![
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(4.0, 0.0),
        ],
        closed: false,
        bulges: Vec::new(),
        widths: vec![(0.1, 0.1); 3],
    };
    assert_eq!(
        run(&[bent, curved, wide], &Settings::default()),
        Outcome::default()
    );
    let straight = polyline(&[(0.0, 0.0), (2.0, 0.0), (4.0, 0.0)], false);
    let off = Settings {
        optimize_polylines: false,
        ..Settings::default()
    };
    assert_eq!(run(&[straight], &off), Outcome::default());
}

fn run(items: &[Entity], settings: &Settings) -> Outcome {
    let grouped: Vec<_> = items
        .iter()
        .map(|e| Candidate {
            entity: e.clone(),
            group: 0,
            protected: false,
        })
        .collect();
    overkill(&grouped, settings).expect("within the limits")
}

#[test]
fn a_duplicate_line_goes_even_when_drawn_the_other_way() {
    let out = run(
        &[line(0.0, 0.0, 5.0, 0.0), line(5.0, 0.0, 0.0, 0.0)],
        &Settings::default(),
    );
    assert_eq!(out.removed, vec![1]);
    assert!(out.changed.is_empty());
}

#[test]
fn lines_within_the_tolerance_are_duplicates_and_beyond_it_are_not() {
    let near = [line(0.0, 0.0, 5.0, 0.0), line(0.0, 0.001, 5.0, 0.001)];
    let loose = Settings {
        tolerance: 0.01,
        ..Settings::default()
    };
    assert_eq!(run(&near, &loose).removed, vec![1]);
    assert!(run(&near, &Settings::default()).removed.is_empty());
}

#[test]
fn a_line_inside_another_goes_and_the_first_takes_the_longer_span() {
    let out = run(
        &[line(2.0, 0.0, 3.0, 0.0), line(0.0, 0.0, 10.0, 0.0)],
        &Settings {
            overlapping: false,
            end_to_end: false,
            ..Settings::default()
        },
    );
    assert_eq!(out.removed, vec![1]);
    assert_eq!(out.changed, vec![(0, line(0.0, 0.0, 10.0, 0.0))]);
}

#[test]
fn partly_overlapping_lines_merge_only_when_allowed() {
    let items = [line(0.0, 0.0, 6.0, 0.0), line(4.0, 0.0, 10.0, 0.0)];
    let out = run(&items, &Settings::default());
    assert_eq!(out.changed, vec![(0, line(0.0, 0.0, 10.0, 0.0))]);
    assert_eq!(out.removed, vec![1]);
    let off = Settings {
        overlapping: false,
        ..Settings::default()
    };
    assert_eq!(run(&items, &off), Outcome::default());
}

#[test]
fn lines_meeting_end_to_end_merge_only_when_allowed() {
    let items = [line(0.0, 0.0, 4.0, 0.0), line(4.0, 0.0, 9.0, 0.0)];
    assert_eq!(run(&items, &Settings::default()).removed, vec![1]);
    let off = Settings {
        end_to_end: false,
        ..Settings::default()
    };
    assert_eq!(run(&items, &off), Outcome::default());
}

#[test]
fn lines_that_are_not_collinear_or_leave_a_gap_stay() {
    let out = run(
        &[
            line(0.0, 0.0, 4.0, 0.0),
            line(4.0, 1.0, 9.0, 1.0),
            line(5.0, 0.0, 9.0, 0.0),
        ],
        &Settings::default(),
    );
    assert_eq!(out, Outcome::default());
}

#[test]
fn a_chain_of_overlaps_ends_as_one_line() {
    let out = run(
        &[
            line(0.0, 0.0, 3.0, 0.0),
            line(8.0, 0.0, 12.0, 0.0),
            line(2.0, 0.0, 9.0, 0.0),
        ],
        &Settings::default(),
    );
    assert_eq!(out.removed, vec![1, 2]);
    assert_eq!(out.changed, vec![(0, line(0.0, 0.0, 12.0, 0.0))]);
}

#[test]
fn objects_of_different_groups_are_left_alone() {
    let item = |group| Candidate {
        entity: line(0.0, 0.0, 5.0, 0.0),
        group,
        protected: false,
    };
    let out = overkill(&[item(0), item(1)], &Settings::default()).expect("fine");
    assert_eq!(out, Outcome::default());
}

#[test]
fn arcs_of_one_circle_merge_and_two_halves_make_a_circle() {
    let out = run(&[arc_of(0.0, 2.0), arc_of(1.0, 3.0)], &Settings::default());
    assert_eq!(out.removed, vec![1]);
    assert!(matches!(&out.changed[0].1,
        Entity::Arc { start, end, .. } if start.abs() < 1e-9 && (end - 3.0).abs() < 1e-9));
    let full = run(&[arc_of(0.0, PI), arc_of(PI, 0.0)], &Settings::default());
    assert!(matches!(full.changed[0].1, Entity::Circle { .. }));
}

#[test]
fn an_arc_across_the_zero_angle_merges_the_right_way_round() {
    let out = run(&[arc_of(6.0, 0.5), arc_of(0.2, 1.0)], &Settings::default());
    assert!(matches!(&out.changed[0].1,
        Entity::Arc { start, end, .. } if (start - 6.0).abs() < 1e-9 && (end - 1.0).abs() < 1e-9));
}

#[test]
fn arcs_with_a_gap_or_another_radius_stay() {
    let other = Entity::Arc {
        center: p(0.0, 0.0),
        radius: 6.0,
        start: 1.0,
        end: 2.0,
    };
    let out = run(
        &[arc_of(0.0, 1.0), arc_of(1.5, 2.5), other],
        &Settings::default(),
    );
    assert_eq!(out, Outcome::default());
}

#[test]
fn a_circle_swallows_its_arcs_and_a_duplicate_circle() {
    let circle = Entity::Circle {
        center: p(0.0, 0.0),
        radius: 5.0,
    };
    let out = run(
        &[arc_of(0.0, 1.0), circle.clone(), circle],
        &Settings::default(),
    );
    assert_eq!(out.removed.len(), 2);
    assert_eq!(out.changed.len(), 1);
    assert!(matches!(out.changed[0].1, Entity::Circle { .. }));
}

#[test]
fn duplicate_polylines_match_in_either_direction_with_their_curves() {
    let a = Entity::Polyline {
        points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)],
        closed: false,
        bulges: vec![0.0, 0.5, 0.0],
        widths: Vec::new(),
    };
    let b = Entity::Polyline {
        points: vec![p(4.0, 3.0), p(4.0, 0.0), p(0.0, 0.0)],
        closed: false,
        bulges: vec![-0.5, 0.0, 0.0],
        widths: Vec::new(),
    };
    assert_eq!(run(&[a.clone(), b], &Settings::default()).removed, vec![1]);
    let c = Entity::Polyline {
        points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)],
        closed: false,
        bulges: Vec::new(),
        widths: Vec::new(),
    };
    assert!(run(&[a, c], &Settings::default()).removed.is_empty());
}

fn wide(points: &[(f64, f64)], widths: &[(f64, f64)], closed: bool) -> Entity {
    Entity::Polyline {
        points: points.iter().map(|p| Point::new(p.0, p.1)).collect(),
        closed,
        bulges: Vec::new(),
        widths: widths.to_vec(),
    }
}

#[test]
fn a_reversed_polyline_with_other_widths_is_not_a_duplicate_but_with_mirrored_ones_is() {
    let a = wide(
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0)],
        &[(1.0, 2.0), (3.0, 4.0), (0.0, 0.0)],
        false,
    );
    let literal = wide(
        &[(4.0, 3.0), (4.0, 0.0), (0.0, 0.0)],
        &[(1.0, 2.0), (3.0, 4.0), (0.0, 0.0)],
        false,
    );
    let mirrored = wide(
        &[(4.0, 3.0), (4.0, 0.0), (0.0, 0.0)],
        &[(4.0, 3.0), (2.0, 1.0), (0.0, 0.0)],
        false,
    );
    let settings = Settings {
        optimize_polylines: false,
        ..Settings::default()
    };
    assert!(run(&[a.clone(), literal], &settings).removed.is_empty());
    assert_eq!(run(&[a, mirrored], &settings).removed, vec![1]);
}

#[test]
fn closed_polylines_match_from_any_start_and_either_way_round() {
    let square = polyline(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)], true);
    let turned = polyline(&[(4.0, 4.0), (0.0, 4.0), (0.0, 0.0), (4.0, 0.0)], true);
    let backwards = polyline(&[(0.0, 4.0), (4.0, 4.0), (4.0, 0.0), (0.0, 0.0)], true);
    let other = polyline(&[(0.0, 0.0), (4.0, 0.0), (4.0, 5.0), (0.0, 5.0)], true);
    let out = run(&[square, turned, backwards, other], &Settings::default());
    assert_eq!(out.removed, vec![1, 2]);
}

fn candidate(entity: Entity, protected: bool) -> Candidate {
    Candidate {
        entity,
        group: 0,
        protected,
    }
}

#[test]
fn a_protected_object_is_never_deleted_or_changed_but_its_duplicate_goes() {
    let settings = Settings::default();
    let out = overkill(
        &[
            candidate(line(0.0, 0.0, 5.0, 0.0), false),
            candidate(line(0.0, 0.0, 5.0, 0.0), true),
        ],
        &settings,
    )
    .expect("fine");
    assert_eq!(out.removed, vec![0], "the unprotected copy goes");
    let out = overkill(
        &[
            candidate(line(0.0, 0.0, 6.0, 0.0), true),
            candidate(line(4.0, 0.0, 10.0, 0.0), false),
        ],
        &settings,
    )
    .expect("fine");
    assert_eq!(
        out,
        Outcome::default(),
        "merging would change the protected one"
    );
    let out = overkill(
        &[
            candidate(line(0.0, 0.0, 10.0, 0.0), true),
            candidate(line(2.0, 0.0, 3.0, 0.0), false),
        ],
        &settings,
    )
    .expect("fine");
    assert_eq!(out.removed, vec![1], "what lies on it may go");
    let out = overkill(
        &[
            candidate(line(0.0, 0.0, 5.0, 0.0), true),
            candidate(line(0.0, 0.0, 5.0, 0.0), true),
        ],
        &settings,
    )
    .expect("fine");
    assert_eq!(out, Outcome::default(), "two protected ones both stay");
}

#[test]
fn a_bad_tolerance_or_too_big_a_selection_is_refused() {
    let items = [candidate(line(0.0, 0.0, 1.0, 0.0), false)];
    for tolerance in [f64::NAN, f64::INFINITY, -1.0] {
        let settings = Settings {
            tolerance,
            ..Settings::default()
        };
        assert_eq!(
            overkill(&items, &settings),
            Err(OverkillError::BadTolerance)
        );
    }
    let many: Vec<_> = (0..MAX_OBJECTS + 1)
        .map(|i| candidate(line(i as f64, 0.0, i as f64 + 0.5, 0.0), false))
        .collect();
    assert!(matches!(
        overkill(&many, &Settings::default()),
        Err(OverkillError::TooManyObjects { .. })
    ));
}

#[test]
fn too_many_comparisons_stop_the_run_without_a_result() {
    let many: Vec<_> = (0..8_000)
        .map(|i| candidate(line(i as f64 * 3.0, 0.0, i as f64 * 3.0 + 1.0, 0.0), false))
        .collect();
    assert!(matches!(
        overkill(&many, &Settings::default()),
        Err(OverkillError::TooMuchWork { .. })
    ));
}

#[test]
fn a_zero_tolerance_asks_for_an_exact_match() {
    let exact = Settings {
        tolerance: 0.0,
        ..Settings::default()
    };
    let same = [line(0.0, 0.0, 5.0, 0.0), line(5.0, 0.0, 0.0, 0.0)];
    assert_eq!(run(&same, &exact).removed, vec![1]);
    let off = [line(0.0, 0.0, 5.0, 0.0), line(0.0, 1e-12, 5.0, 1e-12)];
    assert!(run(&off, &exact).removed.is_empty());
    let touching = [line(0.0, 0.0, 4.0, 0.0), line(4.0, 0.0, 9.0, 0.0)];
    assert_eq!(run(&touching, &exact).removed, vec![1]);
}

#[test]
fn running_it_twice_changes_nothing_the_second_time() {
    let items = [
        line(0.0, 0.0, 6.0, 0.0),
        line(4.0, 0.0, 10.0, 0.0),
        line(0.0, 0.0, 6.0, 0.0),
        arc_of(0.0, 2.0),
        arc_of(1.0, 3.0),
    ];
    let first = run(&items, &Settings::default());
    let mut after: Vec<Entity> = items.to_vec();
    for (i, e) in &first.changed {
        after[*i] = e.clone();
    }
    let kept: Vec<Entity> = (0..after.len())
        .filter(|i| !first.removed.contains(i))
        .map(|i| after[i].clone())
        .collect();
    assert_eq!(run(&kept, &Settings::default()), Outcome::default());
}
