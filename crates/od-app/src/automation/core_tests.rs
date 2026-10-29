//! The automation core: the in-app connection and the checks every request
//! meets, whichever way it arrives.

use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use od_api::limits::QUEUED_REQUESTS_MAX;
use od_api::{ErrorCode, Request, Response, Scope, API_VERSION};
use od_commands::Registry;
use serde_json::{json, Value};

use super::core::{AutomationCore, InProcess, Job};
use super::document_id;
use super::host_tests::{connected, start};
use crate::state::AppState;

const INSTANCE: &str = "inst1";

fn core() -> AutomationCore {
    AutomationCore::new(INSTANCE.to_owned(), Box::new(|| {}))
}

/// The call is made from its own thread while this one polls, as the app
/// does between frames.
fn serve<T: Send>(
    core: &AutomationCore,
    state: &mut AppState,
    registry: &Registry<AppState>,
    call: impl FnOnce() -> T + Send,
) -> T {
    std::thread::scope(|scope| {
        let handle = scope.spawn(call);
        while !handle.is_finished() {
            core.poll(state, registry);
            std::thread::sleep(Duration::from_millis(1));
        }
        handle.join().unwrap()
    })
}

fn code(response: &Response) -> Option<ErrorCode> {
    response.error.as_ref().map(|e| e.code)
}

/// A batch that adds one line to the drawing as it is now.
fn add_a_line(state: &AppState) -> Value {
    json!({
        "document_id": document_id(INSTANCE, state.docs[0].uid),
        "expected_revision": state.session().revision().to_string(),
        "operations": [{"op": "add", "client_ref": "a", "entity": {"geometry": {
            "type": "line", "start": {"x": 0, "y": 0}, "end": {"x": 1, "y": 0}}}}],
    })
}

fn request(scope: Scope, operation: &str, arguments: Value) -> Request {
    Request {
        api_version: API_VERSION.to_string(),
        request_id: "t1".to_owned(),
        deadline_ms: 5000,
        scope,
        operation: operation.to_owned(),
        arguments,
    }
}

fn job(grant_id: u64, request: Request) -> Job {
    Job {
        grant_id,
        request,
        reply: mpsc::channel().0,
        expires: Instant::now() + Duration::from_secs(5),
        abandoned: Arc::new(AtomicBool::new(false)),
    }
}

#[test]
fn an_in_app_connection_starts_read_only_and_is_marked_as_not_an_outside_program() {
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let assistant = core.connect_in_process("OpenDraft assistant").unwrap();
    let grants = core.grants();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].0, assistant.grant_id());
    assert_eq!(grants[0].1.scopes, [Scope::DrawingRead]);
    assert!(grants[0].1.in_process, "so Settings leaves it out");

    let listed = serve(&core, &mut state, &registry, || {
        assistant.call_operation("documents.list", Value::Null)
    });
    assert!(listed.ok, "{listed:?}");
}

#[test]
fn it_cannot_change_a_drawing_until_the_core_is_told_to_allow_it_and_stops_when_that_is_withdrawn()
{
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let assistant = core.connect_in_process("assistant").unwrap();
    let batch = |state: &AppState| add_a_line(state);

    let args = batch(&state);
    let denied = serve(&core, &mut state, &registry, || {
        assistant.call_operation("entities.apply_batch", args)
    });
    assert_eq!(code(&denied), Some(ErrorCode::ScopeDenied));
    assert!(state.session().document().is_empty());

    core.set_write(assistant.grant_id(), true);
    let args = batch(&state);
    let allowed = serve(&core, &mut state, &registry, || {
        assistant.call_operation("entities.apply_batch", args)
    });
    assert!(allowed.ok, "{allowed:?}");
    assert_eq!(state.session().document().len(), 1);
    assert_eq!(
        state.session().history().top_undo().unwrap().provenance,
        od_commands::Provenance::Automation(assistant.grant_id()),
        "attributed to this connection"
    );

    core.set_write(assistant.grant_id(), false);
    let args = batch(&state);
    let denied = serve(&core, &mut state, &registry, || {
        assistant.call_operation("entities.apply_batch", args)
    });
    assert_eq!(code(&denied), Some(ErrorCode::ScopeDenied));
    assert_eq!(
        state.session().document().len(),
        1,
        "nothing more was drawn"
    );
}

#[test]
fn a_request_that_declares_the_wrong_scope_is_refused_before_it_is_queued() {
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let assistant = core.connect_in_process("assistant").unwrap();
    let lie = request(
        Scope::DrawingRead,
        "entities.apply_batch",
        add_a_line(&state),
    );
    let response = serve(&core, &mut state, &registry, || assistant.call(lie));
    assert_eq!(code(&response), Some(ErrorCode::InvalidArguments));
    assert!(state.session().document().is_empty());
}

