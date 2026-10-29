//! What a connection may do beyond its standing scopes: an approval for one
//! exact request, and a task's scopes.

use std::time::Duration;

use od_api::{ErrorCode, Scope};
use serde_json::{json, Value};

use super::core_tests::{code, core, serve, INSTANCE};
use super::document_id;
use super::grants::ApprovalError;
use crate::state::AppState;

/// A batch adding one line at `x`, based on the drawing as it is now.
fn line_at(state: &AppState, x: f64) -> Value {
    json!({
        "document_id": document_id(INSTANCE, state.docs[0].uid),
        "expected_revision": state.session().revision().to_string(),
        "operations": [{"op": "add", "client_ref": "a", "entity": {"geometry": {
            "type": "line", "start": {"x": x, "y": 0}, "end": {"x": x + 1.0, "y": 0}}}}],
    })
}

fn toggle_grid(state: &AppState) -> Value {
    json!({"command_id": "view.grid", "document_id": document_id(INSTANCE, state.docs[0].uid)})
}

#[test]
fn an_approved_request_runs_once_and_only_once() {
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let assistant = core.connect_in_process("assistant").unwrap();
    let args = toggle_grid(&state);
    assert!(core
        .approve_once(assistant.grant_id(), "commands.invoke", &args)
        .is_ok());
    let before = state.settings.show_grid;

    let first = serve(&core, &mut state, &registry, || {
        assistant.call_operation("commands.invoke", args.clone())
    });
    assert!(first.ok, "{first:?}");
    assert_ne!(state.settings.show_grid, before, "it ran");

    let again = serve(&core, &mut state, &registry, || {
        assistant.call_operation("commands.invoke", args.clone())
    });
    assert_eq!(code(&again), Some(ErrorCode::ScopeDenied), "used up");
    assert_ne!(state.settings.show_grid, before, "and did not run twice");
}

#[test]
fn an_approval_is_for_that_exact_request_and_nothing_else() {
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let assistant = core.connect_in_process("assistant").unwrap();
    let shown = line_at(&state, 0.0);
    core.approve_once(assistant.grant_id(), "entities.apply_batch", &shown)
        .unwrap();

    // The user was shown a line at 0; a line at 5 is another request.
    let other = line_at(&state, 5.0);
    let denied = serve(&core, &mut state, &registry, || {
        assistant.call_operation("entities.apply_batch", other)
    });
    assert_eq!(code(&denied), Some(ErrorCode::ScopeDenied));
    assert!(state.session().document().is_empty());

    // The same arguments under another operation are another request too.
    let wrong_tool = serve(&core, &mut state, &registry, || {
        assistant.call_operation("layers.apply_batch", shown.clone())
    });
    assert_eq!(code(&wrong_tool), Some(ErrorCode::ScopeDenied));

    // Neither used it up: the approved request still runs.
    let allowed = serve(&core, &mut state, &registry, || {
        assistant.call_operation("entities.apply_batch", shown)
    });
    assert!(allowed.ok, "{allowed:?}");
    assert_eq!(state.session().document().len(), 1);
    assert_eq!(
        state.session().history().top_undo().unwrap().provenance,
        od_commands::Provenance::Automation(assistant.grant_id())
    );
}

#[test]
fn the_retry_key_is_not_part_of_what_was_approved() {
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let assistant = core.connect_in_process("assistant").unwrap();
    let shown = line_at(&state, 0.0);
    core.approve_once(assistant.grant_id(), "entities.apply_batch", &shown)
        .unwrap();
    let mut with_key = shown;
    with_key["idempotency_key"] = json!("try-1");
    let allowed = serve(&core, &mut state, &registry, || {
        assistant.call_operation("entities.apply_batch", with_key)
    });
    assert!(allowed.ok, "{allowed:?}");
}

