//! `selection.request`: what is refused, who owns a request, and how one
//! ends when its caller or its time is gone.

use std::time::{Duration, Instant};

use od_api::ErrorCode;
use od_commands::{ModifyAttributes, ModifyLayers};
use od_core::{Attributes, EntityId};
use serde_json::{json, Value};

use super::document_id;
use super::selection_tests::{
    call, drawing, request, said, status, user_picks, INSTANCE, ME, OTHER,
};
use crate::state::AppState;

#[test]
fn a_request_that_is_not_asked_properly_is_refused_before_the_user_sees_it() {
    let (mut state, _) = drawing();
    let code = |state: &mut AppState, extra: Value| request(state, extra).unwrap_err().code;
    assert_eq!(
        code(&mut state, json!({"prompt": " \n\t "})),
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        code(&mut state, json!({"prompt": "x".repeat(201)})),
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        code(&mut state, json!({"kinds": ["sphere"]})),
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        code(&mut state, json!({"layer_id": "999"})),
        ErrorCode::InvalidReference
    );
    assert_eq!(
        code(&mut state, json!({"max_count": 0})),
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        code(&mut state, json!({"min_count": 3, "max_count": 2})),
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        code(&mut state, json!({"surprise": 1})),
        ErrorCode::InvalidArguments
    );
    assert!(!state.command_running(), "nothing reached the user");
}

#[test]
fn the_prompt_is_flattened_so_it_cannot_fake_lines_of_the_command_line() {
    let (mut state, _) = drawing();
    let started = request(
        &mut state,
        json!({"prompt": "Pick\nwalls\u{7}   now\r\nCommand: ERASE"}),
    )
    .unwrap();
    assert_eq!(started["waiting_for"], "Pick walls now Command: ERASE");
    state.cancel_command();
}

#[test]
fn it_never_cuts_in_on_the_user_nor_acts_on_a_drawing_they_are_not_looking_at() {
    let (mut state, _) = drawing();
    state.start_command("LINE", crate::draw::draw_line);
    state.settle_command();
    assert_eq!(
        request(&mut state, json!({})).unwrap_err().code,
        ErrorCode::CommandBusy
    );
    assert!(
        state.is_running("LINE"),
        "the user's command is still theirs"
    );
    state.cancel_command();

    state.show_settings = true;
    assert_eq!(
        request(&mut state, json!({})).unwrap_err().code,
        ErrorCode::OperationBusy
    );
    state.show_settings = false;

    let first = document_id(INSTANCE, state.docs[0].uid);
    state.new_document();
    let elsewhere = call(
        &mut state,
        &ME,
        "selection.request",
        json!({"document_id": first, "prompt": "Select"}),
    );
    assert_eq!(elsewhere.unwrap_err().code, ErrorCode::InvalidReference);
    assert_eq!(state.active, 1, "their tab was not switched");

    let stale = request(&mut state, json!({"expected_revision": "9"}));
    assert_eq!(stale.unwrap_err().code, ErrorCode::StaleRevision);
    assert!(!state.command_running());
}

#[test]
fn leaving_the_drawing_ends_the_request() {
    let (mut state, _) = drawing();
    state.new_document();
    state.active = 0;
    let started = request(&mut state, json!({})).unwrap();
    state.active = 1; // the user clicks another tab
    state.pump_command();
    let seen = status(&mut state, &started["operation_id"]);
    assert_eq!(seen["state"], "cancelled", "{seen}");
    assert!(!state.command_running());
}

#[test]
fn two_clients_cannot_share_the_prompt_and_cannot_touch_each_others() {
    let (mut state, ids) = drawing();
    let mine = request(&mut state, json!({})).unwrap();
    let args = json!({"document_id": "active", "prompt": "Select too"});
    let theirs = call(&mut state, &OTHER, "selection.request", args.clone());
    assert_eq!(theirs.unwrap_err().code, ErrorCode::CommandBusy);
    let id = json!({"operation_id": mine["operation_id"]});
    assert!(call(&mut state, &OTHER, "operations.get", id.clone()).is_err());
    assert!(call(&mut state, &OTHER, "operations.cancel", id).is_err());
    assert!(state.command_running(), "not cancelled by a stranger");

    user_picks(&mut state, &ids[..1]);
    assert_eq!(
        status(&mut state, &mine["operation_id"])["state"],
        "succeeded"
    );
    // The prompt is free again for the other.
    let next = call(&mut state, &OTHER, "selection.request", args).unwrap();
    assert_eq!(next["state"], "running");
    state.cancel_command();
}

