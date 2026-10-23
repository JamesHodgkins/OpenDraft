//! Idempotency keys: a repeat does nothing and says what was done.

use od_api::ErrorCode;
use od_core::{Entity, Point};
use serde_json::{json, Value};

use super::idempotency::{Replays, Seen};
use super::{dispatch_test, document_id, Caller};
use crate::state::AppState;

const INSTANCE: &str = "inst1";
const ME: Caller = Caller {
    client: "test client",
    grant_id: 7,
    history_any: false,
};
const OTHER: Caller = Caller {
    client: "other",
    grant_id: 8,
    history_any: false,
};

fn call(
    state: &mut AppState,
    caller: &Caller,
    op: &str,
    mut args: Value,
) -> Result<Value, od_api::ApiError> {
    if args.get("document_id") == Some(&json!("active")) {
        args["document_id"] = json!(document_id(INSTANCE, state.docs[state.active].uid));
    }
    dispatch_test(state, INSTANCE, caller, op, args)
}

fn add_line(client_ref: &str, x: f64) -> Value {
    json!({"op": "add", "client_ref": client_ref, "entity": {"geometry": {
        "type": "line", "start": {"x": x, "y": 0.0}, "end": {"x": x + 1.0, "y": 0.0}}}})
}

fn batch(key: Option<&str>, x: f64) -> Value {
    let mut args = json!({"document_id": "active", "expected_revision": "0", "operations": [add_line("a", x)]});
    if let Some(key) = key {
        args["idempotency_key"] = json!(key);
    }
    args
}

#[test]
fn asking_again_with_the_same_key_changes_nothing_and_says_so() {
    let mut state = AppState::new();
    let first = call(
        &mut state,
        &ME,
        "entities.apply_batch",
        batch(Some("k1"), 0.0),
    )
    .unwrap();
    assert!(first.get("replayed").is_none());
    assert_eq!(state.session().document().len(), 1);
    let revision = state.session().revision();

    let again = call(
        &mut state,
        &ME,
        "entities.apply_batch",
        batch(Some("k1"), 0.0),
    )
    .unwrap();
    assert_eq!(again["replayed"], true);
    assert_eq!(
        again["ids"], first["ids"],
        "the original answer, ids included"
    );
    assert_eq!(again["history_token"], first["history_token"]);
    assert_eq!(state.session().document().len(), 1, "not drawn twice");
    assert_eq!(state.session().revision(), revision);
}

#[test]
fn without_a_key_the_same_repeat_is_refused_as_stale() {
    let mut state = AppState::new();
    call(&mut state, &ME, "entities.apply_batch", batch(None, 0.0)).unwrap();
    let error = call(&mut state, &ME, "entities.apply_batch", batch(None, 0.0)).unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRevision);
}

#[test]
fn the_same_key_with_other_arguments_is_an_error_not_a_guess() {
    let mut state = AppState::new();
    call(
        &mut state,
        &ME,
        "entities.apply_batch",
        batch(Some("k1"), 0.0),
    )
    .unwrap();
    let error = call(
        &mut state,
        &ME,
        "entities.apply_batch",
        batch(Some("k1"), 50.0),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::IdempotencyKeyReused);
    assert_eq!(error.details["idempotency_key"], "k1");
    assert_eq!(state.session().document().len(), 1);
}

#[test]
fn an_attempt_that_failed_is_not_remembered_so_trying_again_works() {
    let mut state = AppState::new();
    let bad = json!({"document_id": "active", "expected_revision": "0", "idempotency_key": "k1",
        "operations": [{"op": "add", "client_ref": "a", "entity": {"geometry": {
            "type": "circle", "center": {"x": 0.0, "y": 0.0}, "radius": 0.0}}}]});
    assert_eq!(
        call(&mut state, &ME, "entities.apply_batch", bad)
            .unwrap_err()
            .code,
        ErrorCode::InvalidGeometry
    );
    // A corrected request under the same key is a new request, not a clash.
    let fixed = call(
        &mut state,
        &ME,
        "entities.apply_batch",
        batch(Some("k1"), 0.0),
    )
    .unwrap();
    assert!(fixed.get("replayed").is_none());
    assert_eq!(state.session().document().len(), 1);
}

#[test]
fn a_key_belongs_to_one_operation_and_one_connection() {
    let mut state = AppState::new();
    call(
        &mut state,
        &ME,
        "entities.apply_batch",
        batch(Some("k1"), 0.0),
    )
    .unwrap();
    let layers = json!({"document_id": "active", "expected_revision": "1", "idempotency_key": "k1",
        "operations": [{"op": "create", "client_ref": "w", "name": "Walls"}]});
    let made = call(&mut state, &ME, "layers.apply_batch", layers).unwrap();
    assert!(
        made.get("replayed").is_none(),
        "another operation: its own key space"
    );
    assert_eq!(state.session().document().layers().len(), 2);

    // Another connection with the same key does its own work.
    let mut args = batch(Some("k1"), 0.0);
    args["expected_revision"] = json!(state.session().revision().to_string());
    let theirs = call(&mut state, &OTHER, "entities.apply_batch", args).unwrap();
    assert!(theirs.get("replayed").is_none());
    assert_eq!(state.session().document().len(), 2);
}