#[test]
fn what_cannot_be_approved_is_refused_and_so_is_an_unknown_connection() {
    let core = core();
    let assistant = core.connect_in_process("assistant").unwrap();
    let id = assistant.grant_id();
    let args = json!({});
    for read_or_prompt_or_unknown in ["documents.list", "selection.request", "files.delete"] {
        assert_eq!(
            core.approve_once(id, read_or_prompt_or_unknown, &args),
            Err(ApprovalError::NotApprovable),
            "{read_or_prompt_or_unknown}"
        );
    }
    assert_eq!(
        core.approve_once(999, "entities.apply_batch", &args),
        Err(ApprovalError::NoSuchConnection)
    );
    for n in 0..8 {
        let args = json!({ "n": n });
        assert!(core.approve_once(id, "entities.apply_batch", &args).is_ok());
    }
    assert_eq!(
        core.approve_once(id, "entities.apply_batch", &json!({"n": 9})),
        Err(ApprovalError::TooMany),
        "a connection cannot pile up approvals"
    );
    core.forget_approvals(id);
    assert!(core
        .approve_once(id, "entities.apply_batch", &json!({"n": 9}))
        .is_ok());
}

#[test]
fn an_approval_that_is_not_used_in_time_is_gone() {
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let assistant = core.connect_in_process("assistant").unwrap();
    let args = toggle_grid(&state);
    core.approve_for(
        assistant.grant_id(),
        "commands.invoke",
        &args,
        Duration::from_millis(1),
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(20));
    let late = serve(&core, &mut state, &registry, || {
        assistant.call_operation("commands.invoke", args)
    });
    assert_eq!(code(&late), Some(ErrorCode::ScopeDenied));
}

#[test]
fn approvals_the_user_withdraws_are_not_honoured() {
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let assistant = core.connect_in_process("assistant").unwrap();
    let args = line_at(&state, 0.0);
    core.approve_once(assistant.grant_id(), "entities.apply_batch", &args)
        .unwrap();
    core.forget_approvals(assistant.grant_id());
    let denied = serve(&core, &mut state, &registry, || {
        assistant.call_operation("entities.apply_batch", args)
    });
    assert_eq!(code(&denied), Some(ErrorCode::ScopeDenied));
    assert!(state.session().document().is_empty());
}

#[test]
fn a_task_holds_its_scopes_counts_its_changes_and_stops_at_its_limit() {
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let assistant = core.connect_in_process("assistant").unwrap();
    let id = assistant.grant_id();
    let held = core
        .begin_task(id, &[Scope::DrawingWrite], Some(2))
        .unwrap();
    assert_eq!(held, [Scope::DrawingWrite]);
    for x in [0.0, 5.0] {
        let args = line_at(&state, x);
        let done = serve(&core, &mut state, &registry, || {
            assistant.call_operation("entities.apply_batch", args)
        });
        assert!(done.ok, "{done:?}");
    }
    assert_eq!(core.task(id).unwrap().changes, 2);

    let args = line_at(&state, 10.0);
    let third = serve(&core, &mut state, &registry, || {
        assistant.call_operation("entities.apply_batch", args)
    });
    assert_eq!(code(&third), Some(ErrorCode::ScopeDenied));
    let error = third.error.unwrap();
    assert!(
        error.message.contains("all the changes it was allowed"),
        "{error:?}"
    );
    assert_eq!(error.details.get("limit").map(String::as_str), Some("2"));
    assert_eq!(state.session().document().len(), 2, "no third line");
}

#[test]
fn ending_a_task_takes_its_scopes_back_and_leaves_what_the_user_switched_on() {
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let assistant = core.connect_in_process("assistant").unwrap();
    let id = assistant.grant_id();
    core.begin_task(id, &[Scope::DrawingWrite], None).unwrap();
    let args = line_at(&state, 0.0);
    assert!(
        serve(&core, &mut state, &registry, || assistant
            .call_operation("entities.apply_batch", args))
        .ok
    );
    core.end_task(id);
    assert!(core.task(id).is_none());
    let args = line_at(&state, 5.0);
    let denied = serve(&core, &mut state, &registry, || {
        assistant.call_operation("entities.apply_batch", args)
    });
    assert_eq!(code(&denied), Some(ErrorCode::ScopeDenied));

    // What the user switched on is theirs, and a task neither adds to it nor
    // takes it away.
    core.set_write(id, true);
    core.begin_task(id, &[Scope::DrawingWrite], Some(0))
        .unwrap();
    core.end_task(id);
    let args = line_at(&state, 5.0);
    assert!(
        serve(&core, &mut state, &registry, || assistant
            .call_operation("entities.apply_batch", args))
        .ok
    );
}

