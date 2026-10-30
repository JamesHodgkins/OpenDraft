use od_api::ErrorCode;
use od_commands::{AddEntity, Provenance};
use od_core::hatch::{loops_from_entities, BoundarySource, HatchEntity, HatchFill, HatchPattern};
use od_core::{Attr, Color, Entity, EntityId, Point};
use serde_json::{json, Value};

use super::{dispatch_test, document_id, Caller};
use crate::state::AppState;

const INSTANCE: &str = "inst1";
const GRANT: u64 = 7;
const CALLER: Caller = Caller {
    client: "test",
    grant_id: GRANT,
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

fn revision(state: &AppState) -> u64 {
    state.session().revision()
}

fn batch(state: &mut AppState, ops: Value) -> Result<Value, od_api::ApiError> {
    let args = json!({
        "document_id": doc_id(state),
        "expected_revision": revision(state).to_string(),
        "label": "Test edit",
        "operations": ops,
    });
    dispatch_test(state, INSTANCE, &CALLER, "entities.apply_batch", args)
}

fn code(result: Result<Value, od_api::ApiError>) -> ErrorCode {
    result.unwrap_err().code
}

fn ids(state: &AppState) -> Vec<u64> {
    state
        .session()
        .document()
        .objects()
        .iter()
        .map(|o| o.id.0)
        .collect()
}

fn add_line(client_ref: &str, x: f64) -> Value {
    json!({"op": "add", "client_ref": client_ref, "entity": {"geometry": {
        "type": "line", "start": {"x": x, "y": 0.0}, "end": {"x": x + 1.0, "y": 0.0}}}})
}

#[test]
fn a_batch_adds_entities_as_one_undo_step_with_provenance() {
    let mut state = AppState::new();
    let result = batch(&mut state, json!([add_line("a", 0.0), add_line("b", 5.0)])).unwrap();
    assert_eq!(result["changed"], true);
    assert_eq!(result["revision_before"], "0");
    assert_eq!(result["revision_after"], "1");
    assert_eq!(result["history_step"], "Test edit");
    let (a, b) = (&result["ids"]["a"], &result["ids"]["b"]);
    assert_ne!(a, b);
    assert_eq!(result["changes"]["added"].as_array().unwrap().len(), 2);
    assert_eq!(ids(&state).len(), 2);
    assert!(state.session().is_modified());

    let top = state.session().history().top_undo().unwrap();
    assert_eq!(top.name, "Test edit");
    assert_eq!(top.provenance, Provenance::Automation(GRANT));
    state.docs[0].session.undo();
    assert!(
        state.session().document().is_empty(),
        "one undo undoes the batch"
    );
}

#[test]
fn a_stale_revision_changes_nothing() {
    let mut state = AppState::new();
    state.add_entity(line(0.0));
    let args = json!({"document_id": doc_id(&state), "expected_revision": "0",
                      "operations": [add_line("a", 9.0)]});
    let error =
        dispatch_test(&mut state, INSTANCE, &CALLER, "entities.apply_batch", args).unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRevision);
    assert_eq!(error.details["actual_revision"], "1");
    assert_eq!(ids(&state).len(), 1);
}

#[test]
fn a_failure_on_the_last_operation_restores_everything() {
    let mut state = AppState::new();
    state.add_entity(line(0.0));
    let existing = ids(&state)[0];
    let before = revision(&state);
    let bad = json!({"op": "add", "client_ref": "bad", "entity": {"geometry": {
        "type": "circle", "center": {"x": 0.0, "y": 0.0}, "radius": 0.0}}});
    let result = batch(
        &mut state,
        json!([add_line("a", 10.0), {"op": "delete", "target": {"entity_id": existing.to_string()}}, bad]),
    );
    let error = result.unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidGeometry);
    assert_eq!(error.details["operation"], "2");
    assert_eq!(
        ids(&state),
        [existing],
        "the first two operations were undone"
    );
    assert_eq!(revision(&state), before);
    assert_eq!(
        state.session().history().top_undo().unwrap().name,
        "Add line"
    );
}

