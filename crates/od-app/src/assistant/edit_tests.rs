//! Making changes: taking a task back, which drawings it may reach, and what
//! it is told about them.

use od_agent::{Step, ToolOutcome};
use od_core::{Entity, Point};
use serde_json::json;

use super::chat_tests::{edit_rig, plan_then_draw, rig};
use super::support::{add_a_line, core};
use super::{AppHost, ApprovalLevel, Drawings, Entry, Kind, Reach, Reply};
use crate::automation::document_id;
use crate::state::AppState;

/// A task that draws one line, run to its end with the plan approved.
fn one_change(rig: &mut super::chat_tests::Rig) {
    let steps = plan_then_draw(rig);
    rig.script(steps);
    rig.send("draw a line");
    rig.pump(|chat| chat.chat().pending.is_some());
    rig.chat
        .answer(Reply::Plan(od_agent::PlanOutcome::Approved));
    rig.finish();
}

fn made(rig: &super::chat_tests::Rig) -> Vec<(usize, Entry)> {
    rig.entries()
        .into_iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, Entry::Made { .. }))
        .collect()
}

fn user_draws(rig: &mut super::chat_tests::Rig) {
    rig.state.add_entity(Entity::Line {
        start: Point::new(0.0, 5.0),
        end: Point::new(3.0, 5.0),
    });
}

#[test]
fn a_task_that_changed_the_drawing_says_so_and_can_be_undone() {
    let mut rig = edit_rig(ApprovalLevel::ApprovePlan);
    one_change(&mut rig);
    assert_eq!(rig.state.session().document().len(), 1);
    let made = made(&rig);
    assert_eq!(made.len(), 1);
    let (
        index,
        Entry::Made {
            changes, outcome, ..
        },
    ) = made[0].clone()
    else {
        panic!("a Made entry");
    };
    assert_eq!((changes, outcome), (1, None));
    rig.chat.undo(index, &mut rig.state);
    assert_eq!(rig.state.session().document().len(), 0, "taken back");
    let Entry::Made { outcome, .. } = &rig.entries()[index] else {
        panic!("still a Made entry");
    };
    assert_eq!(outcome.as_deref(), Some("Undid 1 change."));
}

#[test]
fn a_task_that_changed_nothing_has_nothing_to_undo() {
    let mut rig = rig(Some("k"));
    rig.script(vec![Step::say("Nothing to change.")]);
    rig.send("look");
    rig.finish();
    assert!(made(&rig).is_empty());
}

#[test]
fn undo_is_refused_once_you_have_edited_since_and_your_work_is_kept() {
    let mut rig = edit_rig(ApprovalLevel::ApprovePlan);
    one_change(&mut rig);
    user_draws(&mut rig);
    assert_eq!(rig.state.session().document().len(), 2);
    let (index, _) = made(&rig)[0].clone();
    rig.chat.undo(index, &mut rig.state);
    assert_eq!(
        rig.state.session().document().len(),
        2,
        "nothing was undone"
    );
    let Entry::Made { outcome, .. } = &rig.entries()[index] else {
        panic!("a Made entry");
    };
    assert!(outcome
        .as_deref()
        .is_some_and(|o| o.starts_with("Nothing could be undone")));
}

#[test]
fn undo_takes_back_only_that_task_and_not_an_earlier_one() {
    let mut rig = edit_rig(ApprovalLevel::ApprovePlan);
    one_change(&mut rig);
    // A second task draws another line.
    let steps = vec![
        Step::call(
            "p",
            "propose_plan",
            json!({"summary": "Another", "steps": ["Add"]}),
        ),
        Step::call(
            "c",
            "opendraft_entities_apply_batch",
            add_a_line(&rig.core, &rig.state, 7.0),
        ),
        Step::say("Added."),
        Step::say("Checked."),
    ];
    rig.script(steps);
    rig.send("and another");
    rig.pump(|chat| chat.chat().pending.is_some());
    rig.chat
        .answer(Reply::Plan(od_agent::PlanOutcome::Approved));
    rig.finish();
    assert_eq!(rig.state.session().document().len(), 2);
    let tasks = made(&rig);
    assert_eq!(tasks.len(), 2);
    // Undo the second: one line goes, the first stays.
    rig.chat.undo(tasks[1].0, &mut rig.state);
    assert_eq!(rig.state.session().document().len(), 1);
    // Undo the first: it is now on top, so it goes too.
    rig.chat.undo(tasks[0].0, &mut rig.state);
    assert_eq!(rig.state.session().document().len(), 0);
}

#[test]
fn undo_waits_for_a_running_command() {
    let mut rig = edit_rig(ApprovalLevel::ApprovePlan);
    one_change(&mut rig);
    let (index, _) = made(&rig)[0].clone();
    rig.state.start_command("Line", crate::draw::draw_line);
    rig.state.settle_command();
    assert!(rig.state.command_running());
    rig.chat.undo(index, &mut rig.state);
    assert_eq!(rig.state.session().document().len(), 1, "left alone");
    let Entry::Made { outcome, .. } = &rig.entries()[index] else {
        panic!("a Made entry");
    };
    assert!(outcome
        .as_deref()
        .is_some_and(|o| o.contains("running command")));
}

