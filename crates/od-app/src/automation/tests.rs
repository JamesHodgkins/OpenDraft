use od_api::ErrorCode;
use od_core::{Entity, Point};
use serde_json::{json, Value};

use super::{document_id, handle_read};
use crate::state::AppState;

const INSTANCE: &str = "inst1";

fn line(x: f64) -> Entity {
    Entity::Line {
        start: Point::new(x, 0.0),
        end: Point::new(x + 1.0, 0.0),
    }
}

fn circle(x: f64) -> Entity {
    Entity::Circle {
        center: Point::new(x, 0.0),
        radius: 1.0,
    }
}

fn doc_id(state: &AppState) -> String {
    document_id(INSTANCE, state.docs[state.active].uid)
}

fn read(state: &AppState, op: &str, args: Value) -> Result<Value, od_api::ApiError> {
    handle_read(state, INSTANCE, op, args)
}

fn with(entities: Vec<Entity>) -> AppState {
    let mut state = AppState::new();
    for entity in entities {
        state.add_entity(entity);
    }
    state
}

#[test]
fn documents_list_names_each_drawing_by_a_stable_id() {
    let state = with(vec![line(0.0)]);
    let result = read(&state, "documents.list", Value::Null).unwrap();
    let doc = &result["documents"][0];
    assert_eq!(doc["document_id"], doc_id(&state));
    assert_eq!(doc["available"], true);
    assert_eq!(doc["modified"], true);
    assert_eq!(doc["revision"], "1");
}

#[test]
fn the_hidden_home_placeholder_is_not_a_listed_or_addressable_drawing() {
    let mut state = AppState::new();
    let uid = state.docs[0].uid;
    state.close_document(0);
    assert!(state.docs[0].placeholder);
    let result = read(&state, "documents.list", Value::Null).unwrap();
    assert_eq!(result["documents"].as_array().unwrap().len(), 0);
    let err = read(
        &state,
        "document.get",
        json!({"document_id": document_id(INSTANCE, state.docs[0].uid)}),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::DocumentNotFound);
    assert_ne!(uid, state.docs[0].uid);
}

#[test]
fn the_summary_counts_and_describes_the_drawing() {
    let state = with(vec![line(0.0), circle(5.0)]);
    let result = read(
        &state,
        "document.get",
        json!({"document_id": doc_id(&state)}),
    )
    .unwrap();
    assert_eq!(result["entity_count"], 2);
    assert_eq!(result["model_entity_count"], 2);
    assert_eq!(result["busy"], false);
    assert_eq!(result["revision"], "2");
    assert_eq!(result["units"]["length"], "decimal");
    assert_eq!(result["bounds"]["max"]["x"], 6.0);
}

#[test]
fn an_unknown_or_foreign_document_id_is_not_found() {
    let state = with(vec![]);
    for id in ["", "doc_inst1_999", "doc_other_1", "1", "doc_inst1_x"] {
        let error = read(&state, "document.get", json!({"document_id": id})).unwrap_err();
        assert_eq!(error.code, ErrorCode::DocumentNotFound, "{id:?}");
    }
}

#[test]
fn bad_arguments_are_reported_not_panicked_on() {
    let state = with(vec![]);
    let error = read(&state, "document.get", json!({"document_id": 5})).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArguments);
    let error = read(&state, "document.get", json!({"document_id": "d", "x": 1})).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArguments);
    let error = read(&state, "nonsense", Value::Null).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArguments);
}

#[test]
fn a_query_pages_through_every_entity_once() {
    let state = with((0..7).map(|i| line(f64::from(i) * 2.0)).collect());
    let mut seen = Vec::new();
    let mut cursor = Value::Null;
    loop {
        let result = read(
            &state,
            "entities.query",
            json!({"document_id": doc_id(&state), "limit": 3, "cursor": cursor}),
        )
        .unwrap();
        let page = result["entities"].as_array().unwrap();
        assert!(page.len() <= 3);
        seen.extend(page.iter().map(|e| e["id"].as_str().unwrap().to_owned()));
        match result.get("next_cursor") {
            Some(next) => cursor = next.clone(),
            None => break,
        }
    }
    assert_eq!(seen.len(), 7);
    let mut unique = seen.clone();
    unique.dedup();
    assert_eq!(unique, seen, "no entity twice");
}

