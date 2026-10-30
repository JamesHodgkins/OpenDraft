//! The app side of the assistant, against a real `AppState` and the real
//! core: what is asked of the user, what is allowed without asking, and that
//! the harness has no way round either.

use std::sync::Arc;
use std::time::Duration;

use od_agent::{CancelToken, Host, Plan, PlanOutcome, ToolCall, ToolOutcome};
use od_api::ErrorCode;
use od_core::{Entity, Point};
use serde_json::{json, Value};

use super::support::*;
use super::{AppHost, ApprovalLevel, Decision, Reach, AUTO_APPROVE_CEILING};
use crate::automation::{document_id, AutomationCore, Coverage};
use crate::state::AppState;

const WRITE: &str = "opendraft_entities_apply_batch";

fn setup(level: ApprovalLevel) -> (Arc<AutomationCore>, Arc<Scripted>, AppHost, AppState) {
    let core = core();
    let user = Scripted::new();
    let host = AppHost::new(core.clone(), user.clone(), level, Reach::All).unwrap();
    (core, user, host, AppState::new())
}

fn call(name: &str, arguments: Value) -> ToolCall {
    ToolCall {
        id: "c1".to_owned(),
        name: name.to_owned(),
        arguments,
    }
}

fn plan(steps: usize) -> Plan {
    Plan {
        summary: "Draw lines".to_owned(),
        steps: (0..steps).map(|i| format!("line {i}")).collect(),
    }
}

fn lines(state: &AppState) -> usize {
    state.session().document().len()
}

fn code(outcome: &ToolOutcome) -> Option<ErrorCode> {
    match outcome {
        ToolOutcome::Failed(error) => Some(error.code),
        _ => None,
    }
}

/// One call, with the UI answering the core as it goes.
fn run(host: &AppHost, core: &AutomationCore, state: &mut AppState, c: ToolCall) -> ToolOutcome {
    let cancel = CancelToken::new();
    drive(core, state, quiet_user, || host.call(&c, &cancel))
}

#[test]
fn a_read_runs_without_asking_the_user() {
    let (core, user, host, mut state) = setup(ApprovalLevel::ReviewEach);
    let outcome = run(
        &host,
        &core,
        &mut state,
        call("opendraft_documents_list", json!({})),
    );
    let ToolOutcome::Done(result) = outcome else {
        panic!("{outcome:?}");
    };
    assert!(result.to_string().contains(core.instance_id()));
    assert_eq!(user.change_prompts(), 0);
}

#[test]
fn a_change_is_shown_exactly_as_it_will_be_made_and_happens_once_approved() {
    let (core, user, host, mut state) = setup(ApprovalLevel::ReviewEach);
    let args = add_a_line(&core, &state, 0.0);
    let outcome = run(&host, &core, &mut state, call(WRITE, args.clone()));
    assert!(matches!(outcome, ToolOutcome::Done(_)), "{outcome:?}");
    assert_eq!(lines(&state), 1);
    let shown = user.shown_changes.lock().unwrap().clone();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].tool, WRITE);
    assert_eq!(shown[0].operation, "entities.apply_batch");
    assert_eq!(shown[0].arguments, args);
    assert_eq!(
        state.session().history().top_undo().unwrap().provenance,
        od_commands::Provenance::Automation(host.id()),
        "attributed to the assistant's connection"
    );
}

#[test]
fn a_declined_change_changes_nothing() {
    let (core, user, host, mut state) = setup(ApprovalLevel::ReviewEach);
    user.decides(Decision::Declined);
    let args = add_a_line(&core, &state, 0.0);
    let outcome = run(&host, &core, &mut state, call(WRITE, args));
    assert_eq!(outcome, ToolOutcome::Declined);
    assert_eq!(lines(&state), 0);
    assert_eq!(user.change_prompts(), 1);
}

#[test]
fn stop_while_a_change_is_shown_ends_the_call_and_changes_nothing() {
    let (core, user, host, mut state) = setup(ApprovalLevel::ReviewEach);
    user.decides(Decision::Cancelled);
    let args = add_a_line(&core, &state, 0.0);
    let outcome = run(&host, &core, &mut state, call(WRITE, args));
    assert_eq!(outcome, ToolOutcome::Cancelled);
    assert_eq!(lines(&state), 0);
}

#[test]
fn an_approval_is_for_one_change_so_the_next_is_asked_again() {
    let (core, user, host, mut state) = setup(ApprovalLevel::ReviewEach);
    for x in [0.0, 5.0] {
        let args = add_a_line(&core, &state, x);
        let outcome = run(&host, &core, &mut state, call(WRITE, args));
        assert!(matches!(outcome, ToolOutcome::Done(_)), "{outcome:?}");
    }
    assert_eq!(lines(&state), 2);
    assert_eq!(user.change_prompts(), 2);
}

