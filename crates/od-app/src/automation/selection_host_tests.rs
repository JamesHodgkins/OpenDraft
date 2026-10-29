//! `selection.request` over a real connection: the user's switch and the
//! caller's connection both end a request the user is looking at.

use std::time::{Duration, Instant};

use od_api::ErrorCode;
use serde_json::{json, Value};

use super::host_tests::{call, connected, request, start};
use crate::state::AppState;

fn ask(id: &str) -> Vec<u8> {
    request(
        "selection.request",
        "ui.selection",
        json!({"document_id": id, "prompt": "Select the walls"}),
        5000,
    )
}

#[test]
fn a_read_only_connection_cannot_put_a_prompt_in_front_of_the_user() {
    let fixture = start("select-denied");
    let mut state = AppState::new();
    let stream = connected(&fixture);
    let id = format!("doc_{}_1", fixture.host.instance_id());
    let (_, response) = call(stream, &fixture, &mut state, &ask(&id));
    assert_eq!(response.error.unwrap().code, ErrorCode::ScopeDenied);
    assert!(!state.command_running());
}

#[test]
fn taking_the_switch_back_removes_the_prompt_at_once() {
    let fixture = start("select-revoke");
    let mut state = AppState::new();
    let stream = connected(&fixture);
    let (grant_id, _) = fixture.host.grants().pop().unwrap();
    let id = format!("doc_{}_1", fixture.host.instance_id());
    fixture.host.set_write(grant_id, true);
    let (stream, response) = call(stream, &fixture, &mut state, &ask(&id));
    let started: Value = response.result.unwrap();
    assert_eq!(started["state"], "running", "{started}");
    assert!(state.command_running());

    fixture.host.set_write(grant_id, false);
    fixture
        .host
        .poll(&mut state, &crate::commands::build_registry());
    assert!(
        !state.command_running(),
        "the user is not left with a dead prompt"
    );
    assert!(state
        .cmdline
        .log
        .iter()
        .any(|l| l.contains("no longer allowed")));
    drop(stream);
}

#[test]
fn a_caller_that_disconnects_takes_its_prompt_with_it() {
    let fixture = start("select-gone");
    let mut state = AppState::new();
    let stream = connected(&fixture);
    let (grant_id, _) = fixture.host.grants().pop().unwrap();
    let id = format!("doc_{}_1", fixture.host.instance_id());
    fixture.host.set_write(grant_id, true);
    let (stream, response) = call(stream, &fixture, &mut state, &ask(&id));
    assert!(response.ok, "{response:?}");
    assert!(state.command_running());

    drop(stream);
    let until = Instant::now() + Duration::from_secs(5);
    while state.command_running() && Instant::now() < until {
        fixture
            .host
            .poll(&mut state, &crate::commands::build_registry());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!state.command_running(), "the prompt outlived its caller");
}