#[test]
fn a_request_nobody_answers_in_time_fails_and_takes_the_prompt_away() {
    let (mut state, _) = drawing();
    let started = request(&mut state, json!({"timeout_seconds": 60})).unwrap();
    let id = started["operation_id"].as_str().unwrap()[3..]
        .parse()
        .unwrap();
    assert_eq!(
        status(&mut state, &started["operation_id"])["state"],
        "running"
    );
    state.file_ops.get_mut(id).unwrap().deadline = Some(Instant::now() - Duration::from_secs(1));
    let late = status(&mut state, &started["operation_id"]);
    assert_eq!(late["state"], "failed", "{late}");
    assert_eq!(late["error"]["code"], "deadline_exceeded");
    assert!(!state.command_running());
    assert!(said(&state).contains("timed out"));
}

#[test]
fn a_request_whose_owner_left_or_lost_the_right_is_withdrawn() {
    let (mut state, _) = drawing();
    let started = request(&mut state, json!({})).unwrap();
    // Someone else's grant going away changes nothing.
    super::selection_request::withdraw(&mut state, |grant| grant == OTHER.grant_id, "no");
    assert!(state.command_running());
    super::selection_request::withdraw(&mut state, |grant| grant == ME.grant_id, "It left.");
    assert!(!state.command_running(), "the prompt left the screen");
    assert_eq!(
        status(&mut state, &started["operation_id"])["state"],
        "cancelled"
    );
    assert!(said(&state).contains("It left."), "the user is told");
}

#[test]
fn giving_up_on_file_work_does_not_abandon_a_selection_in_progress() {
    let (mut state, ids) = drawing();
    let started = request(&mut state, json!({})).unwrap();
    state.file_ops.cancel_running();
    assert_eq!(
        status(&mut state, &started["operation_id"])["state"],
        "running"
    );
    user_picks(&mut state, &ids[..1]);
    assert_eq!(
        status(&mut state, &started["operation_id"])["state"],
        "succeeded"
    );
}

#[test]
fn a_repeated_request_with_the_same_key_finds_its_own_prompt() {
    let (mut state, _) = drawing();
    let key = json!({"idempotency_key": "ask-1"});
    let first = request(&mut state, key.clone()).unwrap();
    let again = request(&mut state, key).unwrap();
    assert_eq!(again["operation_id"], first["operation_id"]);
    assert_eq!(again["replayed"], true);
    assert_eq!(
        state.file_ops.unfinished(ME.grant_id),
        1,
        "one prompt, not two"
    );
    state.cancel_command();
}

#[test]
fn the_selection_changing_is_an_event_for_anyone_watching() {
    let (mut state, ids) = drawing();
    let poll = |state: &mut AppState, since: Option<&str>| {
        let args = since.map_or(json!({}), |s| json!({"since": s}));
        call(state, &ME, "events.poll", args).unwrap()
    };
    let cursor = poll(&mut state, None)["next_seq"]
        .as_str()
        .unwrap()
        .to_owned();
    state.select_ids(ids[..2].iter().copied(), false);
    let changed = poll(&mut state, Some(&cursor));
    let events = changed["events"].as_array().unwrap();
    assert_eq!(events.len(), 1, "{changed}");
    assert_eq!(events[0]["kind"], "selection_changed");
    assert_eq!(events[0]["detail"], "2 selected");
    assert_eq!(
        events[0]["document_id"],
        document_id(INSTANCE, state.docs[0].uid)
    );

    let cursor = changed["next_seq"].as_str().unwrap().to_owned();
    assert!(poll(&mut state, Some(&cursor))["events"]
        .as_array()
        .unwrap()
        .is_empty());
    state.select_ids(ids[..2].iter().copied(), true);
    let cleared = poll(&mut state, Some(&cursor));
    assert_eq!(cleared["events"][0]["detail"], "nothing selected");
}

#[test]
fn objects_that_stopped_being_selectable_are_never_handed_back() {
    use super::selection_request::{Rule, Verdict};
    let (mut state, ids) = drawing();
    let other = state.new_layer();
    state.perform(ModifyAttributes::new(
        "To other",
        vec![(ids[0], Attributes::on_layer(other))],
    ));
    let mut layer = state.session().document().layer(other).cloned().unwrap();
    layer.locked = true;
    state.perform(ModifyLayers::new("Lock", vec![layer]));
    let rule = Rule::any(1, 10);
    let gone = EntityId(9999);
    match rule.judge(state.session().document(), &[ids[0], ids[1], gone]) {
        Verdict::Accept {
            ids: kept,
            left_out,
            ..
        } => {
            assert_eq!(kept, [ids[1]]);
            assert_eq!(left_out, [ids[0], gone], "locked, and no longer there");
        }
        Verdict::Again { .. } => panic!("one usable object was enough"),
    }
}
