//! Interactive commands started on request, and the event feed.

use od_api::ErrorCode;
use od_core::{Entity, Point};
use serde_json::{json, Value};

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

fn start(state: &mut AppState, command: &str) -> Result<Value, od_api::ApiError> {
    call(
        state,
        &ME,
        "commands.start_interactive",
        json!({"command_id": command, "document_id": "active"}),
    )
}

fn operation(state: &mut AppState, id: &Value) -> Value {
    call(state, &ME, "operations.get", json!({"operation_id": id})).unwrap()
}

fn user_types(state: &mut AppState, answers: &[&str]) {
    for answer in answers {
        state.cmd_submit(answer);
        state.settle_command();
    }
}

fn line(x: f64) -> Entity {
    Entity::Line {
        start: Point::new(x, 0.0),
        end: Point::new(x + 1.0, 0.0),
    }
}

#[test]
fn a_started_command_waits_for_the_user_and_ends_when_they_finish_it() {
    let mut state = AppState::new();
    let status = start(&mut state, "draw.circle").unwrap();
    assert_eq!(status["state"], "running", "{status}");
    assert_eq!(status["kind"], "command");
    assert_eq!(status["command_id"], "draw.circle");
    assert!(status.get("file_name").is_none(), "no file in it");
    let first = status["waiting_for"].as_str().unwrap().to_lowercase();
    assert!(first.contains("center"), "{first}");
    assert!(state.command_running());
    assert!(
        state.session().document().is_empty(),
        "nothing is drawn by the caller"
    );

    // The user answers in the app: the caller only watches.
    user_types(&mut state, &["0,0"]);
    let asking = operation(&mut state, &status["operation_id"]);
    assert_eq!(asking["state"], "running");
    assert_ne!(
        asking["waiting_for"].as_str().unwrap().to_lowercase(),
        first,
        "the next question"
    );
    user_types(&mut state, &["5"]);
    let done = operation(&mut state, &status["operation_id"]);
    assert_eq!(done["state"], "succeeded", "{done}");
    assert!(done.get("waiting_for").is_none());
    assert_eq!(done["cancellable"], false);
    assert_eq!(state.session().document().len(), 1);
    assert!(matches!(
        state.session().document().objects()[0].entity,
        Entity::Circle { .. }
    ));
}

#[test]
fn the_caller_can_cancel_its_command_and_so_can_the_user() {
    let mut state = AppState::new();
    let status = start(&mut state, "draw.line").unwrap();
    let cancelled = call(
        &mut state,
        &ME,
        "operations.cancel",
        json!({"operation_id": status["operation_id"]}),
    )
    .unwrap();
    assert_eq!(cancelled["state"], "cancelled", "{cancelled}");
    assert!(!state.command_running());
    assert!(state.session().document().is_empty());

    let status = start(&mut state, "draw.line").unwrap();
    state.cancel_command(); // Esc in the app
    let seen = operation(&mut state, &status["operation_id"]);
    assert_eq!(seen["state"], "cancelled");

    // Cancelling one that is over just reports it.
    let again = call(
        &mut state,
        &ME,
        "operations.cancel",
        json!({"operation_id": status["operation_id"]}),
    )
    .unwrap();
    assert_eq!(again["state"], "cancelled");
}

#[test]
fn another_command_replacing_it_is_a_cancel_not_a_success() {
    let mut state = AppState::new();
    let status = start(&mut state, "draw.line").unwrap();
    // The user starts something else from the ribbon.
    state.start_command("CIRCLE", crate::circle::draw_circle);
    state.settle_command();
    let seen = operation(&mut state, &status["operation_id"]);
    assert_eq!(seen["state"], "cancelled", "{seen}");
}

#[test]
fn it_never_cuts_in_on_what_the_user_is_doing() {
    let mut state = AppState::new();
    state.start_command("LINE", crate::draw::draw_line);
    state.settle_command();
    let error = start(&mut state, "draw.circle").unwrap_err();
    assert_eq!(error.code, ErrorCode::CommandBusy);
    assert!(
        state.is_running("LINE"),
        "the user's command is still theirs"
    );

    let mut state = AppState::new();
    state.show_settings = true;
    assert_eq!(
        start(&mut state, "draw.circle").unwrap_err().code,
        ErrorCode::OperationBusy
    );
    assert!(!state.command_running());
}

