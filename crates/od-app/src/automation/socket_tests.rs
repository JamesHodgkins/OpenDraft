//! The core belongs to the app, and the socket is only something in front of
//! it: the in-app assistant needs no socket, and turning the setting off
//! ends the outside programs and leaves it alone.

use std::time::Duration;

use od_api::frame::{read_frame, write_frame};
use od_api::instance;
use serde_json::Value;

use super::core::{fresh_instance_id, AutomationCore, InProcess};
use super::host::SocketServer;
use super::host_tests::{connect, connected, hello, request, start};
use crate::state::AppState;

fn ask(core: &AutomationCore, assistant: &InProcess, state: &mut AppState) -> bool {
    let registry = crate::commands::build_registry();
    std::thread::scope(|scope| {
        let handle = scope.spawn(|| assistant.call_operation("documents.list", Value::Null));
        while !handle.is_finished() {
            core.poll(state, &registry);
            std::thread::sleep(Duration::from_millis(1));
        }
        handle.join().is_ok_and(|answer| answer.ok)
    })
}

#[test]
fn a_core_with_no_socket_answers_the_in_app_connection() {
    let core = AutomationCore::new(fresh_instance_id(), Box::new(|| {}));
    let assistant = core.connect_in_process("assistant").unwrap();
    let mut state = AppState::new();
    assert!(ask(&core, &assistant, &mut state));
}

#[test]
fn stopping_the_socket_ends_outside_connections_and_keeps_the_assistants() {
    let mut fixture = start("socket-stop");
    let mut outside = connected(&fixture);
    let assistant = fixture.host.core().connect_in_process("assistant").unwrap();
    assert_eq!(fixture.host.grants().len(), 2);

    fixture.host.stop_socket();

    let grants = fixture.host.grants();
    assert_eq!(grants.len(), 1, "{grants:?}");
    assert!(grants[0].1.in_process);
    assert!(
        instance::list(&fixture.dir).is_empty(),
        "the descriptor is withdrawn"
    );
    // The outside program's connection ends at its next request.
    let frame = request("documents.list", "drawing.read", Value::Null, 1000);
    let _ = write_frame(&mut outside, &frame);
    assert!(read_frame(&mut outside).is_err());

    let mut state = AppState::new();
    assert!(ask(fixture.host.core(), &assistant, &mut state));
}

#[test]
fn the_socket_can_be_turned_on_again_in_front_of_the_same_core() {
    let mut fixture = start("socket-again");
    let assistant = fixture.host.core().connect_in_process("assistant").unwrap();
    let before = assistant.grant_id();
    fixture.host.stop_socket();

    let server = SocketServer::start(&fixture.dir, fixture.host.core()).unwrap();
    let descriptor = instance::list(&fixture.dir).pop().unwrap();
    assert_eq!(descriptor.instance_id, fixture.host.core().instance_id());
    let mut stream = connect(&descriptor);
    assert!(hello(&mut stream, &descriptor.token, &[1]).ok);
    assert_eq!(fixture.host.grants().len(), 2);
    assert_eq!(assistant.grant_id(), before, "the assistant kept its grant");
    drop(server);
}