// ---- which drawings a task may reach

fn two_drawings() -> (
    std::sync::Arc<crate::automation::AutomationCore>,
    AppState,
    String,
    String,
) {
    let core = core();
    let mut state = AppState::new();
    let first = document_id(core.instance_id(), state.docs[0].uid);
    state.new_document();
    let second = document_id(core.instance_id(), state.docs[state.active].uid);
    (core, state, first, second)
}

fn host(core: &std::sync::Arc<crate::automation::AutomationCore>, reach: Reach) -> AppHost {
    AppHost::new(
        core.clone(),
        super::support::Scripted::new(),
        ApprovalLevel::ReviewEach,
        reach,
    )
    .unwrap()
}

fn call(
    host: &AppHost,
    core: &crate::automation::AutomationCore,
    state: &mut AppState,
    name: &str,
    args: serde_json::Value,
) -> ToolOutcome {
    use od_agent::{CancelToken, Host, ToolCall};
    let call = ToolCall {
        id: "c".into(),
        name: name.into(),
        arguments: args,
    };
    let cancel = CancelToken::new();
    super::support::drive(core, state, super::support::quiet_user, || {
        host.call(&call, &cancel)
    })
}

#[test]
fn a_task_on_one_drawing_cannot_read_another() {
    let (core, mut state, first, second) = two_drawings();
    let host = host(&core, Reach::Only(first.clone()));
    let own = call(
        &host,
        &core,
        &mut state,
        "opendraft_drawing_summary",
        json!({"document_id": first}),
    );
    assert!(matches!(own, ToolOutcome::Done(_)), "{own:?}");
    let other = call(
        &host,
        &core,
        &mut state,
        "opendraft_drawing_summary",
        json!({"document_id": second}),
    );
    let ToolOutcome::Failed(error) = other else {
        panic!("expected a refusal");
    };
    assert_eq!(error.code, od_api::ErrorCode::ScopeDenied);
    assert!(error.message.contains("All open drawings"));
}

#[test]
fn a_task_on_one_drawing_sees_only_that_drawing_listed() {
    let (core, mut state, first, _second) = two_drawings();
    let only = host(&core, Reach::Only(first.clone()));
    let ToolOutcome::Done(listed) = call(
        &only,
        &core,
        &mut state,
        "opendraft_documents_list",
        json!({}),
    ) else {
        panic!("a list");
    };
    let ids: Vec<&str> = listed["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["document_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [first.as_str()]);
    let all = host(&core, Reach::All);
    let ToolOutcome::Done(listed) = call(
        &all,
        &core,
        &mut state,
        "opendraft_documents_list",
        json!({}),
    ) else {
        panic!("a list");
    };
    assert_eq!(listed["documents"].as_array().unwrap().len(), 2);
}

#[test]
fn all_open_drawings_lets_a_task_use_any_of_them() {
    let (core, mut state, _first, second) = two_drawings();
    let all = host(&core, Reach::All);
    let result = call(
        &all,
        &core,
        &mut state,
        "opendraft_drawing_summary",
        json!({"document_id": second}),
    );
    assert!(matches!(result, ToolOutcome::Done(_)), "{result:?}");
}

#[test]
fn a_change_on_another_drawing_is_refused_before_the_user_is_asked() {
    let (core, mut state, first, second) = two_drawings();
    let user = super::support::Scripted::new();
    let host = AppHost::new(
        core.clone(),
        user.clone(),
        ApprovalLevel::ReviewEach,
        Reach::Only(first),
    )
    .unwrap();
    let args = json!({"document_id": second, "expected_revision": "0", "operations": []});
    let result = call(
        &host,
        &core,
        &mut state,
        "opendraft_entities_apply_batch",
        args,
    );
    assert!(matches!(result, ToolOutcome::Failed(_)));
    assert_eq!(
        user.change_prompts(),
        0,
        "no question for what is not allowed"
    );
}

#[test]
fn the_model_is_told_what_it_may_use() {
    let mut rig = rig(Some("k"));
    rig.state.new_document();
    rig.script(vec![Step::say("ok")]);
    rig.script(vec![Step::say("ok")]);
    rig.send("one");
    rig.finish();
    let current = rig.requests(0)[0].messages[0].text();
    assert!(current.contains("only drawing you may use"), "{current}");
    rig.chat.drawings = Drawings::All;
    rig.send("two");
    rig.finish();
    let all = rig.requests(1)[0]
        .messages
        .last()
        .map(|m| m.text())
        .unwrap_or_default();
    assert!(all.contains("all the open drawings"), "{all}");
    assert_eq!(
        all.matches("document_id").count(),
        3,
        "the view, then each of the two: {all}"
    );
    let _ = Kind::Review;
}