#[test]
fn a_task_can_never_hold_files_or_the_history_of_others() {
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let assistant = core.connect_in_process("assistant").unwrap();
    let wanted = [
        Scope::DrawingWrite,
        Scope::FileWrite,
        Scope::FileRead,
        Scope::HistoryAny,
    ];
    let held = core
        .begin_task(assistant.grant_id(), &wanted, None)
        .unwrap();
    assert_eq!(held, [Scope::DrawingWrite]);
    let denied = serve(&core, &mut state, &registry, || {
        assistant.call_operation("files.open", json!({"path": "C:/anywhere/x.odx"}))
    });
    assert_eq!(code(&denied), Some(ErrorCode::ScopeDenied));
    assert!(core.task(assistant.grant_id()).unwrap().scopes == [Scope::DrawingWrite]);
}

#[test]
fn an_approval_is_used_before_the_task_counts_anything() {
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let assistant = core.connect_in_process("assistant").unwrap();
    let id = assistant.grant_id();
    core.begin_task(id, &[Scope::DrawingWrite], Some(1))
        .unwrap();
    let shown = line_at(&state, 0.0);
    core.approve_once(id, "entities.apply_batch", &shown)
        .unwrap();
    assert!(
        serve(&core, &mut state, &registry, || assistant
            .call_operation("entities.apply_batch", shown))
        .ok
    );
    assert_eq!(core.task(id).unwrap().changes, 0, "that one was approved");

    let next = line_at(&state, 5.0);
    assert!(
        serve(&core, &mut state, &registry, || assistant
            .call_operation("entities.apply_batch", next))
        .ok
    );
    assert_eq!(core.task(id).unwrap().changes, 1);
    let last = line_at(&state, 10.0);
    let denied = serve(&core, &mut state, &registry, || {
        assistant.call_operation("entities.apply_batch", last)
    });
    assert_eq!(
        code(&denied),
        Some(ErrorCode::ScopeDenied),
        "the limit is reached"
    );
}

#[test]
fn a_prompt_for_the_user_goes_when_the_task_that_allowed_it_ends() {
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let assistant = core.connect_in_process("assistant").unwrap();
    let id = assistant.grant_id();
    core.begin_task(id, &[Scope::UiSelection], None).unwrap();
    let doc = document_id(INSTANCE, state.docs[0].uid);
    let asked = serve(&core, &mut state, &registry, || {
        assistant.call_operation(
            "selection.request",
            json!({"document_id": doc, "prompt": "Select the walls"}),
        )
    });
    assert!(asked.ok, "{asked:?}");
    assert!(state.command_running(), "the user is being asked");
    core.poll(&mut state, &registry);
    assert!(state.command_running(), "still allowed while the task runs");

    core.end_task(id);
    core.poll(&mut state, &registry);
    assert!(!state.command_running(), "the prompt left the screen");
}

#[test]
fn closing_the_connection_takes_its_approvals_and_task_with_it() {
    let core = core();
    let assistant = core.connect_in_process("assistant").unwrap();
    let id = assistant.grant_id();
    core.begin_task(id, &[Scope::DrawingWrite], None).unwrap();
    core.approve_once(id, "entities.apply_batch", &json!({}))
        .unwrap();
    drop(assistant);
    assert!(core.task(id).is_none());
    assert!(core.grants().is_empty());
    assert_eq!(
        core.approve_once(id, "entities.apply_batch", &json!({})),
        Err(ApprovalError::NoSuchConnection)
    );
}
