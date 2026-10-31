//! The host over a real local socket (a named pipe on Windows), with the
//! test thread standing in for the UI thread by calling `poll`.

use std::path::PathBuf;
use std::time::Duration;

use interprocess::local_socket::{prelude::*, Stream};
use od_api::frame::{read_frame, write_frame};
use od_api::handshake::{Hello, HelloReply};
use od_api::instance::{self, Descriptor};
use od_api::limits::QUEUED_REQUESTS_MAX;
use od_api::{ErrorCode, Response, Scope};
use serde_json::{json, Value};

use super::core::AutomationCore;
use super::host::{socket_name, SocketServer};
use crate::state::AppState;

/// A core with a socket in front of it, as the app has when the setting is
/// on. It answers for the core, so a test calls `poll` and the like on it.
pub(super) struct TestHost {
    core: AutomationCore,
    server: Option<SocketServer>,
}

impl TestHost {
    pub(super) fn core(&self) -> &AutomationCore {
        &self.core
    }

    /// Stop the socket, as turning the setting off does.
    pub(super) fn stop_socket(&mut self) {
        self.server = None;
    }
}

impl std::ops::Deref for TestHost {
    type Target = AutomationCore;

    fn deref(&self) -> &AutomationCore {
        &self.core
    }
}

pub(super) struct Fixture {
    pub(super) host: TestHost,
    pub(super) dir: PathBuf,
    pub(super) descriptor: Descriptor,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub(super) fn start(name: &str) -> Fixture {
    let dir = std::env::temp_dir().join(format!("od-host-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let core = AutomationCore::new(super::core::fresh_instance_id(), Box::new(|| {}));
    let server = SocketServer::start(&dir, &core).unwrap();
    let host = TestHost {
        core,
        server: Some(server),
    };
    let descriptor = instance::list(&dir).pop().unwrap();
    Fixture {
        host,
        dir,
        descriptor,
    }
}

pub(super) fn connect(descriptor: &Descriptor) -> Stream {
    Stream::connect(socket_name(&descriptor.endpoint).unwrap()).unwrap()
}

pub(super) fn hello(stream: &mut Stream, token: &str, versions: &[u32]) -> HelloReply {
    let hello = Hello {
        token: token.to_owned(),
        api_versions: versions.to_vec(),
        client_name: "test".to_owned(),
    };
    write_frame(stream, &serde_json::to_vec(&hello).unwrap()).unwrap();
    serde_json::from_slice(&read_frame(stream).unwrap()).unwrap()
}

pub(super) fn request(op: &str, scope: &str, args: Value, deadline_ms: u64) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "api_version": "1", "request_id": "r1", "deadline_ms": deadline_ms,
        "scope": scope, "operation": op, "arguments": args,
    }))
    .unwrap()
}

/// Send `frame` and wait for the answer while the "UI" polls.
pub(super) fn call(
    mut stream: Stream,
    fixture: &Fixture,
    state: &mut AppState,
    frame: &[u8],
) -> (Stream, Response) {
    write_frame(&mut stream, frame).unwrap();
    let reader = std::thread::spawn(move || {
        let reply = read_frame(&mut stream).unwrap();
        (stream, reply)
    });
    while !reader.is_finished() {
        fixture.host.poll(state, &crate::commands::build_registry());
        std::thread::sleep(Duration::from_millis(1));
    }
    let (stream, bytes) = reader.join().unwrap();
    (stream, serde_json::from_slice(&bytes).unwrap())
}

pub(super) fn connected(fixture: &Fixture) -> Stream {
    let mut stream = connect(&fixture.descriptor);
    let reply = hello(&mut stream, &fixture.descriptor.token, &[1]);
    assert!(reply.ok, "{reply:?}");
    stream
}

