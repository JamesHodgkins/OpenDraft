//! A hatch added through `entities.apply_batch`: made the way the HATCH
//! command makes one, from boundaries named by id or added just before it.

use od_api::ErrorCode;
use od_commands::Provenance;
use od_core::{Entity, Point};
use serde_json::{json, Value};

use super::{dispatch_test, document_id, Caller};
use crate::state::AppState;

const INSTANCE: &str = "inst1";
const CALLER: Caller = Caller {
    client: "test",
    grant_id: 7,
    history_any: false,
};

fn batch(state: &mut AppState, operations: Value) -> Result<Value, od_api::ApiError> {
    let args = json!({
        "document_id": document_id(INSTANCE, state.docs[state.active].uid),
        "expected_revision": state.session().revision().to_string(),
        "operations": operations,
    });
    dispatch_test(state, INSTANCE, &CALLER, "entities.apply_batch", args)
}

fn square(client_ref: &str, at: f64) -> Value {
    json!({"op": "add", "client_ref": client_ref, "entity": {"geometry": {
        "type": "polyline", "closed": true,
        "points": [{"x": at, "y": 0}, {"x": at + 40.0, "y": 0}, {"x": at + 40.0, "y": 40}, {"x": at, "y": 40}]}}})
}

fn hatch(boundary: Value, pattern: Value, associative: bool) -> Value {
    json!({"op": "add", "client_ref": "h", "entity": {"geometry": {
        "type": "hatch", "boundary": boundary, "pattern": pattern, "associative": associative}}})
}

fn ansi31(scale: f64) -> Value {
    json!({"type": "named", "name": "ANSI31", "scale": scale})
}