#[test]
fn a_query_filters_by_kind_layer_and_bounds() {
    let state = with(vec![line(0.0), circle(100.0), line(200.0)]);
    let id = doc_id(&state);
    let ids = |args: Value| -> Vec<Value> {
        read(&state, "entities.query", args).unwrap()["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["kind"].clone())
            .collect()
    };
    assert_eq!(
        ids(json!({"document_id": id, "kinds": ["Line"]})),
        ["line", "line"]
    );
    assert_eq!(ids(json!({"document_id": id, "layer_id": "0"})).len(), 3);
    assert!(ids(json!({"document_id": id, "layer_id": "77"})).is_empty());
    let near_circle = json!({"min_x": 90.0, "min_y": -5.0, "max_x": 110.0, "max_y": 5.0});
    assert_eq!(
        ids(json!({"document_id": id, "bounds": near_circle})),
        ["circle"]
    );
}

#[test]
fn a_cursor_goes_stale_when_the_drawing_changes_and_cannot_be_reused_elsewhere() {
    let mut state = with(vec![line(0.0), line(2.0), line(4.0)]);
    let id = doc_id(&state);
    let first = read(
        &state,
        "entities.query",
        json!({"document_id": id, "limit": 1}),
    )
    .unwrap();
    let cursor = first["next_cursor"].clone();
    // Same cursor, different filters: refused.
    let error = read(
        &state,
        "entities.query",
        json!({"document_id": id, "limit": 1, "kinds": ["circle"], "cursor": cursor}),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArguments);
    // Garbage is refused too.
    let error = read(
        &state,
        "entities.query",
        json!({"document_id": id, "cursor": "zz.1.2"}),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArguments);
    state.add_entity(line(9.0));
    let error = read(
        &state,
        "entities.query",
        json!({"document_id": id, "limit": 1, "cursor": cursor}),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRevision);
}

#[test]
fn get_returns_slots_in_request_order_with_null_for_missing_ids() {
    let state = with(vec![line(0.0), circle(1.0)]);
    let all = read(
        &state,
        "entities.query",
        json!({"document_id": doc_id(&state)}),
    )
    .unwrap();
    let first = all["entities"][0]["id"].clone();
    let second = all["entities"][1]["id"].clone();
    let result = read(
        &state,
        "entities.get",
        json!({"document_id": doc_id(&state), "ids": [second, "99999", first]}),
    )
    .unwrap();
    let entities = result["entities"].as_array().unwrap();
    assert_eq!(entities[0]["kind"], "circle");
    assert!(entities[1].is_null());
    assert_eq!(entities[2]["kind"], "line");
    let too_many: Vec<String> = (0..1001).map(|i| i.to_string()).collect();
    let error = read(
        &state,
        "entities.get",
        json!({"document_id": doc_id(&state), "ids": too_many}),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::PayloadTooLarge);
}

#[test]
fn layers_and_selection_are_readable() {
    let mut state = with(vec![line(0.0)]);
    state.select_all();
    let id = doc_id(&state);
    let layers = read(&state, "layers.list", json!({"document_id": id})).unwrap();
    assert_eq!(layers["layers"][0]["name"], "0");
    assert_eq!(layers["current_layer_id"], "0");
    assert_eq!(layers["layers"][0]["color"]["mode"], "foreground");
    let selection = read(&state, "selection.get", json!({"document_id": id})).unwrap();
    assert_eq!(selection["entity_ids"].as_array().unwrap().len(), 1);
}

#[test]
fn a_drawing_in_the_block_editor_answers_nothing_about_itself() {
    let mut state = with(vec![Entity::Rectangle {
        min: Point::new(10.0, 10.0),
        max: Point::new(12.0, 11.0),
    }]);
    state.select_all();
    state.start_command("BLOCK", crate::block_cmd::define_block);
    state.settle_command();
    for answer in ["Box", "10,10"] {
        state.cmd_submit(answer);
        state.settle_command();
    }
    state.select_all();
    state.bedit();
    assert!(state.in_block_editor());
    let id = doc_id(&state);
    for op in [
        "document.get",
        "layers.list",
        "selection.get",
        "entities.query",
    ] {
        let error = read(&state, op, json!({"document_id": id})).unwrap_err();
        assert_eq!(error.code, ErrorCode::BlockEditorOpen, "{op}");
        assert!(error.retryable);
    }
    let listed = read(&state, "documents.list", Value::Null).unwrap();
    let doc = &listed["documents"][0];
    assert_eq!(doc["available"], false);
    assert_eq!(doc["unavailable_reason"], "block_editor_open");
    assert!(
        doc.get("revision").is_none(),
        "no revision of the temporary block"
    );
}