#[test]
fn only_commands_marked_for_the_user_to_finish_can_be_started() {
    let mut state = AppState::new();
    let code = |state: &mut AppState, id: &str| start(state, id).unwrap_err();
    let immediate = code(&mut state, "view.grid");
    assert_eq!(immediate.code, ErrorCode::InvalidArguments);
    assert!(immediate.message.contains("commands_invoke"));
    for denied in [
        "file.exit",
        "file.save",
        "edit.undo",
        "modify.erase",
        "manage.settings",
        "block.close",
    ] {
        assert_eq!(
            code(&mut state, denied).code,
            ErrorCode::CommandNotAutomatable,
            "{denied}"
        );
    }
    assert_eq!(
        code(&mut state, "no.such").code,
        ErrorCode::InvalidArguments
    );
    // Needs objects to work on, and there are none.
    assert_eq!(
        code(&mut state, "modify.move").code,
        ErrorCode::InvalidArguments
    );
    assert!(!state.command_running());
    assert!(!state.exit_requested);
}

#[test]
fn it_runs_only_in_the_active_tab_and_for_the_revision_read() {
    let mut state = AppState::new();
    let first = document_id(INSTANCE, state.docs[0].uid);
    state.new_document();
    let wrong_tab = call(
        &mut state,
        &ME,
        "commands.start_interactive",
        json!({"command_id": "draw.line", "document_id": first}),
    );
    assert_eq!(wrong_tab.unwrap_err().code, ErrorCode::InvalidReference);
    assert_eq!(state.active, 1, "the user's tab was not switched");
    let stale = call(
        &mut state,
        &ME,
        "commands.start_interactive",
        json!({"command_id": "draw.line", "document_id": "active", "expected_revision": "9"}),
    );
    assert_eq!(stale.unwrap_err().code, ErrorCode::StaleRevision);
    assert!(!state.command_running());
}

#[test]
fn a_selection_made_first_is_used_and_skips_the_selection_step() {
    let mut state = AppState::new();
    state.add_entity(line(0.0));
    let id = state.session().document().objects()[0].id.0.to_string();
    call(
        &mut state,
        &ME,
        "selection.set",
        json!({"document_id": "active", "mode": "replace", "entity_ids": [id]}),
    )
    .unwrap();
    let status = start(&mut state, "modify.move").unwrap();
    let asking = status["waiting_for"].as_str().unwrap().to_lowercase();
    assert!(
        asking.contains("base point"),
        "straight to the base point: {asking}"
    );
    assert!(!asking.contains("select objects"));
    state.cancel_command();
}

#[test]
fn operations_are_private_to_the_connection_and_a_drawing_in_bedit_is_refused() {
    let mut state = AppState::new();
    let status = start(&mut state, "draw.line").unwrap();
    let theirs = call(
        &mut state,
        &OTHER,
        "operations.get",
        json!({"operation_id": status["operation_id"]}),
    );
    assert!(theirs.is_err());
    assert!(call(
        &mut state,
        &OTHER,
        "operations.cancel",
        json!({"operation_id": status["operation_id"]})
    )
    .is_err());
    assert!(state.command_running(), "not cancelled by a stranger");
    state.cancel_command();

    state.add_entity(Entity::Rectangle {
        min: Point::new(10.0, 10.0),
        max: Point::new(12.0, 11.0),
    });
    state.select_all();
    state.start_command("BLOCK", crate::block_cmd::define_block);
    state.settle_command();
    user_types(&mut state, &["Box", "10,10"]);
    state.select_all();
    state.bedit();
    assert!(state.in_block_editor());
    let error = start(&mut state, "draw.line").unwrap_err();
    assert_eq!(error.code, ErrorCode::BlockEditorOpen);
}

fn poll(state: &mut AppState, since: Option<&str>) -> Value {
    let args = match since {
        Some(s) => json!({"since": s}),
        None => json!({}),
    };
    call(state, &ME, "events.poll", args).unwrap()
}