#[test]
fn a_mixed_batch_reports_exact_changes_and_undoes_in_one_step() {
    let mut state = AppState::new();
    state.add_entity(line(0.0));
    state.add_entity(line(10.0));
    state.add_entity(line(20.0));
    let [patch_me, replace_me, delete_me] = [ids(&state)[0], ids(&state)[1], ids(&state)[2]];
    let before = state.session().document().objects().to_vec();
    let result = batch(
        &mut state,
        json!([
            add_line("new", 50.0),
            {"op": "patch", "target": {"client_ref": "new"}, "patch": {"geometry": {"type": "line", "end": {"x": 99.0, "y": 0.0}}}},
            {"op": "patch", "target": {"entity_id": patch_me.to_string()},
             "patch": {"geometry": {"type": "line", "start": {"x": -5.0, "y": 0.0}},
                       "attributes": {"color": {"mode": "rgb", "r": 255, "g": 0, "b": 0}}}},
            {"op": "replace", "target": {"entity_id": replace_me.to_string()},
             "entity": {"geometry": {"type": "circle", "center": {"x": 1.0, "y": 1.0}, "radius": 3.0}}},
            {"op": "delete", "target": {"entity_id": delete_me.to_string()}}
        ]),
    )
    .unwrap();
    let changes = &result["changes"];
    assert_eq!(changes["added"].as_array().unwrap().len(), 1);
    assert_eq!(changes["removed"], json!([delete_me.to_string()]));
    let changed = changes["changed"].as_array().unwrap();
    assert!(changed.contains(&json!(patch_me.to_string())));
    assert!(changed.contains(&json!(replace_me.to_string())));

    let document = state.session().document();
    let new_id = EntityId(result["ids"]["new"].as_str().unwrap().parse().unwrap());
    assert_eq!(
        document.get(new_id),
        Some(&Entity::Line {
            start: Point::new(50.0, 0.0),
            end: Point::new(99.0, 0.0)
        }),
        "the patch saw the entity added earlier in the batch"
    );
    assert!(matches!(
        document.get(EntityId(replace_me)),
        Some(Entity::Circle { .. })
    ));
    assert!(document.get(EntityId(delete_me)).is_none());
    assert_eq!(
        document.object(EntityId(patch_me)).unwrap().attrs.color,
        od_core::Attr::Set(od_core::Color::Rgb(255, 0, 0))
    );

    state.docs[0].session.undo();
    assert_eq!(state.session().document().objects(), before.as_slice());
}

#[test]
fn a_batch_that_changes_nothing_records_nothing() {
    let mut state = AppState::new();
    state.add_entity(line(0.0));
    let id = ids(&state)[0];
    let before = revision(&state);
    let same = json!([{"op": "patch", "target": {"entity_id": id.to_string()},
                       "patch": {"geometry": {"type": "line", "start": {"x": 0.0, "y": 0.0}}}}]);
    let result = batch(&mut state, same).unwrap();
    assert_eq!(result["changed"], false);
    assert_eq!(result["revision_after"], before.to_string());
    assert!(result.get("history_step").is_none());
    let empty = batch(&mut state, json!([])).unwrap();
    assert_eq!(empty["changed"], false);
    assert_eq!(
        state.session().history().top_undo().unwrap().name,
        "Add line"
    );
}

