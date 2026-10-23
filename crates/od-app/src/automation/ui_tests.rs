use od_api::ErrorCode;
use od_core::{Entity, LayerId, Point};
use serde_json::{json, Value};

use super::{dispatch_test, document_id, Caller};
use crate::state::AppState;

const INSTANCE: &str = "inst1";
const ME: Caller = Caller {
    client: "test",
    grant_id: 7,
    history_any: false,
};

fn line(x: f64) -> Entity {
    Entity::Line {
        start: Point::new(x, 0.0),
        end: Point::new(x + 1.0, 0.0),
    }
}

fn doc_id(state: &AppState) -> String {
    document_id(INSTANCE, state.docs[state.active].uid)
}

fn call(
    state: &mut AppState,
    caller: &Caller,
    op: &str,
    mut args: Value,
) -> Result<Value, od_api::ApiError> {
    args["document_id"] = json!(doc_id(state));
    if args["expected_revision"] == "now" {
        args["expected_revision"] = json!(revision(state));
    }
    dispatch_test(state, INSTANCE, caller, op, args)
}

fn revision(state: &AppState) -> String {
    state.session().revision().to_string()
}

fn layers(state: &mut AppState, ops: Value) -> Result<Value, od_api::ApiError> {
    let rev = revision(state);
    call(
        state,
        &ME,
        "layers.apply_batch",
        json!({"expected_revision": rev, "label": "Layers", "operations": ops}),
    )
}

#[test]
fn layers_are_created_changed_and_made_current_in_one_undo_step() {
    let mut state = AppState::new();
    let result = layers(
        &mut state,
        json!([
            {"op": "create", "client_ref": "w", "name": "Walls",
             "props": {"color": {"mode": "rgb", "r": 200, "g": 0, "b": 0}, "lineweight_hundredths_mm": 50}},
            {"op": "create", "client_ref": "d", "name": "Doors", "props": {"locked": true}},
            {"op": "set_current", "target": {"client_ref": "w"}},
            {"op": "update", "target": {"client_ref": "d"}, "props": {"name": "Doors and windows", "visible": false}}
        ]),
    )
    .unwrap();
    assert_eq!(result["changed"], true);
    let walls = LayerId(result["ids"]["w"].as_str().unwrap().parse().unwrap());
    let doors = LayerId(result["ids"]["d"].as_str().unwrap().parse().unwrap());
    assert_eq!(result["current_layer_id"], walls.0.to_string());
    let document = state.session().document();
    assert_eq!(document.layer(walls).unwrap().lineweight.0, 50);
    let d = document.layer(doors).unwrap();
    assert_eq!(
        (d.name.as_str(), d.locked, d.on),
        ("Doors and windows", true, false)
    );
    assert_eq!(document.current_layer(), walls);

    state.docs[0].session.undo();
    assert_eq!(
        state.session().document().layers().len(),
        1,
        "all of it in one step"
    );
    assert_eq!(state.session().document().current_layer(), LayerId::DEFAULT);
}

#[test]
fn layer_rules_are_kept_and_a_bad_operation_changes_nothing() {
    let mut state = AppState::new();
    let before = state.session().revision();
    let bad = |ops: Value, state: &mut AppState| layers(state, ops).unwrap_err();
    let create = |name: &str| json!({"op": "create", "client_ref": "x", "name": name});
    for name in ["", "  ", "a/b", "q?", "0"] {
        assert_eq!(
            bad(json!([create(name)]), &mut state).code,
            ErrorCode::InvalidArguments,
            "{name:?}"
        );
    }
    let rename_zero =
        json!([{"op": "update", "target": {"layer_id": "0"}, "props": {"name": "Zero"}}]);
    assert!(bad(rename_zero, &mut state).message.contains("Layer 0"));
    let freeze_current =
        json!([{"op": "update", "target": {"layer_id": "0"}, "props": {"frozen": true}}]);
    assert!(bad(freeze_current, &mut state)
        .message
        .contains("current layer"));
    let by_layer = json!([{"op": "create", "client_ref": "x", "name": "N", "props": {"color": {"mode": "by_layer"}}}]);
    assert_eq!(bad(by_layer, &mut state).code, ErrorCode::InvalidArguments);
    let linetype =
        json!([{"op": "create", "client_ref": "x", "name": "N", "props": {"linetype": "NOPE"}}]);
    assert_eq!(bad(linetype, &mut state).code, ErrorCode::InvalidReference);
    let missing = json!([{"op": "set_current", "target": {"layer_id": "99"}}]);
    assert_eq!(bad(missing, &mut state).code, ErrorCode::InvalidReference);
    // The second operation fails: the first is rolled back, ids included.
    let half = json!([create("Good"), {"op": "update", "target": {"layer_id": "99"}, "props": {}}]);
    assert_eq!(bad(half, &mut state).code, ErrorCode::InvalidReference);
    // Duplicate names ignore case.
    layers(&mut state, json!([create("Walls")])).unwrap();
    assert_eq!(
        bad(json!([create("WALLS")]), &mut state).code,
        ErrorCode::InvalidArguments
    );
    assert_eq!(state.session().document().layers().len(), 2);
    assert!(state.session().revision() > before);
}