#[test]
fn a_client_with_the_token_reads_the_documents() {
    let fixture = start("read");
    let mut stream = connect(&fixture.descriptor);
    let reply = hello(&mut stream, &fixture.descriptor.token, &[1, 2]);
    assert_eq!(reply.api_version, Some(1));
    assert_eq!(reply.scopes, [Scope::DrawingRead]);
    assert_eq!(fixture.host.grants().len(), 1);

    let mut state = AppState::new();
    let frame = request("documents.list", "drawing.read", Value::Null, 5000);
    let (stream, response) = call(stream, &fixture, &mut state, &frame);
    assert!(response.ok, "{response:?}");
    let id = &response.result.unwrap()["documents"][0]["document_id"];
    assert_eq!(*id, format!("doc_{}_1", fixture.host.instance_id()));

    // The same connection serves a second request.
    let frame = request(
        "document.get",
        "drawing.read",
        json!({"document_id": id}),
        5000,
    );
    let (_, response) = call(stream, &fixture, &mut state, &frame);
    assert_eq!(response.result.unwrap()["entity_count"], 0);
}

#[test]
fn a_wrong_token_or_version_is_refused_and_gets_no_grant() {
    let fixture = start("auth");
    let mut stream = connect(&fixture.descriptor);
    let reply = hello(&mut stream, "not-the-token", &[1]);
    assert!(!reply.ok);
    assert_eq!(reply.error.unwrap().code, ErrorCode::ScopeDenied);
    assert!(read_frame(&mut stream).is_err(), "connection closed");

    let mut stream = connect(&fixture.descriptor);
    let reply = hello(&mut stream, &fixture.descriptor.token, &[7]);
    assert!(!reply.ok);
    assert_eq!(reply.error.unwrap().code, ErrorCode::InvalidArguments);
    assert!(fixture.host.grants().is_empty());
}

