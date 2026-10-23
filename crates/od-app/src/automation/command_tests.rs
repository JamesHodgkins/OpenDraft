use std::collections::HashSet;

use od_api::ErrorCode;
use od_commands::{AutomationPolicy, CommandEffect};
use od_core::{Entity, Point};
use serde_json::{json, Value};

use super::policy::TABLE;
use super::{dispatch_test, document_id, Caller};
use crate::commands::build_registry;
use crate::state::AppState;

const INSTANCE: &str = "inst1";
const ME: Caller = Caller {
    client: "test",
    grant_id: 7,
    history_any: false,
};

fn call(state: &mut AppState, op: &str, mut args: Value) -> Result<Value, od_api::ApiError> {
    if args.get("document_id") == Some(&json!("active")) {
        args["document_id"] = json!(document_id(INSTANCE, state.docs[state.active].uid));
    }
    dispatch_test(state, INSTANCE, &ME, op, args)
}

#[test]
fn every_registered_command_has_one_explicit_row_and_no_row_is_stale() {
    let registry = build_registry();
    let registered: Vec<&str> = registry.iter().map(|c| c.id).collect();
    let rows: Vec<&str> = TABLE.iter().map(|(id, ..)| *id).collect();
    let unique: HashSet<_> = rows.iter().collect();
    assert_eq!(unique.len(), rows.len(), "a command has two rows");
    for id in &registered {
        assert!(rows.contains(id), "{id} has no automation row: decide it");
    }
    for id in &rows {
        assert!(
            registered.contains(id),
            "{id} is in the table but not registered"
        );
    }
    // What the registry carries is what the table says.
    for command in registry.iter() {
        let row = TABLE.iter().find(|(id, ..)| *id == command.id).unwrap();
        assert_eq!(
            (command.automation, command.effect),
            (row.1, row.2),
            "{}",
            command.id
        );
    }
}

#[test]
fn the_table_keeps_the_safety_rules() {
    for &(id, policy, effect) in TABLE {
        match effect {
            CommandEffect::FileRead
            | CommandEffect::FileWrite
            | CommandEffect::ApplicationLifecycle => {
                assert_eq!(
                    policy,
                    AutomationPolicy::Denied,
                    "{id}: files, dialogs and lifecycle stay denied"
                );
            }
            CommandEffect::DrawingMutation => {
                assert_ne!(
                    policy,
                    AutomationPolicy::ImmediateSafe,
                    "{id}: edits go through the batch tool"
                );
            }
            CommandEffect::ReadOnly | CommandEffect::UiState => {}
        }
        if policy == AutomationPolicy::ImmediateSafe {
            assert!(
                id.starts_with("view.") || id == "edit.select_all",
                "{id}: only view, mode and selection commands are immediate"
            );
        }
    }
    // The ones that must never be reachable.
    for id in [
        "file.exit",
        "file.close",
        "file.new",
        "file.save",
        "file.save_as",
        "file.open",
        "manage.settings",
        "edit.undo",
        "block.close",
    ] {
        let row = TABLE.iter().find(|(i, ..)| *i == id).unwrap();
        assert_eq!(row.1, AutomationPolicy::Denied, "{id}");
    }
}

#[test]
fn an_unclassified_command_is_denied() {
    let command = od_commands::Command::<AppState>::new("x.new_thing", "New", |_| {});
    assert_eq!(
        super::classified(command).automation,
        AutomationPolicy::Denied
    );
}

#[test]
fn the_list_hides_what_is_denied_and_marks_what_can_run() {
    let mut state = AppState::new();
    let result = call(&mut state, "commands.list", json!({})).unwrap();
    let commands = result["commands"].as_array().unwrap();
    assert!(commands.iter().all(|c| c["policy"] != "denied"));
    let find = |id: &str| {
        commands
            .iter()
            .find(|c| c["id"] == id)
            .unwrap_or_else(|| panic!("{id}"))
    };
    assert_eq!(find("view.grid")["invocable"], true);
    assert_eq!(find("view.grid")["checked"], true, "the grid starts on");
    assert_eq!(find("draw.line")["policy"], "interactive_in_app");
    assert_eq!(
        find("draw.line")["invocable"],
        false,
        "never pretended to be immediate"
    );
    assert_eq!(
        find("edit.select_all")["enabled"],
        false,
        "nothing to select yet"
    );
    assert_eq!(
        result["active_document_id"],
        document_id(INSTANCE, state.docs[0].uid)
    );
    let all = call(&mut state, "commands.list", json!({"include_denied": true})).unwrap();
    assert!(all["commands"].as_array().unwrap().len() > commands.len());
}