#[test]
fn without_an_approved_plan_a_plan_level_task_cannot_change_anything() {
    let (core, user, host, mut state) = setup(ApprovalLevel::ApprovePlan);
    let args = add_a_line(&core, &state, 0.0);
    let outcome = run(&host, &core, &mut state, call(WRITE, args));
    assert_eq!(code(&outcome), Some(ErrorCode::ScopeDenied));
    assert_eq!(lines(&state), 0);
    assert_eq!(user.change_prompts(), 0, "it does not fall back to asking");
}

#[test]
fn an_approved_plan_allows_as_many_changes_as_it_has_steps_and_no_more() {
    let (core, user, host, mut state) = setup(ApprovalLevel::ApprovePlan);
    assert_eq!(
        host.propose_plan(&plan(2), &CancelToken::new()),
        PlanOutcome::Approved
    );
    assert_eq!(user.plan_prompts(), 1);
    let mut outcomes = Vec::new();
    for x in [0.0, 5.0, 10.0] {
        let args = add_a_line(&core, &state, x);
        outcomes.push(run(&host, &core, &mut state, call(WRITE, args)));
    }
    assert!(matches!(outcomes[0], ToolOutcome::Done(_)));
    assert!(matches!(outcomes[1], ToolOutcome::Done(_)));
    assert_eq!(code(&outcomes[2]), Some(ErrorCode::ScopeDenied));
    let ToolOutcome::Failed(error) = &outcomes[2] else {
        panic!("{:?}", outcomes[2]);
    };
    assert!(
        error.message.contains("Propose a new plan"),
        "the model is told what to do: {}",
        error.message
    );
    assert_eq!(lines(&state), 2, "the third change was not made");
    assert_eq!(user.change_prompts(), 0, "the plan was the approval");
}

#[test]
fn a_rejected_plan_closes_what_an_earlier_one_opened() {
    let (core, user, host, mut state) = setup(ApprovalLevel::ApprovePlan);
    assert_eq!(
        host.propose_plan(&plan(3), &CancelToken::new()),
        PlanOutcome::Approved
    );
    user.answers_plan(PlanOutcome::Rejected(None));
    assert!(matches!(
        host.propose_plan(&plan(5), &CancelToken::new()),
        PlanOutcome::Rejected(_)
    ));
    let args = add_a_line(&core, &state, 0.0);
    let outcome = run(&host, &core, &mut state, call(WRITE, args));
    assert_eq!(code(&outcome), Some(ErrorCode::ScopeDenied));
    assert_eq!(lines(&state), 0);
}

#[test]
fn review_each_shows_the_plan_but_a_yes_to_it_allows_no_change_by_itself() {
    let (core, user, host, mut state) = setup(ApprovalLevel::ReviewEach);
    assert_eq!(
        host.propose_plan(&plan(4), &CancelToken::new()),
        PlanOutcome::Approved
    );
    assert_eq!(
        core.task(host.id()).unwrap().scopes,
        [od_api::Scope::UiSelection]
    );
    let args = add_a_line(&core, &state, 0.0);
    run(&host, &core, &mut state, call(WRITE, args));
    assert_eq!(user.change_prompts(), 1, "the change is still asked");
}

#[test]
fn auto_approve_is_capped_by_the_code_whatever_the_setting() {
    let (core, user, host, _state) = setup(ApprovalLevel::AutoApprove { limit: 1000 });
    assert_eq!(
        host.propose_plan(&plan(1), &CancelToken::new()),
        PlanOutcome::Approved
    );
    assert_eq!(user.plan_prompts(), 0, "the plan is not put to the user");
    let task = core.task(host.id()).unwrap();
    assert_eq!(task.max_changes, Some(AUTO_APPROVE_CEILING));
    assert!(task.scopes.contains(&od_api::Scope::DrawingWrite));
}

#[test]
fn finishing_takes_back_the_task_and_every_unused_approval() {
    let (core, _user, host, state) = setup(ApprovalLevel::ApprovePlan);
    host.propose_plan(&plan(2), &CancelToken::new());
    let args = add_a_line(&core, &state, 0.0);
    assert_eq!(
        core.coverage(host.id(), "entities.apply_batch", &args),
        Coverage::Covered
    );
    host.finish();
    assert!(core.task(host.id()).is_none());
    assert_eq!(
        core.coverage(host.id(), "entities.apply_batch", &args),
        Coverage::NeedsApproval
    );
}

