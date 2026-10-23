use super::tests::{at, click, entities, key, number, p, polyline, square, started};
use super::*;
use crate::state::AppState;

fn editing(items: &[Entity]) -> AppState {
    let mut state = started(items);
    click(&mut state, 0);
    key(&mut state, "Edit vertex");
    state
}

#[test]
fn insert_adds_a_vertex_after_the_marked_one() {
    let mut state = editing(&[square()]);
    key(&mut state, "Insert");
    at(&mut state, 2.0, 1.0);
    let Entity::Polyline { points, .. } = &entities(&state)[0] else {
        panic!("a polyline");
    };
    assert_eq!(points[1], p(2.0, 1.0));
    assert_eq!(points.len(), 5);
}

#[test]
fn move_follows_next_and_previous() {
    let mut state = editing(&[square()]);
    key(&mut state, "Next");
    key(&mut state, "Next");
    key(&mut state, "Previous");
    key(&mut state, "Move");
    at(&mut state, 9.0, 9.0);
    let Entity::Polyline { points, .. } = &entities(&state)[0] else {
        panic!("a polyline");
    };
    assert_eq!(points[1], p(9.0, 9.0));
}

#[test]
fn width_of_one_segment() {
    let mut state = editing(&[square()]);
    key(&mut state, "Width");
    number(&mut state, 1.0);
    number(&mut state, 3.0);
    let Entity::Polyline { widths, .. } = &entities(&state)[0] else {
        panic!("a polyline");
    };
    assert_eq!(widths[0], (1.0, 3.0));
    assert_eq!(widths[1], (0.0, 0.0));
}

#[test]
fn straighten_removes_the_vertices_between() {
    let mut state = editing(&[square()]);
    key(&mut state, "Straighten");
    key(&mut state, "Next");
    key(&mut state, "Next");
    key(&mut state, "Go");
    let Entity::Polyline { points, .. } = &entities(&state)[0] else {
        panic!("a polyline");
    };
    assert_eq!(points.len(), 3);
    assert_eq!(points[1], p(4.0, 4.0));
}

#[test]
fn break_leaves_two_polylines_on_the_layer() {
    let five = polyline(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0), (4.0, 0.0)]);
    let mut state = editing(&[five]);
    key(&mut state, "Next");
    key(&mut state, "Break");
    key(&mut state, "Next");
    key(&mut state, "Next");
    key(&mut state, "Go");
    let now = entities(&state);
    assert_eq!(now.len(), 2);
    let Entity::Polyline { points, .. } = &now[1] else {
        panic!("a polyline");
    };
    assert_eq!(points, &vec![p(3.0, 0.0), p(4.0, 0.0)]);
    // The break ended vertex editing.
    assert!(state.cmd_prompt().text.contains("Enter an option"));
}

#[test]
fn exit_goes_back_to_the_main_options() {
    let mut state = editing(&[square()]);
    key(&mut state, "eXit");
    assert!(state.cmd_prompt().plain().contains("Join"));
}

#[test]
fn next_and_previous_stop_at_the_ends() {
    let mut state = editing(&[polyline(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)])]);
    key(&mut state, "Previous");
    key(&mut state, "Previous");
    for _ in 0..5 {
        key(&mut state, "Next");
    }
    key(&mut state, "Move");
    at(&mut state, 9.0, 9.0);
    let Entity::Polyline { points, .. } = &entities(&state)[0] else {
        panic!("a polyline");
    };
    assert_eq!(points[2], p(9.0, 9.0), "stopped on the last vertex");
    assert_eq!(points[0], p(0.0, 0.0));
}

#[test]
fn insert_on_an_arc_keeps_the_circle() {
    let arc = Entity::Polyline {
        points: vec![p(0.0, 0.0), p(4.0, 0.0)],
        closed: false,
        bulges: vec![1.0, 0.0],
        widths: Vec::new(),
    };
    let mut state = editing(&[arc]);
    key(&mut state, "Insert");
    at(&mut state, 2.0, -5.0);
    let Entity::Polyline { points, bulges, .. } = &entities(&state)[0] else {
        panic!("a polyline");
    };
    // A semicircle of radius 2 about (2, 0): the new point is on it.
    assert!((points[1].x - 2.0).abs() < 1e-9 && (points[1].y + 2.0).abs() < 1e-9);
    assert!(bulges[0] != 0.0 && bulges[1] != 0.0);
}
