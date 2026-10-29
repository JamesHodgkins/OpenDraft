//! `drawing.audit`: read-only housekeeping checks.

use od_api::ErrorCode;
use od_commands::{ModifyAttributes, ModifyLayers};
use od_core::mtext::MText;
use od_core::text_style::TextStyleId;
use od_core::{Attributes, Entity, EntityId, LayerId, Point};
use serde_json::{json, Value};

use super::{document_id, handle_read};
use crate::state::AppState;

const INSTANCE: &str = "inst1";

fn line(x1: f64, x2: f64) -> Entity {
    Entity::Line {
        start: Point::new(x1, 0.0),
        end: Point::new(x2, 0.0),
    }
}

fn audit(state: &AppState, extra: Value) -> Result<Value, od_api::ApiError> {
    let mut args = json!({"document_id": document_id(INSTANCE, state.docs[state.active].uid)});
    for (key, value) in extra.as_object().into_iter().flatten() {
        args[key] = value.clone();
    }
    handle_read(state, INSTANCE, "drawing.audit", args)
}

fn finding<'a>(result: &'a Value, check: &str) -> &'a Value {
    result["findings"]
        .as_array()
        .and_then(|all| all.iter().find(|f| f["check"] == check))
        .unwrap_or(&Value::Null)
}

fn ids(state: &AppState) -> Vec<EntityId> {
    state
        .session()
        .document()
        .objects()
        .iter()
        .map(|o| o.id)
        .collect()
}

/// A new layer with this name.
fn layer_named(state: &mut AppState, name: &str) -> LayerId {
    let id = state.new_layer();
    let mut layer = state.session().document().layer(id).cloned().unwrap();
    layer.name = name.to_owned();
    state.perform(ModifyLayers::new("Rename", vec![layer]));
    id
}

fn put_on(state: &mut AppState, object: EntityId, layer: LayerId) {
    state.perform(ModifyAttributes::new(
        "To layer",
        vec![(object, Attributes::on_layer(layer))],
    ));
}

#[test]
fn a_repeated_object_is_a_duplicate_and_the_original_is_not() {
    let mut state = AppState::new();
    state.add_entity(line(0.0, 4.0));
    state.add_entity(line(0.0, 4.0));
    state.add_entity(line(10.0, 14.0));
    let all = ids(&state);
    let result = audit(&state, json!({"checks": ["duplicates", "overlaps"]})).unwrap();
    let dup = finding(&result, "duplicates");
    assert_eq!(dup["count"], 1, "{result}");
    assert_eq!(dup["entity_ids"], json!([all[1].0.to_string()]));
    assert_eq!(
        finding(&result, "overlaps")["count"],
        0,
        "a duplicate is not also an overlap"
    );
    assert_eq!(result["scanned"], 3);
    assert_eq!(result["revision"], state.session().revision().to_string());
}

#[test]
fn objects_overlapping_in_part_are_found_and_ones_meeting_end_to_end_are_not() {
    let mut state = AppState::new();
    state.add_entity(line(0.0, 4.0));
    state.add_entity(line(2.0, 6.0)); // overlaps the first
    state.add_entity(line(10.0, 12.0));
    state.add_entity(line(12.0, 14.0)); // only meets the one before
    let all = ids(&state);
    let result = audit(&state, json!({"checks": ["overlaps", "duplicates"]})).unwrap();
    let overlaps = finding(&result, "overlaps");
    assert_eq!(overlaps["count"], 2, "both take part: {result}");
    assert_eq!(
        overlaps["entity_ids"],
        json!([all[0].0.to_string(), all[1].0.to_string()])
    );
    assert_eq!(finding(&result, "duplicates")["count"], 0);
    // The checks come back in a fixed order whatever order they were asked in.
    assert_eq!(result["findings"][0]["check"], "duplicates");
}

#[test]
fn empty_things_are_found_even_on_a_locked_layer_which_purge_would_leave() {
    let mut state = AppState::new();
    state.add_entity(line(1.0, 1.0)); // no length
    state.add_entity(line(0.0, 3.0));
    let empty = MText::new(Point::new(0.0, 0.0), 2.5, "  ", TextStyleId::STANDARD);
    state.add_entity(Entity::MText(Box::new(empty)));
    let all = ids(&state);
    let locked = layer_named(&mut state, "Locked");
    put_on(&mut state, all[0], locked);
    let mut layer = state.session().document().layer(locked).cloned().unwrap();
    layer.locked = true;
    state.perform(ModifyLayers::new("Lock", vec![layer]));
    let result = audit(&state, json!({"checks": ["zero_length", "empty_text"]})).unwrap();
    assert_eq!(
        finding(&result, "zero_length")["entity_ids"],
        json!([all[0].0.to_string()])
    );
    assert_eq!(
        finding(&result, "empty_text")["entity_ids"],
        json!([all[2].0.to_string()])
    );
    assert!(
        od_core::purge::unnamed(state.session().document())
            .zero_length
            .is_empty(),
        "PURGE itself leaves it: that is why the audit does not"
    );
}