#[test]
fn what_a_request_says_about_its_scope_is_not_believed_when_it_runs() {
    // Any new way of getting a request to the UI thread might build one that
    // claims the scope its grant has. The core goes by the operation.
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let assistant = core.connect_in_process("assistant").unwrap();
    let claim = request(
        Scope::DrawingRead,
        "entities.apply_batch",
        add_a_line(&state),
    );
    let response = core.execute(&mut state, &registry, &job(assistant.grant_id(), claim));
    assert_eq!(
        code(&response),
        Some(ErrorCode::ScopeDenied),
        "{response:?}"
    );
    assert!(state.session().document().is_empty(), "nothing was drawn");

    // Nor does an operation nobody knows get through by claiming anything.
    let unknown = request(Scope::DrawingRead, "files.delete", Value::Null);
    let response = core.execute(&mut state, &registry, &job(assistant.grant_id(), unknown));
    assert_eq!(code(&response), Some(ErrorCode::InvalidArguments));
}

#[test]
fn a_grant_that_is_gone_gets_nothing_even_for_a_read() {
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let read = request(Scope::DrawingRead, "documents.list", Value::Null);
    let response = core.execute(&mut state, &registry, &job(999, read));
    assert_eq!(code(&response), Some(ErrorCode::ScopeDenied));
}

#[test]
fn dropping_the_connection_gives_its_grant_back() {
    let core = core();
    let assistant = core.connect_in_process("assistant").unwrap();
    let other = core.connect_in_process("another").unwrap();
    assert_ne!(assistant.grant_id(), other.grant_id());
    drop(assistant);
    let left = core.grants();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].0, other.grant_id());
}

#[test]
fn a_prompt_for_the_user_is_taken_away_when_the_connection_loses_its_permission() {
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let assistant = core.connect_in_process("assistant").unwrap();
    core.set_write(assistant.grant_id(), true);
    let id = document_id(INSTANCE, state.docs[0].uid);
    let asked = serve(&core, &mut state, &registry, || {
        assistant.call_operation(
            "selection.request",
            json!({"document_id": id, "prompt": "Select the walls"}),
        )
    });
    assert!(asked.ok, "{asked:?}");
    assert!(state.command_running(), "the user is being asked");

    core.set_write(assistant.grant_id(), false);
    core.poll(&mut state, &registry);
    assert!(!state.command_running(), "the prompt left the screen");
}

#[test]
fn the_queue_is_bounded_for_an_in_app_connection_as_for_a_socket() {
    let core = core();
    let assistant = core.connect_in_process("assistant").unwrap();
    // Nobody polls, so what is accepted waits, and the rest is refused.
    let wanted = QUEUED_REQUESTS_MAX + 8;
    let answers: Vec<Response> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..wanted)
            .map(|n| {
                let assistant = &assistant;
                scope.spawn(move || {
                    let mut read = request(Scope::DrawingRead, "documents.list", Value::Null);
                    read.request_id = format!("flood_{n}");
                    read.deadline_ms = 300;
                    assistant.call(read)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let refused = answers
        .iter()
        .filter(|a| code(a) == Some(ErrorCode::RateLimited))
        .count();
    let timed_out = answers
        .iter()
        .filter(|a| code(a) == Some(ErrorCode::DeadlineExceeded))
        .count();
    assert_eq!((refused, timed_out), (8, QUEUED_REQUESTS_MAX));
}

#[test]
fn a_request_nobody_reached_in_time_is_not_run_later() {
    let core = core();
    let (mut state, registry) = (AppState::new(), crate::commands::build_registry());
    let assistant = core.connect_in_process("assistant").unwrap();
    core.set_write(assistant.grant_id(), true);
    let mut late = request(
        Scope::DrawingWrite,
        "entities.apply_batch",
        add_a_line(&state),
    );
    late.deadline_ms = 20;
    let answer = assistant.call(late); // no poll: the UI never gets there
    assert_eq!(code(&answer), Some(ErrorCode::DeadlineExceeded));
    core.poll(&mut state, &registry);
    assert!(state.session().document().is_empty(), "it must not run now");
}

#[test]
fn an_outside_socket_and_the_in_app_connection_share_one_core() {
    let fixture = start("core-shared");
    let mut state = AppState::new();
    let _outside = connected(&fixture);
    let assistant: InProcess = fixture.host.core().connect_in_process("assistant").unwrap();
    let grants = fixture.host.grants();
    assert_eq!(grants.len(), 2);
    assert_eq!(
        grants.iter().filter(|(_, g)| g.in_process).count(),
        1,
        "one outside program, one in-app connection: {grants:?}"
    );
    let registry = crate::commands::build_registry();
    let answer = std::thread::scope(|scope| {
        let handle = scope.spawn(|| assistant.call_operation("documents.list", Value::Null));
        while !handle.is_finished() {
            fixture.host.poll(&mut state, &registry);
            std::thread::sleep(Duration::from_millis(1));
        }
        handle.join().unwrap()
    });
    assert!(answer.ok, "the host's own poll serves it: {answer:?}");
}