fn kinds(result: &Value) -> Vec<String> {
    result["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn events_report_edits_by_anyone_once_and_in_order() {
    let mut state = AppState::new();
    let first = poll(&mut state, None);
    assert!(
        kinds(&first).is_empty(),
        "the first look only records where things stand: {first}"
    );
    let cursor = first["next_seq"].as_str().unwrap().to_owned();

    state.add_entity(line(0.0)); // the user draws
    let id = document_id(INSTANCE, state.docs[0].uid);
    let args = json!({"document_id": id, "expected_revision": "1", "label": "Assistant edit", "operations": [
        {"op": "add", "client_ref": "a", "entity": {"geometry": {"type": "line", "start": {"x": 5, "y": 0}, "end": {"x": 6, "y": 0}}}}]});
    call(&mut state, &ME, "entities.apply_batch", args).unwrap();
    let second = poll(&mut state, Some(&cursor));
    // Both edits happened between two looks: one change event for the latest.
    assert_eq!(kinds(&second), ["document_changed"], "{second}");
    let event = &second["events"][0];
    assert_eq!(event["document_id"], id);
    assert_eq!(event["revision"], "2");
    assert_eq!(event["detail"], "Assistant edit");

    // Nothing new: nothing returned, and the cursor stands still.
    let cursor = second["next_seq"].as_str().unwrap().to_owned();
    let quiet = poll(&mut state, Some(&cursor));
    assert!(kinds(&quiet).is_empty());
    assert_eq!(quiet["next_seq"], cursor.as_str());
    state.docs[0].session.undo();
    let undone = poll(&mut state, Some(&cursor));
    assert_eq!(kinds(&undone), ["document_changed"]);
}

#[test]
fn events_report_drawings_coming_and_going_and_commands_starting_and_ending() {
    let mut state = AppState::new();
    let cursor = poll(&mut state, None)["next_seq"]
        .as_str()
        .unwrap()
        .to_owned();
    state.new_document();
    let opened = poll(&mut state, Some(&cursor));
    assert_eq!(kinds(&opened), ["document_opened"]);
    let cursor = opened["next_seq"].as_str().unwrap().to_owned();

    state.start_command("LINE", crate::draw::draw_line);
    state.settle_command();
    state.observe_events();
    state.cancel_command();
    let ran = poll(&mut state, Some(&cursor));
    assert_eq!(kinds(&ran), ["command_started", "command_ended"], "{ran}");
    assert_eq!(ran["events"][0]["detail"], "LINE");
    let cursor = ran["next_seq"].as_str().unwrap().to_owned();

    state.request_close(1);
    state.answer_confirm(crate::files::Choice::Discard);
    let closed = poll(&mut state, Some(&cursor));
    assert!(
        kinds(&closed).contains(&"document_closed".to_owned()),
        "{closed}"
    );
}

#[test]
fn a_caller_that_falls_far_behind_is_told_to_start_over() {
    let mut state = AppState::new();
    poll(&mut state, None);
    // Far more than the log keeps, one look per edit.
    for i in 0..300 {
        state.add_entity(line(f64::from(i)));
        state.observe_events();
    }
    let behind = poll(&mut state, Some("1"));
    assert_eq!(behind["reset"], true, "{behind}");
    let all = poll(&mut state, None);
    assert_eq!(
        all["reset"], false,
        "asking for everything kept is not behind"
    );
    assert!(all["events"].as_array().unwrap().len() <= 256);
    // Paging: ask for a few at a time and reach the end.
    let mut cursor = all["events"][0]["seq"]
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap()
        - 1;
    let mut total = 0;
    loop {
        let page = call(
            &mut state,
            &ME,
            "events.poll",
            json!({"since": cursor.to_string(), "limit": 100}),
        )
        .unwrap();
        total += page["events"].as_array().unwrap().len();
        cursor = page["next_seq"].as_str().unwrap().parse().unwrap();
        if page["more"] != true {
            break;
        }
    }
    assert!(
        total > 200 && total <= 256,
        "the whole log, in pages: {total}"
    );
}

#[test]
fn event_text_is_bounded_and_free_of_control_characters() {
    let mut state = AppState::new();
    poll(&mut state, None);
    state.docs[0].title = format!("Plan\u{7}{}", "x".repeat(500));
    state.new_document();
    state.docs[1].title = format!("Evil\n{}", "y".repeat(500));
    state.observe_events();
    let result = poll(&mut state, None);
    for event in result["events"].as_array().unwrap() {
        if let Some(detail) = event["detail"].as_str() {
            assert!(
                detail.chars().count() <= 80 && !detail.chars().any(char::is_control),
                "{detail:?}"
            );
        }
    }
}

#[test]
fn the_new_operations_need_the_right_scopes() {
    use od_api::{ops, Scope};
    assert_eq!(
        ops::required_scope(ops::COMMANDS_START_INTERACTIVE),
        Some(Scope::CommandInvoke)
    );
    assert_eq!(
        ops::required_scope(ops::EVENTS_POLL),
        Some(Scope::DrawingRead)
    );
}