#[test]
fn definitions_nothing_uses_are_named_and_stock_linetypes_are_not() {
    let mut state = AppState::new();
    layer_named(&mut state, "Spare");
    let result = audit(&state, json!({"checks": ["unused_definitions"]})).unwrap();
    let found = finding(&result, "unused_definitions");
    let names = found["names"].as_array().unwrap();
    assert!(
        names
            .iter()
            .any(|n| n["kind"] == "layer" && n["name"] == "Spare"),
        "{result}"
    );
    assert!(names.iter().all(|n| n["kind"] != "linetype"), "{result}");
    assert_eq!(found["count"].as_u64().unwrap() as usize, names.len());
}

#[test]
fn layers_off_the_standard_are_named_with_how_many_objects_are_on_them() {
    let mut state = AppState::new();
    state.add_entity(line(0.0, 1.0));
    layer_named(&mut state, "A-WALL-EXT");
    let junk = layer_named(&mut state, "my layer");
    let object = ids(&state)[0];
    put_on(&mut state, object, junk);
    let result = audit(&state, json!({"allowed_layers": ["a-wall-*", "Defpoints"]})).unwrap();
    let found = finding(&result, "layer_names");
    assert_eq!(
        found["count"], 1,
        "layer 0 and the matching name pass: {result}"
    );
    assert_eq!(found["names"][0]["name"], "my layer");
    assert_eq!(found["names"][0]["objects"], 1);

    // Asked for with no standard to check against, it says so.
    let none = audit(&state, json!({"checks": ["layer_names"]})).unwrap_err();
    assert_eq!(none.code, ErrorCode::InvalidArguments);
    // Left out unless a standard is given.
    let plain = audit(&state, json!({})).unwrap();
    assert!(finding(&plain, "layer_names").is_null());
}

#[test]
fn the_audit_can_be_narrowed_to_a_layer_or_a_window() {
    let mut state = AppState::new();
    let other = layer_named(&mut state, "Other");
    for x in [0.0, 100.0] {
        state.add_entity(line(x, x + 4.0));
        state.add_entity(line(x, x + 4.0));
    }
    let all = ids(&state);
    put_on(&mut state, all[2], other);
    put_on(&mut state, all[3], other);
    let whole = audit(&state, json!({"checks": ["duplicates"]})).unwrap();
    assert_eq!(finding(&whole, "duplicates")["count"], 2);

    let window = json!({"min_x": -1, "min_y": -1, "max_x": 10, "max_y": 1});
    let near = audit(&state, json!({"checks": ["duplicates"], "bounds": window})).unwrap();
    assert_eq!(near["scanned"], 2);
    assert_eq!(finding(&near, "duplicates")["count"], 1);

    let layer = other.0.to_string();
    let on_layer = audit(&state, json!({"checks": ["duplicates"], "layer_id": layer})).unwrap();
    assert_eq!(on_layer["scanned"], 2);
    assert_eq!(finding(&on_layer, "duplicates")["count"], 1);
}

#[test]
fn a_list_is_cut_to_the_limit_but_the_count_is_complete() {
    let mut state = AppState::new();
    for _ in 0..6 {
        state.add_entity(line(0.0, 4.0));
    }
    let result = audit(&state, json!({"checks": ["duplicates"], "limit": 2})).unwrap();
    let dup = finding(&result, "duplicates");
    assert_eq!(dup["count"], 5);
    assert_eq!(dup["entity_ids"].as_array().unwrap().len(), 2);
    assert_eq!(dup["truncated"], true);
}

#[test]
fn bad_arguments_are_refused_and_nothing_is_changed() {
    let state = AppState::new();
    let revision = state.session().revision();
    let code = |extra: Value| audit(&state, extra).unwrap_err().code;
    assert_eq!(code(json!({"checks": []})), ErrorCode::InvalidArguments);
    assert_eq!(
        code(json!({"checks": ["tidy_up"]})),
        ErrorCode::InvalidArguments
    );
    assert_eq!(code(json!({"tolerance": -1})), ErrorCode::InvalidArguments);
    assert_eq!(
        code(json!({"layer_id": "999"})),
        ErrorCode::InvalidReference
    );
    assert_eq!(
        code(json!({"allowed_layers": [""], "checks": ["layer_names"]})),
        ErrorCode::InvalidArguments
    );
    assert_eq!(code(json!({"surprise": 1})), ErrorCode::InvalidArguments);
    assert_eq!(
        state.session().revision(),
        revision,
        "a look changes nothing"
    );
}

#[test]
fn too_many_objects_to_compare_quickly_say_so_instead_of_stalling() {
    let mut state = AppState::new();
    let limit = od_api::limits::AUDIT_OBJECTS_MAX;
    state.docs[0]
        .session
        .transaction("Fill", |tx| {
            for i in 0..=limit {
                let x = (i * 3) as f64;
                tx.add(line(x, x + 1.0));
            }
            Ok::<(), od_commands::Cancelled>(())
        })
        .unwrap();
    let result = audit(&state, json!({"checks": ["duplicates", "zero_length"]})).unwrap();
    let dup = finding(&result, "duplicates");
    assert!(
        dup["not_run"].as_str().unwrap().contains("Narrow it"),
        "{dup}"
    );
    assert_eq!(dup["count"], 0);
    // The checks that do not compare pairs still ran.
    assert!(finding(&result, "zero_length")["not_run"].is_null());
}