#[test]
fn bad_targets_are_refused_before_any_edit() {
    let mut state = AppState::new();
    state.add_entity(line(0.0));
    let id = ids(&state)[0].to_string();
    let before = revision(&state);
    let delete = |id: &str| json!({"op": "delete", "target": {"entity_id": id}});
    assert_eq!(
        code(batch(&mut state, json!([delete("9999")]))),
        ErrorCode::EntityNotFound
    );
    assert_eq!(
        code(batch(&mut state, json!([delete(&id), delete(&id)]))),
        ErrorCode::InvalidArguments,
        "one existing entity, two operations"
    );
    let forward =
        json!([{"op": "delete", "target": {"client_ref": "later"}}, add_line("later", 0.0)]);
    assert_eq!(
        code(batch(&mut state, forward)),
        ErrorCode::InvalidReference
    );
    let twice = json!([add_line("x", 0.0), add_line("x", 1.0)]);
    assert_eq!(code(batch(&mut state, twice)), ErrorCode::InvalidArguments);
    let after_delete = json!([add_line("x", 0.0),
        {"op": "delete", "target": {"client_ref": "x"}},
        {"op": "patch", "target": {"client_ref": "x"}, "patch": {}}]);
    assert_eq!(
        code(batch(&mut state, after_delete)),
        ErrorCode::InvalidReference
    );
    let unknown_layer = json!([{"op": "add", "client_ref": "l", "entity": {
        "geometry": {"type": "line", "start": {"x": 0, "y": 0}, "end": {"x": 1, "y": 0}},
        "attributes": {"layer_id": "42"}}}]);
    assert_eq!(
        code(batch(&mut state, unknown_layer)),
        ErrorCode::InvalidReference
    );
    assert_eq!(revision(&state), before);
    assert_eq!(ids(&state).len(), 1);
}

#[test]
fn the_size_limits_are_enforced() {
    let mut state = AppState::new();
    let many: Vec<Value> = (0..1001)
        .map(|i| add_line(&format!("r{i}"), f64::from(i)))
        .collect();
    assert_eq!(
        code(batch(&mut state, Value::Array(many))),
        ErrorCode::PayloadTooLarge
    );
    let points: Vec<Value> = (0..60_000)
        .map(|i| json!({"x": f64::from(i), "y": 0.0}))
        .collect();
    let big = json!([{"op": "add", "client_ref": "p", "entity": {"geometry": {"type": "polyline", "points": points}}}]);
    assert_eq!(code(batch(&mut state, big)), ErrorCode::PayloadTooLarge);
    assert!(state.session().document().is_empty());
}

#[test]
fn a_running_command_blocks_edits() {
    let mut state = AppState::new();
    state.start_command("BLOCK", crate::block_cmd::define_block);
    state.settle_command();
    assert!(state.command_running());
    let error = batch(&mut state, json!([add_line("a", 0.0)])).unwrap_err();
    assert_eq!(error.code, ErrorCode::CommandBusy);
    assert!(error.retryable);
    assert_eq!(error.details["command"], "BLOCK");
    assert!(state.session().document().is_empty());
}

