//! `selection.request`: the user picks objects in the app for a caller.

use od_core::{Entity, EntityId, Point};
use serde_json::{json, Value};

use super::{dispatch_test, document_id, Caller};
use crate::state::AppState;

pub(super) const INSTANCE: &str = "inst1";
pub(super) const ME: Caller = Caller {
    client: "test client",
    grant_id: 7,
    history_any: false,
};
pub(super) const OTHER: Caller = Caller {
    client: "other",
    grant_id: 8,
    history_any: false,
};

pub(super) fn call(
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

pub(super) fn request(state: &mut AppState, extra: Value) -> Result<Value, od_api::ApiError> {
    let mut args = json!({"document_id": "active", "prompt": "Select the walls"});
    for (key, value) in extra.as_object().into_iter().flatten() {
        args[key] = value.clone();
    }
    call(state, &ME, "selection.request", args)
}

pub(super) fn status(state: &mut AppState, id: &Value) -> Value {
    call(state, &ME, "operations.get", json!({"operation_id": id})).unwrap()
}

pub(super) fn line(x: f64) -> Entity {
    Entity::Line {
        start: Point::new(x, 0.0),
        end: Point::new(x + 1.0, 0.0),
    }
}

pub(super) fn circle(x: f64) -> Entity {
    Entity::Circle {
        center: Point::new(x, 5.0),
        radius: 1.0,
    }
}

/// Two lines and a circle: ids in drawing order.
pub(super) fn drawing() -> (AppState, Vec<EntityId>) {
    let mut state = AppState::new();
    state.add_entity(line(0.0));
    state.add_entity(line(3.0));
    state.add_entity(circle(8.0));
    let ids = state
        .session()
        .document()
        .objects()
        .iter()
        .map(|o| o.id)
        .collect();
    (state, ids)
}

/// The user picks on the canvas and presses Enter.
pub(super) fn user_picks(state: &mut AppState, ids: &[EntityId]) {
    state.select_ids(ids.iter().copied(), false);
    state.answer_enter();
    state.settle_command();
}

pub(super) fn wire(ids: &[EntityId]) -> Vec<String> {
    ids.iter().map(|id| id.0.to_string()).collect()
}

pub(super) fn said(state: &AppState) -> String {
    state.cmdline.log.join("\n")
}

#[test]
fn the_user_picks_and_the_caller_gets_the_ids_and_the_revision() {
    let (mut state, ids) = drawing();
    let revision = state.session().revision();
    let started = request(&mut state, json!({})).unwrap();
    assert_eq!(started["kind"], "selection");
    assert_eq!(started["state"], "running", "{started}");
    assert_eq!(started["cancellable"], true);
    assert_eq!(
        started["document_id"],
        document_id(INSTANCE, state.docs[0].uid)
    );
    assert_eq!(started["waiting_for"], "Select the walls");
    assert!(started.get("entity_ids").is_none(), "no answer yet");
    assert_eq!(
        state.runner_prompt().unwrap().command.to_uppercase(),
        "ASSISTANT",
        "the prompt is labelled as the assistant's, not as the app's"
    );

    user_picks(&mut state, &ids[..2]);
    let done = status(&mut state, &started["operation_id"]);
    assert_eq!(done["state"], "succeeded", "{done}");
    assert_eq!(done["entity_ids"], json!(wire(&ids[..2])));
    assert_eq!(done["revision"], revision.to_string());
    assert!(done.get("waiting_for").is_none());
    assert_eq!(done["cancellable"], false);

    // Nothing was drawn or changed, and what the user picked stays selected.
    assert_eq!(state.session().revision(), revision);
    assert!(!state.command_running());
    assert_eq!(state.selection().len(), 2);
}

#[test]
fn the_selection_starts_empty_unless_the_caller_keeps_the_users() {
    let (mut state, ids) = drawing();
    state.select_ids([ids[2]], false);
    let started = request(&mut state, json!({})).unwrap();
    assert!(state.selection().is_empty(), "only what they pick for you");
    state.cancel_command();
    assert_eq!(
        status(&mut state, &started["operation_id"])["state"],
        "cancelled"
    );

    state.select_ids([ids[2]], false);
    let started = request(&mut state, json!({"keep_current_selection": true})).unwrap();
    assert_eq!(state.selection().len(), 1);
    state.answer_enter(); // accepts what is already selected
    state.settle_command();
    let done = status(&mut state, &started["operation_id"]);
    assert_eq!(done["entity_ids"], json!(wire(&ids[2..])));
}

#[test]
fn escape_refuses_and_the_caller_can_withdraw_its_own_request() {
    let (mut state, ids) = drawing();
    let started = request(&mut state, json!({})).unwrap();
    state.cancel_command(); // Esc
    let refused = status(&mut state, &started["operation_id"]);
    assert_eq!(refused["state"], "cancelled", "{refused}");
    assert!(refused.get("entity_ids").is_none());

    let started = request(&mut state, json!({})).unwrap();
    let id = started["operation_id"].clone();
    let cancelled = call(
        &mut state,
        &ME,
        "operations.cancel",
        json!({"operation_id": id}),
    )
    .unwrap();
    assert_eq!(cancelled["state"], "cancelled");
    assert!(!state.command_running(), "the prompt is gone");
    state.select_ids([ids[0]], false);
    assert_eq!(status(&mut state, &id)["state"], "cancelled");
}

#[test]
fn another_command_taking_over_ends_the_request_without_an_answer() {
    let (mut state, _) = drawing();
    let started = request(&mut state, json!({})).unwrap();
    state.start_command("CIRCLE", crate::circle::draw_circle);
    state.settle_command();
    let seen = status(&mut state, &started["operation_id"]);
    assert_eq!(seen["state"], "cancelled", "{seen}");
    state.cancel_command();
}

#[test]
fn only_the_kinds_asked_for_come_back_and_the_rest_leave_the_selection() {
    let (mut state, ids) = drawing();
    let started = request(&mut state, json!({"kinds": ["Circle"]})).unwrap();
    user_picks(&mut state, &ids);
    let done = status(&mut state, &started["operation_id"]);
    assert_eq!(done["state"], "succeeded", "{done}");
    assert_eq!(done["entity_ids"], json!(wire(&ids[2..])));
    assert_eq!(
        state.selection().iter().copied().collect::<Vec<_>>(),
        &ids[2..],
        "the canvas shows what was accepted"
    );
    let log = said(&state);
    assert!(
        log.contains("2 objects did not qualify (wanted: circle)"),
        "{log}"
    );
}

#[test]
fn a_pick_with_nothing_that_qualifies_asks_again() {
    let (mut state, ids) = drawing();
    let started = request(&mut state, json!({"kinds": ["circle"]})).unwrap();
    user_picks(&mut state, &ids[..2]);
    let waiting = status(&mut state, &started["operation_id"]);
    assert_eq!(waiting["state"], "running", "still asking: {waiting}");
    assert!(state.selection().is_empty());
    assert!(said(&state).contains("0 objects usable; at least 1 needed"));

    user_picks(&mut state, &ids[2..]);
    let done = status(&mut state, &started["operation_id"]);
    assert_eq!(done["entity_ids"], json!(wire(&ids[2..])));
}

#[test]
fn how_many_may_be_chosen_is_enforced_and_said() {
    let (mut state, ids) = drawing();
    let started = request(&mut state, json!({"min_count": 2, "max_count": 2})).unwrap();
    assert_eq!(started["waiting_for"], "Select the walls (exactly 2)");
    user_picks(&mut state, &ids[..1]);
    assert_eq!(
        status(&mut state, &started["operation_id"])["state"],
        "running"
    );
    assert!(said(&state).contains("1 object usable; at least 2 needed"));
    user_picks(&mut state, &ids[1..]); // now all three are selected
    assert_eq!(
        status(&mut state, &started["operation_id"])["state"],
        "running"
    );
    assert!(said(&state).contains("3 objects selected; at most 2 can be used"));
    state.select_ids([ids[2]], true);
    state.answer_enter();
    state.settle_command();
    let done = status(&mut state, &started["operation_id"]);
    assert_eq!(done["entity_ids"], json!(wire(&ids[..2])), "{done}");
}

#[test]
fn zero_as_the_least_lets_the_user_finish_with_nothing() {
    let (mut state, _) = drawing();
    let started = request(&mut state, json!({"min_count": 0})).unwrap();
    state.answer_enter();
    state.settle_command();
    let done = status(&mut state, &started["operation_id"]);
    assert_eq!(done["state"], "succeeded", "{done}");
    assert_eq!(done["entity_ids"], json!([]), "an answer, not no answer");
}

#[test]
fn enter_at_the_cursor_finishes_a_request_like_every_other_enter() {
    let (mut state, ids) = drawing();
    let started = request(&mut state, json!({"kinds": ["line"]})).unwrap();
    state.select_ids(ids[..2].iter().copied(), false);
    // With dynamic input on, Enter over the canvas arrives as an empty line.
    assert_eq!(state.answer_dynamic(""), Ok(()));
    state.settle_command();
    let done = status(&mut state, &started["operation_id"]);
    assert_eq!(done["state"], "succeeded", "{done}");
    assert_eq!(done["entity_ids"], json!(wire(&ids[..2])));
}
