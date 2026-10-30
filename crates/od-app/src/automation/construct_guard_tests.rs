//! `entities.construct`: what it refuses, that a refused request changes
//! nothing, and how its results behave as edits (one undo step, layers kept,
//! a retry not repeated).

use od_api::ErrorCode;
use od_commands::Provenance;
use od_core::Entity;
use serde_json::{json, Value};

use super::construct_tests::{call, circle, circles, construct, draw, entities, id};
use crate::state::AppState;

#[test]
fn everything_is_one_undo_step_made_by_the_caller() {
    let mut state = AppState::new();
    let ids = draw(&mut state, json!([circle("c", 20.0, 0.0, 1.0)]));
    construct(
        &mut state,
        json!([{"op": "polar_array", "targets": [id(&ids, "c")],
                "center": {"x": 0, "y": 0}, "count": 6, "ref_prefix": "a"}]),
    )
    .unwrap();
    assert_eq!(circles(&state).len(), 6);
    let top = state.session().history().top_undo().unwrap();
    assert_eq!(top.provenance, Provenance::Automation(7));
    state.docs[0].session.undo();
    assert_eq!(
        circles(&state).len(),
        1,
        "one undo takes back all the copies"
    );
}

#[test]
fn copies_keep_the_layer_of_the_shape_they_came_from() {
    let mut state = AppState::new();
    let layers = call(
        &mut state,
        "layers.apply_batch",
        json!({"operations": [{"op": "create", "client_ref": "k", "name": "Holes"}]}),
    )
    .unwrap();
    let layer = layers["ids"]["k"].clone();
    let mut hole = circle("hole", 10.0, 0.0, 1.0);
    hole["entity"]["attributes"] = json!({"layer_id": layer});
    let ids = draw(&mut state, json!([hole]));
    construct(
        &mut state,
        json!([{"op": "polar_array", "targets": [id(&ids, "hole")],
                "center": {"x": 0, "y": 0}, "count": 4, "ref_prefix": "h"}]),
    )
    .unwrap();
    let on_layer = state
        .session()
        .document()
        .objects()
        .iter()
        .filter(|o| {
            state
                .session()
                .document()
                .layer(o.attrs.layer)
                .is_some_and(|l| l.name == "Holes")
        })
        .count();
    assert_eq!(on_layer, 4);
}

#[test]
fn a_polygon_can_be_put_on_a_layer() {
    let mut state = AppState::new();
    let layers = call(
        &mut state,
        "layers.apply_batch",
        json!({"operations": [{"op": "create", "client_ref": "k", "name": "Bolt"}]}),
    )
    .unwrap();
    construct(
        &mut state,
        json!([{"op": "polygon", "client_ref": "p", "center": {"x": 0, "y": 0}, "sides": 6,
                "radius": 5, "layer_id": layers["ids"]["k"]}]),
    )
    .unwrap();
    let object = state.session().document().objects().last().unwrap().clone();
    let name = state
        .session()
        .document()
        .layer(object.attrs.layer)
        .map(|l| l.name.clone());
    assert_eq!(name.as_deref(), Some("Bolt"));
}

fn refused(state: &mut AppState, steps: Value) -> od_api::ApiError {
    let before = (entities(state).len(), state.session().revision());
    let error = construct(state, steps).unwrap_err();
    assert_eq!(
        (entities(state).len(), state.session().revision()),
        before,
        "a refused request changes nothing"
    );
    error
}

#[test]
fn a_failing_step_undoes_the_steps_before_it() {
    let mut state = AppState::new();
    let error = refused(
        &mut state,
        json!([
            {"op": "polygon", "client_ref": "p", "center": {"x": 0, "y": 0}, "sides": 6, "radius": 10},
            {"op": "move", "targets": ["nope"], "dx": 1, "dy": 1},
        ]),
    );
    assert_eq!(error.code, ErrorCode::InvalidReference);
    assert_eq!(error.details.get("step").map(String::as_str), Some("1"));
}

#[test]
fn bad_arguments_are_refused_with_the_reason() {
    let mut state = AppState::new();
    let ids = draw(&mut state, json!([circle("c", 5.0, 0.0, 1.0)]));
    let target = json!([id(&ids, "c")]);
    let cases = [
        json!([{"op": "polygon", "client_ref": "p", "center": {"x": 0, "y": 0}, "sides": 2, "radius": 5}]),
        json!([{"op": "polygon", "client_ref": "p", "center": {"x": 0, "y": 0}, "sides": 6, "radius": 0}]),
        json!([{"op": "scale", "targets": target, "base": {"x": 0, "y": 0}, "factor": 0}]),
        json!([{"op": "mirror", "targets": target, "axis_start": {"x": 1, "y": 1}, "axis_end": {"x": 1, "y": 1}}]),
        json!([{"op": "move", "targets": [], "dx": 1, "dy": 1}]),
        json!([{"op": "rect_array", "targets": target, "columns": 0, "rows": 2,
                "column_spacing": 1, "row_spacing": 1, "ref_prefix": "a"}]),
        json!([{"op": "polar_array", "targets": target, "center": {"x": 0, "y": 0}, "count": 4,
                "fill_angle": 10, "ref_prefix": "a"}]),
        json!([{"op": "move", "targets": target, "dx": "far", "dy": 0}]),
        json!([]),
    ];
    for steps in cases {
        let error = refused(&mut state, steps.clone());
        assert!(
            matches!(error.code, ErrorCode::InvalidArguments),
            "{steps}: {error:?}"
        );
    }
}

