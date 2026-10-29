//! The whole loop: the real harness, a scripted model, the app's host, and a
//! real drawing. These are the checks that a change the model asks for is
//! one undoable step made under the user's permission, and that Stop leaves
//! nothing behind.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use od_agent::{
    edit, qa_review, run_task, CancelToken, FakeProvider, PlanOutcome, Report, Step, TaskConfig,
    TaskState, Workflow,
};
use serde_json::json;

use super::support::*;
use super::{AppHost, ApprovalLevel, Decision};
use crate::state::AppState;

const WRITE: &str = "opendraft_entities_apply_batch";

struct Outcome {
    report: Report,
    user: Arc<Scripted>,
    host: AppHost,
}

fn config() -> TaskConfig {
    let mut config = TaskConfig::new("t1");
    config.retry.base = Duration::from_millis(1);
    config
}

fn run(
    state: &mut AppState,
    level: ApprovalLevel,
    workflow: &Workflow,
    script: impl FnOnce(&crate::automation::AutomationCore, &AppState, &Scripted) -> Vec<Step>,
    cancel: &CancelToken,
) -> (Outcome, Arc<crate::automation::AutomationCore>) {
    let core = core();
    let user = Scripted::new();
    let host = AppHost::new(core.clone(), user.clone(), level).unwrap();
    let steps = script(&core, state, &user);
    let provider = Arc::new(FakeProvider::new(steps));
    let report = drive(&core, state, quiet_user, || {
        run_task(
            provider,
            &host,
            workflow,
            "please",
            config(),
            cancel,
            &mut |_| {},
        )
    });
    (Outcome { report, user, host }, core)
}

#[test]
fn a_review_looks_at_the_drawing_and_reports_without_asking_anything() {
    let mut state = AppState::new();
    let (out, _) = run(
        &mut state,
        ApprovalLevel::ReviewEach,
        &qa_review(),
        |_, _, _| {
            vec![
                Step::call("c1", "opendraft_documents_list", json!({})),
                Step::say("One drawing, empty."),
            ]
        },
        &CancelToken::new(),
    );
    assert_eq!(out.report.state, TaskState::Completed);
    assert_eq!(out.user.change_prompts(), 0);
    assert_eq!(out.user.plan_prompts(), 0);
}

#[test]
fn an_approved_plan_makes_one_undoable_step_attributed_to_the_assistant() {
    let mut state = AppState::new();
    let (out, core) = run(
        &mut state,
        ApprovalLevel::ApprovePlan,
        &edit(),
        |core, state, _| {
            vec![
                Step::call(
                    "c1",
                    "propose_plan",
                    json!({"summary": "Add a line", "steps": ["Add a line"]}),
                ),
                Step::call("c2", WRITE, add_a_line(core, state, 0.0)),
                Step::say("Added."),
                Step::say("Checked."),
            ]
        },
        &CancelToken::new(),
    );
    assert_eq!(out.report.state, TaskState::Completed);
    assert_eq!(state.session().document().len(), 1);
    assert_eq!(out.user.plan_prompts(), 1);
    assert_eq!(out.user.change_prompts(), 0);
    let top = state.session().history().top_undo().unwrap();
    assert_eq!(
        top.provenance,
        od_commands::Provenance::Automation(out.host.id())
    );
    state.docs[0].session.undo();
    assert!(state.session().document().is_empty(), "one undo undoes it");
    assert!(
        core.task(out.host.id()).is_none(),
        "the task's grant is gone"
    );
}

#[test]
fn a_third_change_beyond_a_two_step_plan_is_not_made() {
    let mut state = AppState::new();
    let (out, _) = run(
        &mut state,
        ApprovalLevel::ApprovePlan,
        &edit(),
        |core, state, _| {
            let mut steps = vec![Step::call(
                "p",
                "propose_plan",
                json!({"summary": "Two lines", "steps": ["one", "two"]}),
            )];
            // The drawing's revision moves as each change lands, so each
            // call is built against a drawing as it will be: the first two
            // against revisions 0 and 1; the third is refused before it runs.
            for (i, x) in [0.0, 3.0, 6.0].into_iter().enumerate() {
                let mut args = add_a_line(core, state, x);
                args["expected_revision"] = json!(i.to_string());
                steps.push(Step::call(&format!("c{i}"), WRITE, args));
            }
            steps.push(Step::say("Done as far as allowed."));
            steps.push(Step::say("Checked."));
            steps
        },
        &CancelToken::new(),
    );
    assert_eq!(out.report.state, TaskState::Completed);
    assert_eq!(state.session().document().len(), 2);
    let told = format!("{:?}", out.report.messages);
    assert!(told.contains("scope_denied"), "the model was told why");
}

#[test]
fn a_declined_change_is_told_to_the_model_and_nothing_is_drawn() {
    let mut state = AppState::new();
    let (out, _) = run(
        &mut state,
        ApprovalLevel::ReviewEach,
        &edit(),
        |core, state, user| {
            user.decides(Decision::Declined);
            vec![
                Step::call("p", "propose_plan", json!({"summary": "s", "steps": ["a"]})),
                Step::call("c", WRITE, add_a_line(core, state, 0.0)),
                Step::say("Understood, I will leave it."),
            ]
        },
        &CancelToken::new(),
    );
    assert_eq!(out.report.state, TaskState::Completed);
    assert!(state.session().document().is_empty());
    assert_eq!(out.user.change_prompts(), 1);
}

#[test]
fn a_plan_the_user_rejects_means_no_change_can_be_made() {
    let mut state = AppState::new();
    let (out, _) = run(
        &mut state,
        ApprovalLevel::ApprovePlan,
        &edit(),
        |core, state, user| {
            user.answers_plan(PlanOutcome::Rejected(Some("not now".into())));
            vec![
                Step::call("p", "propose_plan", json!({"summary": "s", "steps": ["a"]})),
                Step::call("c", WRITE, add_a_line(core, state, 0.0)),
                Step::say("Stopping."),
            ]
        },
        &CancelToken::new(),
    );
    assert_eq!(out.report.state, TaskState::Completed);
    assert!(state.session().document().is_empty());
    assert_eq!(out.user.change_prompts(), 0);
}

#[test]
fn stop_ends_the_task_and_leaves_no_grant_behind() {
    let mut state = AppState::new();
    let cancel = CancelToken::new();
    let stopper = cancel.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(80));
        stopper.cancel();
    });
    let (out, core) = run(
        &mut state,
        ApprovalLevel::ApprovePlan,
        &edit(),
        |_, _, _| vec![Step::Hang],
        &cancel,
    );
    assert_eq!(out.report.state, TaskState::Cancelled);
    assert!(core.task(out.host.id()).is_none());
    assert!(state.session().document().is_empty());
}