/// A fixture whose server drops a connection that sends no hello for `wait`.
pub(super) fn start_impatient(name: &str, wait: Duration) -> Fixture {
    let dir = std::env::temp_dir().join(format!("od-host-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let core = AutomationCore::new(super::core::fresh_instance_id(), Box::new(|| {}));
    let server = SocketServer::start_with(&dir, &core, wait).unwrap();
    let host = TestHost {
        core,
        server: Some(server),
    };
    let descriptor = instance::list(&dir).pop().unwrap();
    Fixture {
        host,
        dir,
        descriptor,
    }
}

#[test]
fn a_request_before_the_hello_is_not_served() {
    let fixture = start("nohello");
    let mut stream = connect(&fixture.descriptor);
    write_frame(
        &mut stream,
        &request("documents.list", "drawing.read", Value::Null, 100),
    )
    .unwrap();
    let reply: HelloReply = serde_json::from_slice(&read_frame(&mut stream).unwrap()).unwrap();
    assert!(!reply.ok, "treated as a bad hello");
}

#[test]
fn unknown_operations_and_mismatched_scopes_are_refused() {
    let fixture = start("scope");
    let mut state = AppState::new();
    let stream = connected(&fixture);
    let frame = request("files.delete", "file.write", json!({}), 1000);
    let (stream, response) = call(stream, &fixture, &mut state, &frame);
    assert_eq!(response.error.unwrap().code, ErrorCode::InvalidArguments);
    let frame = request("documents.list", "drawing.write", Value::Null, 1000);
    let (stream, response) = call(stream, &fixture, &mut state, &frame);
    assert_eq!(response.error.unwrap().code, ErrorCode::InvalidArguments);
    let (_, response) = call(stream, &fixture, &mut state, b"{not json");
    assert_eq!(response.error.unwrap().code, ErrorCode::InvalidArguments);
}

#[test]
fn a_request_the_ui_never_reaches_times_out_and_is_not_run_later() {
    let fixture = start("deadline");
    let mut stream = connected(&fixture);
    write_frame(
        &mut stream,
        &request("documents.list", "drawing.read", Value::Null, 50),
    )
    .unwrap();
    let response: Response = serde_json::from_slice(&read_frame(&mut stream).unwrap()).unwrap();
    assert_eq!(response.error.unwrap().code, ErrorCode::DeadlineExceeded);
    // When the UI wakes up the late job is dropped; the next one is answered.
    let mut state = AppState::new();
    fixture
        .host
        .poll(&mut state, &crate::commands::build_registry());
    let frame = request("documents.list", "drawing.read", Value::Null, 5000);
    let (_, response) = call(stream, &fixture, &mut state, &frame);
    assert!(response.ok);
}

#[test]
fn a_flood_fills_the_queue_to_its_limit_and_is_then_refused_until_the_ui_drains_it() {
    let fixture = start("flood");
    let mut stream = connected(&fixture);
    // Nobody polls, so each request times out but stays queued until the UI
    // looks: the queue is bounded, not the number of attempts.
    let frame = request("documents.list", "drawing.read", Value::Null, 5);
    let mut codes = Vec::new();
    for _ in 0..QUEUED_REQUESTS_MAX + 6 {
        write_frame(&mut stream, &frame).unwrap();
        let response: Response = serde_json::from_slice(&read_frame(&mut stream).unwrap()).unwrap();
        codes.push(response.error.unwrap().code);
    }
    let refused = codes
        .iter()
        .filter(|c| **c == ErrorCode::RateLimited)
        .count();
    assert_eq!(refused, 6, "{codes:?}");
    // The UI wakes: it drops the abandoned jobs and service resumes.
    let mut state = AppState::new();
    for _ in 0..8 {
        fixture
            .host
            .poll(&mut state, &crate::commands::build_registry());
    }
    let frame = request("documents.list", "drawing.read", Value::Null, 5000);
    let (_, response) = call(stream, &fixture, &mut state, &frame);
    assert!(response.ok, "{response:?}");
}

#[test]
fn an_oversized_frame_closes_the_connection() {
    let fixture = start("big");
    use std::io::Write;
    let mut stream = connected(&fixture);
    stream.write_all(&u32::MAX.to_be_bytes()).unwrap();
    stream.flush().unwrap();
    assert!(read_frame(&mut stream).is_err());
}

#[test]
fn the_descriptor_goes_when_the_host_does() {
    let dir;
    {
        let fixture = start("gone");
        dir = fixture.dir.clone();
        assert_eq!(instance::list(&dir).len(), 1);
        let Fixture { host, .. } = &fixture;
        assert!(!host.instance_id().is_empty());
    }
    assert!(instance::list(&dir).is_empty());
}

#[test]
fn a_descriptor_left_by_a_crashed_run_is_swept_at_start() {
    let dir = std::env::temp_dir().join(format!("od-host-sweep-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let stale = Descriptor {
        instance_id: "deadbeef".to_owned(),
        pid: 1,
        app_version: "0".to_owned(),
        api_versions: vec![1],
        endpoint: "od-nothing-listens-here".to_owned(),
        token: "t".to_owned(),
    };
    instance::write(&dir, &stale).unwrap();
    let core = AutomationCore::new(super::core::fresh_instance_id(), Box::new(|| {}));
    let host = SocketServer::start(&dir, &core).unwrap();
    let left = instance::list(&dir);
    assert_eq!(left.len(), 1, "{left:?}");
    assert_ne!(left[0].instance_id, "deadbeef");
    drop(host);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn writes_need_the_users_switch_and_stop_when_it_is_taken_back() {
    let fixture = start("write");
    let mut state = AppState::new();
    let mut stream = connected(&fixture);
    let (grant_id, _) = fixture.host.grants().pop().unwrap();
    let id = format!("doc_{}_1", fixture.host.instance_id());
    let batch = |revision: u64| {
        request(
            "entities.apply_batch",
            "drawing.write",
            json!({"document_id": id, "expected_revision": revision.to_string(), "operations": [
                {"op": "add", "client_ref": "a", "entity": {"geometry": {
                    "type": "line", "start": {"x": 0, "y": 0}, "end": {"x": 1, "y": 0}}}}]}),
            5000,
        )
    };

    // A fresh connection can read but not write.
    let (next, response) = call(stream, &fixture, &mut state, &batch(0));
    stream = next;
    assert_eq!(response.error.unwrap().code, ErrorCode::ScopeDenied);
    assert!(state.session().document().is_empty());

    // The user ticks the box.
    fixture.host.set_write(grant_id, true);
    let (next, response) = call(stream, &fixture, &mut state, &batch(0));
    stream = next;
    assert!(response.ok, "{response:?}");
    assert_eq!(state.session().document().len(), 1);
    assert_eq!(
        state.session().history().top_undo().unwrap().provenance,
        od_commands::Provenance::Automation(grant_id)
    );

    // And unticks it: the very next request is refused.
    fixture.host.set_write(grant_id, false);
    let (_, response) = call(stream, &fixture, &mut state, &batch(1));
    assert_eq!(response.error.unwrap().code, ErrorCode::ScopeDenied);
    assert_eq!(state.session().document().len(), 1);
}

#[test]
fn running_commands_has_its_own_grant_apart_from_writing() {
    let fixture = start("commands");
    let mut state = AppState::new();
    let stream = connected(&fixture);
    let (grant_id, _) = fixture.host.grants().pop().unwrap();
    let id = format!("doc_{}_1", fixture.host.instance_id());
    let invoke = request(
        "commands.invoke",
        "command.invoke",
        json!({"command_id": "view.grid", "document_id": id}),
        5000,
    );
    // Even with write access, running a command is a separate permission.
    fixture.host.set_write(grant_id, true);
    let (stream, response) = call(stream, &fixture, &mut state, &invoke);
    assert_eq!(response.error.unwrap().code, ErrorCode::ScopeDenied);
    assert!(state.settings.show_grid);

    fixture.host.set_commands(grant_id, true);
    let (stream, response) = call(stream, &fixture, &mut state, &invoke);
    assert!(response.ok, "{response:?}");
    assert!(!state.settings.show_grid);

    // Listing needs only reading.
    fixture.host.set_commands(grant_id, false);
    fixture.host.set_write(grant_id, false);
    let list = request("commands.list", "drawing.read", json!({}), 5000);
    let (_, response) = call(stream, &fixture, &mut state, &list);
    assert!(response.ok, "{response:?}");
}

#[test]
fn file_access_has_its_own_grant_and_still_needs_an_approved_folder() {
    let fixture = start("files");
    let mut state = AppState::new();
    let stream = connected(&fixture);
    let (grant_id, _) = fixture.host.grants().pop().unwrap();
    let path = std::env::temp_dir()
        .join("anywhere.odx")
        .to_string_lossy()
        .into_owned();
    let open = request("files.open", "file.read", json!({"path": path}), 5000);
    // Writing drawings and running commands do not give file access.
    fixture.host.set_write(grant_id, true);
    fixture.host.set_commands(grant_id, true);
    let (stream, response) = call(stream, &fixture, &mut state, &open);
    assert_eq!(response.error.unwrap().code, ErrorCode::ScopeDenied);

    // With file access it reaches the path policy, which has no folder to allow.
    fixture.host.set_files(grant_id, true);
    let (stream, response) = call(stream, &fixture, &mut state, &open);
    assert_eq!(response.error.unwrap().code, ErrorCode::PathDenied);

    // Taking it back stops the next call at the door.
    fixture.host.set_files(grant_id, false);
    let (_, response) = call(stream, &fixture, &mut state, &open);
    assert_eq!(response.error.unwrap().code, ErrorCode::ScopeDenied);
}

#[test]
fn a_connection_that_may_only_start_commands_can_watch_and_cancel_them() {
    let fixture = start("watch");
    let mut state = AppState::new();
    let stream = connected(&fixture);
    let (grant_id, _) = fixture.host.grants().pop().unwrap();
    fixture.host.set_commands(grant_id, true); // and nothing else
    let id = format!("doc_{}_1", fixture.host.instance_id());
    let begin = request(
        "commands.start_interactive",
        "command.invoke",
        json!({"command_id": "draw.circle", "document_id": id}),
        5000,
    );
    let (stream, response) = call(stream, &fixture, &mut state, &begin);
    assert!(response.ok, "{response:?}");
    let status = response.result.unwrap();
    assert_eq!(status["state"], "running", "{status}");
    assert!(state.command_running());

    let watch = request(
        "operations.get",
        "drawing.read",
        json!({"operation_id": status["operation_id"]}),
        5000,
    );
    let (stream, response) = call(stream, &fixture, &mut state, &watch);
    assert!(response.ok, "{response:?}");
    let stop = request(
        "operations.cancel",
        "drawing.read",
        json!({"operation_id": status["operation_id"]}),
        5000,
    );
    let (_, response) = call(stream, &fixture, &mut state, &stop);
    assert_eq!(response.result.unwrap()["state"], "cancelled");
    assert!(!state.command_running());
}