#[test]
fn a_name_used_twice_is_refused() {
    let mut state = AppState::new();
    let error = refused(
        &mut state,
        json!([
            {"op": "polygon", "client_ref": "p", "center": {"x": 0, "y": 0}, "sides": 6, "radius": 10},
            {"op": "polygon", "client_ref": "p", "center": {"x": 30, "y": 0}, "sides": 6, "radius": 10},
        ]),
    );
    assert_eq!(error.code, ErrorCode::InvalidArguments);
}

#[test]
fn only_the_kinds_the_batch_tool_can_write_are_touched() {
    let mut state = AppState::new();
    let ids = draw(
        &mut state,
        json!([
            {"op": "add", "client_ref": "sq", "entity": {"geometry": {"type": "polyline", "closed": true,
                "points": [{"x": 0, "y": 0}, {"x": 9, "y": 0}, {"x": 9, "y": 9}, {"x": 0, "y": 9}]}}},
        ]),
    );
    call(
        &mut state,
        "entities.apply_batch",
        json!({"operations": [{"op": "add", "client_ref": "h", "entity": {"geometry": {
            "type": "hatch", "boundary": [{"entity_id": ids["sq"]}],
            "pattern": {"type": "solid"}}}}]}),
    )
    .unwrap();
    let hatch = state
        .session()
        .document()
        .objects()
        .iter()
        .find(|o| matches!(o.entity, Entity::Hatch(_)))
        .map(|o| o.id.0.to_string())
        .unwrap();
    let error = refused(
        &mut state,
        json!([{"op": "move", "targets": [hatch], "dx": 1, "dy": 1}]),
    );
    assert_eq!(error.code, ErrorCode::UnsupportedEntityWrite);
}

#[test]
fn a_stale_revision_is_refused_like_any_edit() {
    let mut state = AppState::new();
    let error = call(
        &mut state,
        "entities.construct",
        json!({"expected_revision": "99", "steps": [
            {"op": "polygon", "client_ref": "p", "center": {"x": 0, "y": 0}, "sides": 6, "radius": 10}]}),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRevision);
}

#[test]
fn a_retry_with_the_same_key_does_not_construct_twice() {
    let mut state = AppState::new();
    let step = json!([{"op": "polygon", "client_ref": "p", "center": {"x": 0, "y": 0}, "sides": 6, "radius": 10}]);
    let first = call(
        &mut state,
        "entities.construct",
        json!({"steps": step, "idempotency_key": "k1"}),
    )
    .unwrap();
    assert_eq!(first["changed"], true);
    let again = call(
        &mut state,
        "entities.construct",
        json!({"steps": step, "idempotency_key": "k1", "expected_revision": first["revision_before"]}),
    )
    .unwrap();
    assert_eq!(again["replayed"], true);
    assert_eq!(entities(&state).len(), 1);
}

#[test]
fn too_many_steps_are_refused() {
    let mut state = AppState::new();
    let one = json!({"op": "polygon", "client_ref": "p", "center": {"x": 0, "y": 0}, "sides": 6, "radius": 10});
    let steps = Value::Array(vec![one; od_api::construct_ops::STEPS_MAX + 1]);
    assert_eq!(refused(&mut state, steps).code, ErrorCode::InvalidArguments);
}

#[test]
fn an_array_of_several_shapes_names_each_copy_by_item_and_shape() {
    let mut state = AppState::new();
    let ids = draw(
        &mut state,
        json!([circle("a", 0.0, 0.0, 1.0), circle("b", 0.0, 5.0, 1.0)]),
    );
    let made = construct(
        &mut state,
        json!([{"op": "rect_array", "targets": [id(&ids, "a"), id(&ids, "b")],
                "columns": 3, "rows": 1, "column_spacing": 10, "row_spacing": 0,
                "ref_prefix": "g"}]),
    )
    .unwrap();
    for name in ["g_1_1", "g_1_2", "g_2_1", "g_2_2"] {
        assert!(made["ids"][name].is_string(), "{name}: {made}");
    }
    assert_eq!(circles(&state).len(), 6);
}

#[test]
fn a_name_that_is_all_digits_is_refused_so_it_is_never_taken_for_an_id() {
    let mut state = AppState::new();
    let before = entities(&state).len();
    let error = construct(
        &mut state,
        json!([{"op": "polygon", "client_ref": "123", "center": {"x": 0, "y": 0},
                "sides": 6, "radius": 10}]),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArguments);
    assert_eq!(entities(&state).len(), before);
}

#[test]
fn a_copy_made_by_mirroring_keeps_the_layer_of_the_original() {
    let mut state = AppState::new();
    let layers = call(
        &mut state,
        "layers.apply_batch",
        json!({"operations": [{"op": "create", "client_ref": "k", "name": "Walls"}]}),
    )
    .unwrap();
    let mut wall = circle("wall", 10.0, 0.0, 1.0);
    wall["entity"]["attributes"] = json!({"layer_id": layers["ids"]["k"]});
    let ids = draw(&mut state, json!([wall]));
    construct(
        &mut state,
        json!([{"op": "mirror", "targets": [id(&ids, "wall")], "copy_as": "m",
                "axis_start": {"x": 0, "y": -1}, "axis_end": {"x": 0, "y": 1}}]),
    )
    .unwrap();
    let on_layer = state
        .session()
        .document()
        .objects()
        .iter()
        .filter(|o| {
            state
                .session()
                .document()
                .layer(o.attrs.layer)
                .is_some_and(|l| l.name == "Walls")
        })
        .count();
    assert_eq!(on_layer, 2);
}