#[test]
fn the_assistants_grant_is_read_only_beyond_its_task_and_hidden_from_settings() {
    let (core, _user, host, _state) = setup(ApprovalLevel::ReviewEach);
    let grants = core.grants();
    let mine = grants.iter().find(|(id, _)| *id == host.id()).unwrap();
    assert!(mine.1.in_process);
    assert_eq!(mine.1.scopes, [od_api::Scope::DrawingRead]);
}

#[test]
fn the_user_picks_objects_and_the_model_gets_their_ids() {
    let (core, _user, host, mut state) = setup(ApprovalLevel::ReviewEach);
    for x in [0.0, 3.0] {
        state.add_entity(Entity::Line {
            start: Point::new(x, 0.0),
            end: Point::new(x + 1.0, 0.0),
        });
    }
    let ids: Vec<_> = state
        .session()
        .document()
        .objects()
        .iter()
        .map(|o| o.id)
        .collect();
    let doc = crate::automation::document_id(core.instance_id(), state.docs[0].uid);
    let c = call(
        "opendraft_selection_request",
        json!({"document_id": doc, "prompt": "Pick the lines", "min_count": 1}),
    );
    let cancel = CancelToken::new();
    let mut picked = false;
    let outcome = drive(
        &core,
        &mut state,
        |state| {
            if !picked && state.command_running() {
                picked = true;
                state.select_ids(ids.iter().copied(), false);
                state.answer_enter();
                state.settle_command();
            }
        },
        || host.call(&c, &cancel),
    );
    let ToolOutcome::Done(status) = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(status["state"], "succeeded", "{status}");
    assert_eq!(status["entity_ids"].as_array().map(Vec::len), Some(2));
}

#[test]
fn stop_withdraws_a_prompt_the_user_has_not_answered() {
    let (core, _user, host, mut state) = setup(ApprovalLevel::ReviewEach);
    state.add_entity(Entity::Line {
        start: Point::new(0.0, 0.0),
        end: Point::new(1.0, 0.0),
    });
    let doc = crate::automation::document_id(core.instance_id(), state.docs[0].uid);
    let c = call(
        "opendraft_selection_request",
        json!({"document_id": doc, "prompt": "Pick"}),
    );
    let cancel = CancelToken::new();
    let stopper = cancel.clone();
    let mut asked_at = None;
    let outcome = drive(
        &core,
        &mut state,
        |state| {
            if state.command_running() && asked_at.is_none() {
                asked_at = Some(std::time::Instant::now());
            }
            if asked_at.is_some_and(|t| t.elapsed() > Duration::from_millis(100)) {
                stopper.cancel();
            }
        },
        || host.call(&c, &cancel),
    );
    assert_eq!(outcome, ToolOutcome::Cancelled);
    // The next frame takes the prompt off the screen.
    let registry = crate::commands::build_registry();
    for _ in 0..5 {
        core.poll(&mut state, &registry);
    }
    assert!(!state.command_running(), "the prompt is gone");
}

#[test]
fn a_change_comes_back_with_a_picture_of_what_it_changed() {
    let (core, _user, host, mut state) = setup(ApprovalLevel::ReviewEach);
    let args = add_a_line(&core, &state, 0.0);
    let ToolOutcome::Done(result) = run(&host, &core, &mut state, call(WRITE, args)) else {
        panic!("not done");
    };
    assert!(result["png_base64"].as_str().is_some_and(|p| p.len() > 100));
    assert!(result["picture_of"].as_str().is_some());
    assert!(result["picture_region"].is_object());
    assert_eq!(
        result["changed"], true,
        "the change itself is still reported"
    );
}

#[test]
fn a_read_gets_no_picture_it_did_not_ask_for() {
    let (core, _user, host, mut state) = setup(ApprovalLevel::ReviewEach);
    let ToolOutcome::Done(result) = run(
        &host,
        &core,
        &mut state,
        call("opendraft_documents_list", json!({})),
    ) else {
        panic!("not done");
    };
    assert!(result.get("png_base64").is_none());
}

#[test]
fn a_change_to_layers_alone_gets_no_picture() {
    let (core, _user, host, mut state) = setup(ApprovalLevel::ReviewEach);
    let args = json!({
        "document_id": document_id(core.instance_id(), state.docs[0].uid),
        "expected_revision": state.session().revision().to_string(),
        "operations": [{"op": "create", "client_ref": "k", "name": "Holes"}],
    });
    let outcome = run(
        &host,
        &core,
        &mut state,
        call("opendraft_layers_apply_batch", args),
    );
    let ToolOutcome::Done(result) = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(result["changed"], true);
    assert!(result.get("png_base64").is_none(), "{result}");
}
