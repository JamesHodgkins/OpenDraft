//! The refusals and limits of the file operations: paths, consent, who owns
//! an operation, cancelling, and scopes. The happy paths are in `file_tests`.

use std::time::Duration;

use od_api::ErrorCode;
use serde_json::json;

use super::file_state::{Kind, Plan};
use super::file_tests::{app, call, line, make_file, ok, Area, ME, OTHER};
use super::{answer_consent, consent_text};
use od_core::{Entity, Point};

#[test]
fn paths_outside_the_approved_folders_or_of_the_wrong_kind_never_start_anything() {
    let area = Area::new("denied");
    make_file(&area.path("elsewhere/secret.odx"), 1);
    make_file(&area.path("approved/ok.odx"), 1);
    std::fs::write(area.path("approved/notes.txt"), b"x").unwrap();
    let mut state = app(&area);
    let sneaky = format!(
        "{}{sep}approved{sep}..{sep}elsewhere{sep}secret.odx",
        area.0.display(),
        sep = std::path::MAIN_SEPARATOR
    );
    for path in [
        area.arg("elsewhere/secret.odx"),
        sneaky,
        area.arg("approved/notes.txt"),
        area.arg("approved/missing.odx"),
        "relative.odx".to_owned(),
        String::new(),
    ] {
        let error = call(&mut state, &ME, "files.open", json!({"path": path})).unwrap_err();
        assert_eq!(error.code, ErrorCode::PathDenied, "{path:?}");
    }
    assert_eq!(state.docs.len(), 1);
    assert!(state.session().document().is_empty());
    // With no folder approved at all, nothing is reachable.
    state.settings.automation_roots.clear();
    let error = call(
        &mut state,
        &ME,
        "files.open",
        json!({"path": area.arg("approved/ok.odx")}),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::PathDenied);
    assert!(error.message.contains("Settings"));
}

#[test]
fn declining_leaves_the_file_alone_and_says_no_to_the_caller() {
    let area = Area::new("decline");
    make_file(&area.path("approved/keep.odx"), 4);
    let mut state = app(&area);
    state.add_entity(line(0.0));
    let waiting = ok(
        &mut state,
        "files.save_as",
        json!({"document_id": "active", "path": area.arg("approved/keep.odx")}),
    );
    let (id, _) = consent_text(&state).unwrap();
    answer_consent(&mut state, id, false);
    let done = ok(
        &mut state,
        "operations.get",
        json!({"operation_id": waiting["operation_id"]}),
    );
    assert_eq!(done["state"], "failed");
    assert_eq!(done["error"]["code"], "consent_required");
    assert_eq!(
        od_files::load(&area.path("approved/keep.odx"))
            .unwrap()
            .document
            .len(),
        4
    );
    assert!(
        state.session().is_modified(),
        "still unsaved, still tied to nothing"
    );
    assert!(state.docs[0].path.is_none());
    // Answering again, or later, does nothing.
    answer_consent(&mut state, id, true);
    assert_eq!(
        od_files::load(&area.path("approved/keep.odx"))
            .unwrap()
            .document
            .len(),
        4
    );
}

#[test]
fn an_unanswered_question_expires_and_a_cancelled_one_goes_away() {
    let area = Area::new("expire");
    make_file(&area.path("approved/a.odx"), 1);
    make_file(&area.path("approved/b.odx"), 1);
    let mut state = app(&area);
    state.add_entity(line(0.0));
    let first = ok(
        &mut state,
        "files.save_as",
        json!({"document_id": "active", "path": area.arg("approved/a.odx")}),
    );
    let second = ok(
        &mut state,
        "files.save_as",
        json!({"document_id": "active", "path": area.arg("approved/b.odx")}),
    );
    let cancelled = ok(
        &mut state,
        "operations.cancel",
        json!({"operation_id": second["operation_id"]}),
    );
    assert_eq!(cancelled["state"], "cancelled");
    assert_eq!(cancelled["cancellable"], false);
    state.file_ops.expire_after(Duration::ZERO);
    let late = ok(
        &mut state,
        "operations.get",
        json!({"operation_id": first["operation_id"]}),
    );
    assert_eq!(late["error"]["code"], "deadline_exceeded");
    assert!(consent_text(&state).is_none());
    assert_eq!(
        od_files::load(&area.path("approved/a.odx"))
            .unwrap()
            .document
            .len(),
        1
    );
    assert_eq!(
        od_files::load(&area.path("approved/b.odx"))
            .unwrap()
            .document
            .len(),
        1
    );
}