#[test]
fn a_frozen_layer_cannot_become_current() {
    let mut state = AppState::new();
    layers(
        &mut state,
        json!([{"op": "create", "client_ref": "f", "name": "Frozen", "props": {"frozen": true}}]),
    )
    .unwrap();
    let id = state.session().document().layers()[1].id.0.to_string();
    let error = layers(
        &mut state,
        json!([{"op": "set_current", "target": {"layer_id": id}}]),
    )
    .unwrap_err();
    assert!(error.message.contains("frozen"));
}

#[test]
fn selection_can_be_replaced_extended_trimmed_and_cleared() {
    let mut state = AppState::new();
    for x in [0.0, 5.0, 10.0] {
        state.add_entity(line(x));
    }
    let ids: Vec<String> = state
        .session()
        .document()
        .objects()
        .iter()
        .map(|o| o.id.0.to_string())
        .collect();
    let set = |state: &mut AppState, mode: &str, picks: &[&String]| {
        let result = call(
            state,
            &ME,
            "selection.set",
            json!({"mode": mode, "entity_ids": picks}),
        )
        .unwrap();
        result["entity_ids"].as_array().unwrap().len()
    };
    assert_eq!(set(&mut state, "replace", &[&ids[0], &ids[1]]), 2);
    assert_eq!(set(&mut state, "add", &[&ids[2]]), 3);
    assert_eq!(set(&mut state, "remove", &[&ids[0]]), 2);
    assert_eq!(set(&mut state, "replace", &[&ids[1]]), 1);
    assert_eq!(set(&mut state, "clear", &[]), 0);
    let error = call(
        &mut state,
        &ME,
        "selection.set",
        json!({"mode": "add", "entity_ids": ["999"]}),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::EntityNotFound);
    assert!(state.docs[0].selection.is_empty());
    assert_eq!(
        state.session().history().top_undo().unwrap().name,
        "Add line",
        "selection is not history"
    );
}

#[test]
fn hidden_entities_cannot_be_selected_unless_asked_to_skip_them() {
    let mut state = AppState::new();
    state.add_entity(line(0.0));
    state.add_entity(line(5.0));
    let hidden = state.session().document().objects()[1].id;
    layers(
        &mut state,
        json!([{"op": "create", "client_ref": "h", "name": "Hidden", "props": {"visible": false}}]),
    )
    .unwrap();
    let layer = state.session().document().layers()[1].id;
    let rev = revision(&state);
    let batch = json!({"expected_revision": rev, "operations": [
        {"op": "patch", "target": {"entity_id": hidden.0.to_string()}, "patch": {"attributes": {"layer_id": layer.0.to_string()}}}]});
    call(&mut state, &ME, "entities.apply_batch", batch).unwrap();
    let all: Vec<String> = state
        .session()
        .document()
        .objects()
        .iter()
        .map(|o| o.id.0.to_string())
        .collect();
    let args = json!({"mode": "replace", "entity_ids": all});
    let error = call(&mut state, &ME, "selection.set", args).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidReference);
    assert_eq!(error.details["unselectable"], hidden.0.to_string());
    let args = json!({"mode": "replace", "entity_ids": all, "ignore_unselectable": true});
    let result = call(&mut state, &ME, "selection.set", args).unwrap();
    assert_eq!(result["entity_ids"].as_array().unwrap().len(), 1);
}