#[test]
fn an_associative_hatch_pins_the_kind_of_its_boundary() {
    let mut state = AppState::new();
    let square = Entity::Rectangle {
        min: Point::new(0.0, 0.0),
        max: Point::new(10.0, 10.0),
    };
    state.add_entity(square.clone());
    let boundary = ids(&state)[0];
    let (mut loops, _) = loops_from_entities(&[square], 1e-6).unwrap();
    loops[0].sources = vec![BoundarySource::Entity(EntityId(boundary))];
    let pattern = HatchPattern::user_defined(2.0, 0.0, false, Point::default()).unwrap();
    let mut hatch = HatchEntity::new(loops, HatchFill::Pattern(pattern));
    hatch.associative = true;
    state.docs[0]
        .session
        .perform(AddEntity::new(Entity::Hatch(Box::new(hatch))));
    let hatch_id = ids(&state)[1];

    // Changing the kind would orphan the hatch: refused.
    let as_circle = json!([{"op": "replace", "target": {"entity_id": boundary.to_string()},
        "entity": {"geometry": {"type": "circle", "center": {"x": 0.0, "y": 0.0}, "radius": 4.0}}}]);
    assert_eq!(
        code(batch(&mut state, as_circle)),
        ErrorCode::InvalidReference
    );
    // Moving it (same kind) is fine, and the hatch follows in the same step.
    let moved = json!([{"op": "patch", "target": {"entity_id": boundary.to_string()},
        "patch": {"geometry": {"type": "rectangle", "min": {"x": 50.0, "y": 0.0}, "max": {"x": 60.0, "y": 10.0}}}}]);
    let result = batch(&mut state, moved).unwrap();
    assert!(result["changes"]["changed"]
        .as_array()
        .unwrap()
        .contains(&json!(hatch_id.to_string())));
    let Some(Entity::Hatch(h)) = state.session().document().get(EntityId(hatch_id)) else {
        panic!("hatch");
    };
    assert_eq!(h.bounds().unwrap().min, Point::new(50.0, 0.0));
    // The shape of a hatch is read-only through the API, though its colour
    // is not.
    let reshape_hatch = json!([{"op": "patch", "target": {"entity_id": hatch_id.to_string()},
        "patch": {"geometry": {"type": "circle", "center": {"x": 0.0, "y": 0.0}, "radius": 4.0}}}]);
    assert_eq!(
        code(batch(&mut state, reshape_hatch)),
        ErrorCode::UnsupportedEntityWrite
    );
    let recolour_hatch = json!([{"op": "patch", "target": {"entity_id": hatch_id.to_string()},
        "patch": {"attributes": {"color": {"mode": "rgb", "r": 255, "g": 255, "b": 0}}}}]);
    batch(&mut state, recolour_hatch).unwrap();
    let object = state
        .session()
        .document()
        .object(EntityId(hatch_id))
        .unwrap();
    assert_eq!(object.attrs.color, Attr::Set(Color::Rgb(255, 255, 0)));
    state.docs[0].session.undo();
    state.docs[0].session.undo();
    let Some(Entity::Rectangle { min, .. }) = state.session().document().get(EntityId(boundary))
    else {
        panic!("rectangle");
    };
    assert_eq!(
        *min,
        Point::new(0.0, 0.0),
        "one undo moves boundary and hatch back"
    );
}

#[test]
fn deleting_in_a_batch_prunes_the_selection() {
    let mut state = AppState::new();
    state.add_entity(line(0.0));
    state.select_all();
    let id = ids(&state)[0];
    batch(
        &mut state,
        json!([{"op": "delete", "target": {"entity_id": id.to_string()}}]),
    )
    .unwrap();
    assert!(state.docs[0].selection.is_empty());
}

#[test]
fn edits_to_a_drawing_in_the_block_editor_are_refused() {
    let mut state = AppState::new();
    state.add_entity(Entity::Rectangle {
        min: Point::new(10.0, 10.0),
        max: Point::new(12.0, 11.0),
    });
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
    let args = json!({"document_id": doc_id(&state), "expected_revision": "0", "operations": []});
    let error =
        dispatch_test(&mut state, INSTANCE, &CALLER, "entities.apply_batch", args).unwrap_err();
    assert_eq!(error.code, ErrorCode::BlockEditorOpen);
}

#[test]
fn text_can_be_added_and_its_formatting_survives_a_patch() {
    let mut state = AppState::new();
    let add = json!([{"op": "add", "client_ref": "t", "entity": {"geometry": {
        "type": "mtext", "insertion": {"x": 1.0, "y": 2.0}, "contents": "{\\Lunder;Hello}",
        "nominal_height": 2.5, "attachment": "middle_center"}}}]);
    let result = batch(&mut state, add).unwrap();
    let id = result["ids"]["t"].as_str().unwrap().to_owned();
    let patch = json!([{"op": "patch", "target": {"entity_id": id},
        "patch": {"geometry": {"type": "mtext", "nominal_height": 4.0}}}]);
    batch(&mut state, patch).unwrap();
    let Some(Entity::MText(m)) = state
        .session()
        .document()
        .get(EntityId(id.parse().unwrap()))
    else {
        panic!("text");
    };
    assert_eq!(m.nominal_height, 4.0);
    assert_eq!(m.contents, "{\\Lunder;Hello}");
    assert_eq!(m.attachment, od_core::mtext::MTextAttachment::MiddleCenter);
}