#[test]
fn operations_belong_to_the_connection_that_started_them() {
    let area = Area::new("owner");
    make_file(&area.path("approved/plan.odx"), 1);
    let mut state = app(&area);
    let status = ok(
        &mut state,
        "files.open",
        json!({"path": area.arg("approved/plan.odx")}),
    );
    let id = status["operation_id"].clone();
    let error = call(
        &mut state,
        &OTHER,
        "operations.get",
        json!({"operation_id": id}),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArguments);
    for bad in ["op_999", "nope", "op_", "op_-1"] {
        assert!(call(
            &mut state,
            &ME,
            "operations.get",
            json!({"operation_id": bad})
        )
        .is_err());
    }
    assert!(call(
        &mut state,
        &OTHER,
        "operations.cancel",
        json!({"operation_id": id})
    )
    .is_err());
}

#[test]
fn one_connection_cannot_pile_up_questions_without_limit() {
    let area = Area::new("limit");
    let mut state = app(&area);
    state.add_entity(line(0.0));
    for i in 0..8 {
        make_file(&area.path(&format!("approved/f{i}.odx")), 1);
        let status = ok(
            &mut state,
            "files.save_as",
            json!({"document_id": "active", "path": area.arg(&format!("approved/f{i}.odx"))}),
        );
        assert_eq!(status["state"], "awaiting_consent");
    }
    make_file(&area.path("approved/f9.odx"), 1);
    let error = call(
        &mut state,
        &ME,
        "files.save_as",
        json!({"document_id": "active", "path": area.arg("approved/f9.odx")}),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationBusy);
    assert!(error.retryable);
    // Another connection is not held up by it.
    assert!(call(
        &mut state,
        &OTHER,
        "files.save_as",
        json!({"document_id": "active", "path": area.arg("approved/f9.odx")})
    )
    .is_ok());
}

#[test]
fn cancelling_a_running_operation_stops_only_that_job() {
    let area = Area::new("cancel");
    make_file(&area.path("approved/plan.odx"), 1);
    let mut state = app(&area);
    state.io = crate::io::Io::background();
    let (release, gate) = std::sync::mpsc::channel::<()>();
    let (user_release, user_gate) = std::sync::mpsc::channel::<()>();
    let stuck = state.io.submit_cancellable(move |_| {
        let _ = gate.recv_timeout(Duration::from_secs(20));
        crate::io::Done::Finished
    });
    let users = state.io.submit_cancellable(move |_| {
        let _ = user_gate.recv_timeout(Duration::from_secs(20));
        crate::io::Done::Finished
    });
    let id = state.file_ops.add(
        ME.grant_id,
        "test client",
        Kind::Open,
        Plan::Open,
        area.path("approved/plan.odx"),
        None,
    );
    state.file_ops.get_mut(id).unwrap().job = Some(stuck);
    state.opening.push(area.path("approved/plan.odx"));
    let cancelled = ok(
        &mut state,
        "operations.cancel",
        json!({"operation_id": format!("op_{id}")}),
    );
    assert_eq!(cancelled["state"], "cancelled");
    assert!(state.opening.is_empty(), "it can be opened again");
    assert!(state.io.busy(), "the user's own job is still running");
    assert!(!state.io.cancel_job(stuck), "already given up on");
    assert!(state.io.cancel_job(users));
    let _ = (release, user_release);
    // Cancelling something that has finished just reports it.
    let again = ok(
        &mut state,
        "operations.cancel",
        json!({"operation_id": format!("op_{id}")}),
    );
    assert_eq!(again["state"], "cancelled");
}

#[test]
fn a_drawing_in_the_block_editor_is_not_written() {
    let area = Area::new("bedit");
    let mut state = app(&area);
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
    let error = call(
        &mut state,
        &ME,
        "files.save_as",
        json!({"document_id": "active", "path": area.arg("approved/x.odx")}),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::BlockEditorOpen);
    assert!(!area.path("approved/x.odx").exists());
}

#[test]
fn file_tools_need_the_file_scopes() {
    use od_api::{ops, Scope};
    assert_eq!(ops::required_scope(ops::FILES_OPEN), Some(Scope::FileRead));
    // Watching or stopping your own operation takes only the baseline.
    for op in [ops::OPERATIONS_GET, ops::OPERATIONS_CANCEL] {
        assert_eq!(ops::required_scope(op), Some(Scope::DrawingRead));
    }
    for op in [ops::FILES_SAVE, ops::FILES_SAVE_AS, ops::FILES_EXPORT] {
        assert_eq!(ops::required_scope(op), Some(Scope::FileWrite));
    }
}
