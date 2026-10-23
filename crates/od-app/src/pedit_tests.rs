use super::*;
use crate::pedit_memory::remember;
use crate::state::AppState;
use od_commands::Reply;
use od_core::Point;

thread_local! {
    /// The settings PEDIT remembers are shared, so a test holds its turn at
    /// them until its thread ends.
    static TURN: std::cell::RefCell<Option<std::sync::MutexGuard<'static, ()>>> =
        const { std::cell::RefCell::new(None) };
}

/// A new drawing, after taking this test's turn at the settings.
pub(super) fn fresh() -> AppState {
    TURN.with(|t| *t.borrow_mut() = Some(crate::pedit_memory::take_turn_for_tests()));
    AppState::new()
}

pub(super) fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

pub(super) fn line(a: Point, b: Point) -> Entity {
    Entity::Line { start: a, end: b }
}

pub(super) fn polyline(points: &[(f64, f64)]) -> Entity {
    Entity::Polyline {
        points: points.iter().map(|&(x, y)| p(x, y)).collect(),
        closed: false,
        bulges: Vec::new(),
        widths: Vec::new(),
    }
}

pub(super) fn entities(state: &AppState) -> Vec<Entity> {
    let objects = state.session().document().objects();
    objects.iter().map(|o| o.entity.clone()).collect()
}

/// PEDIT started with `items` drawn and nothing selected.
pub(super) fn started(items: &[Entity]) -> AppState {
    let mut state = fresh();
    for e in items {
        state.add_entity(e.clone());
    }
    state.start_command("PEDIT", pedit_objects);
    state.settle_command();
    state
}

pub(super) fn click(state: &mut AppState, index: usize) {
    let id = state.session().document().objects()[index].id;
    state.answer_object(p(0.5, 0.0), Some(id), false);
    state.settle_command();
}

pub(super) fn key(state: &mut AppState, label: &str) {
    state.answer_keyword(label);
    state.settle_command();
}

pub(super) fn number(state: &mut AppState, n: f64) {
    state.reply(Reply::Number(n));
    state.settle_command();
}

pub(super) fn at(state: &mut AppState, x: f64, y: f64) {
    state.answer_point(p(x, y));
    state.settle_command();
}

pub(super) fn enter(state: &mut AppState) {
    state.answer_enter();
    state.settle_command();
}

pub(super) fn said(state: &AppState, text: &str) -> bool {
    state.cmdline.log.iter().any(|l| l.contains(text))
}

pub(super) fn square() -> Entity {
    polyline(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)])
}

#[test]
fn close_then_open_and_undo_takes_the_change_back() {
    let mut state = started(&[square()]);
    click(&mut state, 0);
    key(&mut state, "Close");
    assert!(matches!(
        entities(&state)[0],
        Entity::Polyline { closed: true, .. }
    ));
    key(&mut state, "Open");
    assert!(matches!(
        entities(&state)[0],
        Entity::Polyline { closed: false, .. }
    ));
    key(&mut state, "Undo");
    assert!(matches!(
        entities(&state)[0],
        Entity::Polyline { closed: true, .. }
    ));
}

#[test]
fn a_line_is_turned_into_a_polyline_on_request_and_widened() {
    let mut state = started(&[line(p(0.0, 0.0), p(4.0, 0.0))]);
    click(&mut state, 0);
    assert!(state.cmd_prompt().text.contains("turn it into one"));
    enter(&mut state);
    assert!(matches!(entities(&state)[0], Entity::Polyline { .. }));
    key(&mut state, "Width");
    number(&mut state, 2.0);
    let Entity::Polyline { widths, .. } = &entities(&state)[0] else {
        panic!("a polyline");
    };
    assert_eq!(widths, &vec![(2.0, 2.0); 2]);
}

#[test]
fn declining_the_conversion_leaves_the_line() {
    let mut state = started(&[line(p(0.0, 0.0), p(4.0, 0.0))]);
    click(&mut state, 0);
    key(&mut state, "No");
    assert!(matches!(entities(&state)[0], Entity::Line { .. }));
}

#[test]
fn join_takes_a_selection_of_lines_onto_the_polyline_in_one_step() {
    let mut state = started(&[
        polyline(&[(0.0, 0.0), (4.0, 0.0)]),
        line(p(4.0, 0.0), p(4.0, 3.0)),
        line(p(4.0, 3.0), p(0.0, 3.0)),
        line(p(50.0, 50.0), p(60.0, 50.0)),
    ]);
    click(&mut state, 0);
    key(&mut state, "Join");
    let ids: Vec<_> = state
        .session()
        .document()
        .objects()
        .iter()
        .map(|o| o.id)
        .collect();
    state.reply(Reply::Selection(ids));
    state.settle_command();
    let now = entities(&state);
    assert_eq!(now.len(), 2);
    assert!(matches!(&now[0], Entity::Polyline { points, .. } if points.len() == 4));
    assert!(said(&state, "2 segments added to polyline."));
    key(&mut state, "Undo");
    assert_eq!(entities(&state).len(), 4);
}

#[test]
fn reverse_and_decurve() {
    let curved = Entity::Polyline {
        points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0)],
        closed: false,
        bulges: vec![0.5, 0.0, 0.0],
        widths: Vec::new(),
    };
    let mut state = started(&[curved]);
    click(&mut state, 0);
    key(&mut state, "Reverse");
    let Entity::Polyline { points, bulges, .. } = &entities(&state)[0] else {
        panic!("a polyline");
    };
    assert_eq!(points[0], p(4.0, 4.0));
    assert_eq!(bulges, &vec![0.0, -0.5, 0.0]);
    key(&mut state, "Decurve");
    assert!(matches!(&entities(&state)[0], Entity::Polyline { bulges, .. } if bulges.is_empty()));
}