#[test]
fn a_safe_command_runs_and_reports_its_state() {
    let mut state = AppState::new();
    let result = call(
        &mut state,
        "commands.invoke",
        json!({"command_id": "view.grid", "document_id": "active"}),
    )
    .unwrap();
    assert_eq!(result["checked"], false);
    assert!(!state.settings.show_grid);
    assert_eq!(
        result["revision_before"], result["revision_after"],
        "the drawing did not change"
    );
    call(
        &mut state,
        "commands.invoke",
        json!({"command_id": "view.grid", "document_id": "active"}),
    )
    .unwrap();
    assert!(state.settings.show_grid);

    state.add_entity(Entity::Line {
        start: Point::new(0.0, 0.0),
        end: Point::new(5.0, 5.0),
    });
    call(
        &mut state,
        "commands.invoke",
        json!({"command_id": "edit.select_all", "document_id": "active"}),
    )
    .unwrap();
    assert_eq!(state.docs[0].selection.len(), 1);
}

#[test]
fn commands_that_are_interactive_or_denied_do_not_run() {
    let mut state = AppState::new();
    state.add_entity(Entity::Line {
        start: Point::new(0.0, 0.0),
        end: Point::new(5.0, 5.0),
    });
    let objects = state.session().document().len();
    for (id, policy) in [
        ("draw.line", "interactive_in_app"),
        ("modify.erase", "denied"),
        ("edit.undo", "denied"),
        ("file.exit", "denied"),
        ("file.close", "denied"),
        ("file.save", "denied"),
        ("manage.settings", "denied"),
    ] {
        let error = call(
            &mut state,
            "commands.invoke",
            json!({"command_id": id, "document_id": "active"}),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::CommandNotAutomatable, "{id}");
        assert_eq!(error.details["policy"], policy, "{id}");
    }
    assert!(!state.exit_requested && !state.show_settings && !state.command_running());
    assert_eq!(state.session().document().len(), objects);
    assert!(state.session().history().can_undo(), "undo was not run");
}

#[test]
fn bad_requests_are_refused_clearly() {
    let mut state = AppState::new();
    let code =
        |state: &mut AppState, args: Value| call(state, "commands.invoke", args).unwrap_err().code;
    assert_eq!(
        code(
            &mut state,
            json!({"command_id": "no.such", "document_id": "active"})
        ),
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        code(
            &mut state,
            json!({"command_id": "view.grid", "document_id": "doc_nope_1"})
        ),
        ErrorCode::DocumentNotFound
    );
    // Not enabled: nothing to select.
    assert_eq!(
        code(
            &mut state,
            json!({"command_id": "edit.select_all", "document_id": "active"})
        ),
        ErrorCode::InvalidArguments
    );
    // Automation never switches the user's tab.
    let first = document_id(INSTANCE, state.docs[0].uid);
    state.new_document();
    assert_eq!(state.active, 1);
    assert_eq!(
        code(
            &mut state,
            json!({"command_id": "view.grid", "document_id": first})
        ),
        ErrorCode::InvalidReference
    );
    assert_eq!(state.active, 1);
}

#[test]
fn the_scopes_keep_listing_apart_from_running() {
    assert_eq!(
        od_api::ops::required_scope("commands.list"),
        Some(od_api::Scope::DrawingRead)
    );
    assert_eq!(
        od_api::ops::required_scope("commands.invoke"),
        Some(od_api::Scope::CommandInvoke)
    );
}

#[test]
fn a_command_waits_while_a_dialog_is_open() {
    let mut state = AppState::new();
    state.show_settings = true;
    let args = json!({"command_id": "view.grid", "document_id": "active"});
    let error = call(&mut state, "commands.invoke", args.clone()).unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationBusy);
    assert!(error.retryable && state.settings.show_grid);
    state.show_settings = false;
    assert!(call(&mut state, "commands.invoke", args).is_ok());
}