fn hatches(state: &AppState) -> Vec<od_core::HatchEntity> {
    state
        .session()
        .document()
        .objects()
        .iter()
        .filter_map(|o| match &o.entity {
            Entity::Hatch(h) => Some((**h).clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_shape_and_its_hatch_are_added_together_as_one_undo_step() {
    let mut state = AppState::new();
    let result = batch(
        &mut state,
        json!([
            square("a", 0.0),
            hatch(json!([{"client_ref": "a"}]), ansi31(8.0), false)
        ]),
    )
    .unwrap();
    assert_eq!(result["changed"], true);
    assert_eq!(state.session().document().len(), 2);
    let all = hatches(&state);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].loops.len(), 1);
    assert!(!all[0].associative);
    let top = state.session().history().top_undo().unwrap();
    assert_eq!(top.provenance, Provenance::Automation(7));
    state.docs[0].session.undo();
    assert!(state.session().document().is_empty(), "one undo takes both");
}

#[test]
fn an_associative_hatch_remembers_the_shape_it_follows() {
    let mut state = AppState::new();
    batch(
        &mut state,
        json!([
            square("a", 0.0),
            hatch(json!([{"client_ref": "a"}]), ansi31(8.0), true)
        ]),
    )
    .unwrap();
    let all = hatches(&state);
    assert!(all[0].associative);
    assert_eq!(all[0].source_ids().count(), 1);
}

#[test]
fn two_separate_shapes_in_one_hatch_are_two_loops() {
    let mut state = AppState::new();
    batch(
        &mut state,
        json!([
            square("a", 0.0),
            square("b", 100.0),
            hatch(
                json!([{"client_ref": "a"}, {"client_ref": "b"}]),
                ansi31(8.0),
                false
            )
        ]),
    )
    .unwrap();
    assert_eq!(hatches(&state)[0].loops.len(), 2);
}

#[test]
fn a_shape_already_in_the_drawing_can_be_hatched_by_its_id() {
    let mut state = AppState::new();
    let first = batch(&mut state, json!([square("a", 0.0)])).unwrap();
    let id = first["ids"]["a"].clone();
    batch(
        &mut state,
        json!([hatch(json!([{"entity_id": id}]), ansi31(8.0), false)]),
    )
    .unwrap();
    assert_eq!(hatches(&state).len(), 1);
}

#[test]
fn a_solid_fill_is_allowed() {
    let mut state = AppState::new();
    batch(
        &mut state,
        json!([
            square("a", 0.0),
            hatch(
                json!([{"client_ref": "a"}]),
                json!({"type": "solid"}),
                false
            )
        ]),
    )
    .unwrap();
    assert_eq!(hatches(&state).len(), 1);
}

fn refused(state: &mut AppState, operations: Value) -> od_api::ApiError {
    let before = state.session().document().len();
    let error = batch(state, operations).unwrap_err();
    assert_eq!(
        state.session().document().len(),
        before,
        "nothing was added"
    );
    error
}

#[test]
fn an_unknown_pattern_is_refused_with_the_names_to_use() {
    let mut state = AppState::new();
    let error = refused(
        &mut state,
        json!([
            square("a", 0.0),
            hatch(
                json!([{"client_ref": "a"}]),
                json!({"type": "named", "name": "NOPE"}),
                false
            )
        ]),
    );
    assert_eq!(error.code, ErrorCode::InvalidArguments);
    assert!(error.message.contains("ANSI31"), "{}", error.message);
}

#[test]
fn a_boundary_that_is_not_closed_is_refused() {
    let mut state = AppState::new();
    let line = json!({"op": "add", "client_ref": "l", "entity": {"geometry": {
        "type": "line", "start": {"x": 0, "y": 0}, "end": {"x": 10, "y": 0}}}});
    let error = refused(
        &mut state,
        json!([
            line,
            hatch(json!([{"client_ref": "l"}]), ansi31(8.0), false)
        ]),
    );
    assert!(
        matches!(
            error.code,
            ErrorCode::InvalidGeometry | ErrorCode::InvalidReference
        ),
        "{error:?}"
    );
}

#[test]
fn a_boundary_named_before_it_is_added_is_refused() {
    let mut state = AppState::new();
    let error = refused(
        &mut state,
        json!([
            hatch(json!([{"client_ref": "a"}]), ansi31(8.0), false),
            square("a", 0.0)
        ]),
    );
    assert_eq!(error.code, ErrorCode::InvalidReference);
    assert!(
        error.message.contains("earlier in this batch"),
        "{}",
        error.message
    );
}

#[test]
fn a_boundary_that_is_not_in_the_drawing_is_refused() {
    let mut state = AppState::new();
    let error = refused(
        &mut state,
        json!([hatch(json!([{"entity_id": "999"}]), ansi31(8.0), false)]),
    );
    assert_eq!(error.code, ErrorCode::EntityNotFound);
}

#[test]
fn a_hatch_with_no_boundary_is_refused() {
    let mut state = AppState::new();
    let error = refused(&mut state, json!([hatch(json!([]), ansi31(8.0), false)]));
    assert_eq!(error.code, ErrorCode::InvalidArguments);
}

#[test]
fn a_boundary_of_something_that_cannot_bound_a_hatch_is_refused() {
    let mut state = AppState::new();
    let circle = json!({"op": "add", "client_ref": "c", "entity": {"geometry": {
        "type": "circle", "center": {"x": 0, "y": 0}, "radius": 5}}});
    let text = json!({"op": "add", "client_ref": "t", "entity": {"geometry": {
        "type": "mtext", "insertion": {"x": 0, "y": 0}, "contents": "x", "nominal_height": 2.5}}});
    let error = refused(
        &mut state,
        json!([
            circle,
            text,
            hatch(
                json!([{"client_ref": "c"}, {"client_ref": "t"}]),
                ansi31(8.0),
                false
            )
        ]),
    );
    assert_eq!(error.code, ErrorCode::InvalidReference);
    assert!(error.message.contains("closed shape"), "{}", error.message);
}

#[test]
fn a_hatch_too_fine_to_draw_is_refused_and_says_to_change_the_scale() {
    let mut state = AppState::new();
    let error = refused(
        &mut state,
        json!([
            square("a", 0.0),
            hatch(json!([{"client_ref": "a"}]), ansi31(1.0e-3), false)
        ]),
    );
    assert_eq!(error.code, ErrorCode::InvalidGeometry);
    assert!(error.message.contains("scale"), "{}", error.message);
}

#[test]
fn a_hatch_cannot_replace_another_entity() {
    let mut state = AppState::new();
    let made = batch(&mut state, json!([square("a", 0.0)])).unwrap();
    let id = made["ids"]["a"].clone();
    let replace = json!({"op": "replace", "target": {"entity_id": id}, "entity": {"geometry": {
        "type": "hatch", "boundary": [{"entity_id": id}], "pattern": {"type": "solid"}}}});
    let error = batch(&mut state, json!([replace])).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArguments);
    let _ = Point::new(0.0, 0.0);
}