#[test]
fn a_selected_polyline_skips_the_pick() {
    let mut state = fresh();
    state.add_entity(square());
    state.select_all();
    state.start_command("PEDIT", pedit_objects);
    state.settle_command();
    assert!(state.cmd_prompt().text.contains("Enter an option"));
}

#[test]
fn something_that_is_not_a_polyline_is_refused() {
    let mut state = started(&[Entity::Circle {
        center: p(0.0, 0.0),
        radius: 2.0,
    }]);
    click(&mut state, 0);
    assert!(said(&state, "Object is not a polyline."));
}

#[test]
fn multiple_closes_and_joins_what_was_selected() {
    let mut state = fresh();
    state.add_entity(square());
    state.add_entity(polyline(&[(10.0, 0.0), (12.0, 0.0)]));
    state.add_entity(line(p(12.0, 0.0), p(12.0, 2.0)));
    state.select_all();
    state.start_command("PEDIT", pedit_objects);
    state.settle_command();
    enter(&mut state);
    assert!(state.cmd_prompt().text.contains("Enter an option"));
    key(&mut state, "Close");
    assert!(entities(&state)
        .iter()
        .all(|e| matches!(e, Entity::Polyline { closed: true, .. })));
    key(&mut state, "Open");
    key(&mut state, "Join");
    enter(&mut state);
    assert_eq!(entities(&state).len(), 2);
}

#[test]
fn ltype_gen_sets_the_flag_on_the_object_and_undo_takes_it_back() {
    let mut state = started(&[square()]);
    click(&mut state, 0);
    key(&mut state, "Ltype gen");
    key(&mut state, "ON");
    let flag = |s: &AppState| {
        s.session().document().objects()[0]
            .attrs
            .continuous_generation
    };
    assert!(flag(&state));
    key(&mut state, "Undo");
    assert!(!flag(&state));
}

#[test]
fn multiple_sets_ltype_gen_for_all() {
    let mut state = fresh();
    state.add_entity(square());
    state.add_entity(polyline(&[(10.0, 0.0), (12.0, 0.0)]));
    state.select_all();
    state.start_command("PEDIT", pedit_objects);
    state.settle_command();
    key(&mut state, "Ltype gen");
    key(&mut state, "ON");
    let doc = state.session().document();
    assert!(doc.objects().iter().all(|o| o.attrs.continuous_generation));
}

#[test]
fn a_spline_is_refused_by_name_and_the_pick_prompt_stays() {
    let spline = Entity::Spline(Box::new(
        od_core::Spline::from_control_points(1, vec![p(0.0, 0.0), p(5.0, 0.0)]).unwrap(),
    ));
    let mut state = started(&[spline, square()]);
    click(&mut state, 0);
    assert!(said(
        &state,
        "Spline-to-polyline conversion is not yet supported"
    ));
    assert!(state.cmd_prompt().text.contains("Select polyline"));
    click(&mut state, 1);
    assert!(state.cmd_prompt().text.contains("Enter an option"));
}

#[test]
fn peditaccept_turns_a_line_into_a_polyline_without_asking() {
    let mut state = fresh();
    state.add_entity(line(p(0.0, 0.0), p(4.0, 0.0)));
    remember(|m| m.accept = true);
    state.start_command("PEDIT", pedit_objects);
    state.settle_command();
    click(&mut state, 0);
    assert!(matches!(entities(&state)[0], Entity::Polyline { .. }));
    assert!(state.cmd_prompt().text.contains("Enter an option"));
    key(&mut state, "Undo");
    assert!(matches!(entities(&state)[0], Entity::Line { .. }));
}

#[test]
fn undo_takes_back_the_conversion_too() {
    let mut state = started(&[line(p(0.0, 0.0), p(4.0, 0.0))]);
    click(&mut state, 0);
    enter(&mut state);
    key(&mut state, "Undo");
    assert!(matches!(entities(&state)[0], Entity::Line { .. }));
}

#[test]
fn a_preselected_object_that_is_no_polyline_goes_back_to_the_pick() {
    let mut state = fresh();
    state.add_entity(Entity::Circle {
        center: p(0.0, 0.0),
        radius: 2.0,
    });
    state.select_all();
    state.start_command("PEDIT", pedit_objects);
    state.settle_command();
    assert!(said(&state, "Object is not a polyline."));
    assert!(state.cmd_prompt().text.contains("Select polyline"));
}

/// Typed answers, as a user gives them, pick the menu's keywords.
#[test]
fn the_menus_take_typed_keywords_and_enter_finishes() {
    let mut state = started(&[square()]);
    click(&mut state, 0);
    for text in ["c", "e", "n", "m", "9,9", "x"] {
        state.cmd_submit(text);
        state.settle_command();
    }
    let Entity::Polyline { points, closed, .. } = &entities(&state)[0] else {
        panic!("a polyline");
    };
    assert!(*closed, "c closed it");
    assert_eq!(points[1], p(9.0, 9.0), "e, n, m moved the second vertex");
    assert!(state.cmd_prompt().text.contains("Enter an option"));
    state.cmd_submit("");
    state.settle_command();
    assert!(!state.command_running(), "Enter ends the command");
}

#[test]
fn the_prompt_lists_the_options_once() {
    let mut state = started(&[square()]);
    click(&mut state, 0);
    let line = state.cmd_prompt().plain();
    assert_eq!(line.matches("Join").count(), 1, "{line}");
    assert!(line.contains("Edit vertex"));
}