fn add_via_api(state: &mut AppState, caller: &Caller, x: f64) -> Value {
    let rev = revision(state);
    call(
        state,
        caller,
        "entities.apply_batch",
        json!({"expected_revision": rev, "label": "Mine", "operations": [{"op": "add", "client_ref": "a",
            "entity": {"geometry": {"type": "line", "start": {"x": x, "y": 0}, "end": {"x": x + 1.0, "y": 0}}}}]}),
    )
    .unwrap()
}

#[test]
fn a_connection_can_undo_and_redo_its_own_edit_by_token() {
    let mut state = AppState::new();
    let result = add_via_api(&mut state, &ME, 0.0);
    let token = result["history_token"].clone();
    let summary = call(&mut state, &ME, "document.get", json!({}));
    assert_eq!(summary.unwrap()["undo_step"]["made_by"], "automation");

    let undone = call(
        &mut state,
        &ME,
        "history.undo",
        json!({"expected_revision": "now", "step_token": token}),
    )
    .unwrap();
    assert_eq!(undone["step_name"], "Mine");
    assert!(state.session().document().is_empty());
    let redone = call(
        &mut state,
        &ME,
        "history.redo",
        json!({"expected_revision": "now", "step_token": token}),
    )
    .unwrap();
    assert_eq!(redone["changes"]["added"].as_array().unwrap().len(), 1);
    assert_eq!(state.session().document().len(), 1);
}

#[test]
fn another_connection_or_the_user_cannot_be_undone_without_consent() {
    let mut state = AppState::new();
    let result = add_via_api(&mut state, &ME, 0.0);
    let token = result["history_token"].clone();
    // A restarted sidecar is a new grant.
    let successor = Caller {
        client: "test",
        grant_id: 8,
        history_any: false,
    };
    let args = json!({"expected_revision": "now", "step_token": token});
    let error = call(&mut state, &successor, "history.undo", args.clone()).unwrap_err();
    assert_eq!(error.code, ErrorCode::ScopeDenied);
    assert_eq!(error.details["required"], "history.any");
    assert_eq!(state.session().document().len(), 1);
    // With the user's consent it can.
    let trusted = Caller {
        client: "test",
        grant_id: 8,
        history_any: true,
    };
    call(&mut state, &trusted, "history.undo", args.clone()).unwrap();
    assert!(state.session().document().is_empty());

    // The user's own step is never ours to undo.
    state.add_entity(line(3.0));
    let users = state.session().history().top_undo().unwrap().token;
    let args = json!({"expected_revision": "now", "step_token": users.to_string()});
    let error = call(&mut state, &ME, "history.undo", args).unwrap_err();
    assert_eq!(error.code, ErrorCode::ScopeDenied);
    assert_eq!(state.session().document().len(), 1);
}

#[test]
fn undo_refuses_a_stale_revision_or_a_step_that_moved() {
    let mut state = AppState::new();
    let result = add_via_api(&mut state, &ME, 0.0);
    let token = result["history_token"].clone();
    let old_revision = revision(&state);
    // The user draws something after the client looked.
    state.add_entity(line(9.0));
    let args = json!({"expected_revision": old_revision, "step_token": token});
    let error = call(&mut state, &ME, "history.undo", args).unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRevision);
    // Even with a fresh revision the token no longer names the top step.
    let args = json!({"expected_revision": "now", "step_token": token});
    let error = call(&mut state, &ME, "history.undo", args).unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRevision);
    assert!(error.details.contains_key("actual_step_token"));
    assert_eq!(state.session().document().len(), 2);
}

#[test]
fn nothing_to_undo_is_an_error_not_a_panic() {
    let mut state = AppState::new();
    let args = json!({"expected_revision": "0", "step_token": "1"});
    let error = call(&mut state, &ME, "history.undo", args).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArguments);
}