#[test]
fn bad_keys_are_refused_and_reads_take_none() {
    let mut state = AppState::new();
    for bad in [
        json!(""),
        json!("a b"),
        json!("é"),
        json!("k".repeat(65)),
        json!(5),
        json!(null),
    ] {
        let mut args = batch(None, 0.0);
        args["idempotency_key"] = bad.clone();
        let error = call(&mut state, &ME, "entities.apply_batch", args).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArguments, "{bad}");
    }
    assert!(state.session().document().is_empty());
    // A read that checks its arguments does not accept a key.
    let query = json!({"document_id": "active", "idempotency_key": "x"});
    let error = call(&mut state, &ME, "entities.query", query).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArguments);
}

#[test]
fn a_repeated_undo_undoes_once() {
    let mut state = AppState::new();
    let made = call(&mut state, &ME, "entities.apply_batch", batch(None, 0.0)).unwrap();
    let undo = |revision: &str| {
        json!({"document_id": "active", "expected_revision": revision, "step_token": made["history_token"],
               "idempotency_key": "undo-1"})
    };
    let first = call(&mut state, &ME, "history.undo", undo("1")).unwrap();
    assert!(state.session().document().is_empty());
    let revision = state.session().revision();
    // The caller does not know it went through and asks again, as it saw it.
    let again = call(&mut state, &ME, "history.undo", undo("1")).unwrap();
    assert_eq!(again["replayed"], true);
    assert_eq!(again["step_name"], first["step_name"]);
    assert_eq!(state.session().revision(), revision);
}

#[test]
fn a_toggle_command_flips_once_per_key() {
    let mut state = AppState::new();
    let toggle = |key: &str| json!({"command_id": "view.grid", "document_id": "active", "idempotency_key": key});
    call(&mut state, &ME, "commands.invoke", toggle("t1")).unwrap();
    assert!(!state.settings.show_grid);
    let again = call(&mut state, &ME, "commands.invoke", toggle("t1")).unwrap();
    assert_eq!(again["replayed"], true);
    assert!(!state.settings.show_grid, "not flipped back");
    call(&mut state, &ME, "commands.invoke", toggle("t2")).unwrap();
    assert!(state.settings.show_grid, "a new key is a new request");
}

#[test]
fn a_repeated_save_as_is_one_operation() {
    let dir = std::env::temp_dir().join(format!("od-keys-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("approved")).unwrap();
    let mut state = AppState::new();
    state.settings.automation_roots = vec![dir.join("approved")];
    state.add_entity(Entity::Line {
        start: Point::new(0.0, 0.0),
        end: Point::new(1.0, 0.0),
    });
    let path = dir.join("approved/plan.odx").to_string_lossy().into_owned();
    let args = json!({"document_id": "active", "path": path, "idempotency_key": "s1"});
    let first = call(&mut state, &ME, "files.save_as", args.clone()).unwrap();
    let again = call(&mut state, &ME, "files.save_as", args).unwrap();
    assert_eq!(again["replayed"], true);
    assert_eq!(
        again["operation_id"], first["operation_id"],
        "one operation, not two"
    );
    assert_eq!(state.file_ops.unfinished(ME.grant_id), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_so_many_answers_are_kept_and_an_old_key_just_runs_again() {
    let mut state = AppState::new();
    let toggle = |key: &str| json!({"command_id": "view.grid", "document_id": "active", "idempotency_key": key});
    for i in 0..300 {
        call(&mut state, &ME, "commands.invoke", toggle(&format!("k{i}"))).unwrap();
    }
    assert!(state.settings.show_grid, "an even number of flips");
    let recent = call(&mut state, &ME, "commands.invoke", toggle("k299")).unwrap();
    assert_eq!(recent["replayed"], true);
    assert!(state.settings.show_grid, "k299 is still remembered");
    let old = call(&mut state, &ME, "commands.invoke", toggle("k0")).unwrap();
    assert!(old.get("replayed").is_none(), "k0 was pushed out");
    assert!(!state.settings.show_grid);
}

#[test]
fn a_huge_answer_is_not_kept() {
    let mut replays = Replays::default();
    let big = json!({"data": "x".repeat(300 * 1024)});
    let args = json!({"a": 1});
    replays.keep(1, "files.open", "k", &args, &big);
    assert!(matches!(
        replays.look(1, "files.open", "k", &args),
        Seen::New
    ));
    replays.keep(1, "files.open", "k", &args, &json!({"ok": true}));
    assert!(matches!(
        replays.look(1, "files.open", "k", &args),
        Seen::Answer(_)
    ));
    assert!(matches!(
        replays.look(1, "files.open", "k", &json!({"a": 2})),
        Seen::Reused
    ));
}

#[test]
fn the_order_of_fields_does_not_make_a_repeat_look_different() {
    let mut state = AppState::new();
    let one = json!({"command_id": "view.grid", "document_id": "active", "idempotency_key": "o1"});
    call(&mut state, &ME, "commands.invoke", one).unwrap();
    let flipped =
        json!({"idempotency_key": "o1", "document_id": "active", "command_id": "view.grid"});
    let again = call(&mut state, &ME, "commands.invoke", flipped).unwrap();
    assert_eq!(again["replayed"], true);
}
