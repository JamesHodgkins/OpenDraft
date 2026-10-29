//! Plan first: while there is no approved plan the tools that change the
//! drawing are not offered, and a call to one anyway goes nowhere.

use serde_json::json;

use super::support::*;
use crate::{edit, Event, PlanOutcome, Step, TaskState};

const WRITE: &str = "opendraft_entities_apply_batch";

#[test]
fn before_a_plan_is_approved_no_change_is_offered_but_the_plan_tool_is() {
    let host = FakeHost::new();
    let run = go(vec![Step::say("nothing to do")], &host, &edit());
    let names = tool_names(&run, 0);
    assert!(!names.contains(&WRITE.to_owned()));
    assert!(!names.contains(&"opendraft_layers_apply_batch".to_owned()));
    assert!(names.contains(&"propose_plan".to_owned()));
    assert!(names.contains(&"opendraft_drawing_summary".to_owned()));
}

#[test]
fn a_change_called_before_approval_is_refused_and_never_reaches_the_host() {
    let host = FakeHost::new();
    let run = go(vec![batch_call("c1"), Step::say("ok")], &host, &edit());
    assert!(host.called().is_empty());
    assert!(last_results(&run)[0].1);
}

#[test]
fn an_approved_plan_opens_the_changes_and_the_plan_says_how_many() {
    let host = FakeHost::new();
    let run = go(
        vec![
            plan_call("c1", json!(["Move the line", "Change its layer"])),
            batch_call("c2"),
            Step::say("Done. Verified."),
            Step::say("Verified."),
        ],
        &host,
        &edit(),
    );
    assert_eq!(host.plans.lock().map(|p| p[0].steps.len()).unwrap_or(0), 2);
    let names_after = tool_names(&run, 1);
    assert!(names_after.contains(&WRITE.to_owned()));
    assert_eq!(host.called().len(), 1);
    assert_eq!(host.called()[0].name, WRITE);
    assert!(run
        .events
        .contains(&Event::State(TaskState::AwaitingApproval)));
    assert_eq!(run.report.state, TaskState::Completed);
    // The model was told the cap the app will enforce.
    let second = &run.provider.requests()[1];
    let told = format!("{:?}", second.messages);
    assert!(told.contains("up to 2 changes"), "{told}");
}

#[test]
fn a_change_in_the_same_turn_as_the_plan_waits_for_the_next_turn() {
    let host = FakeHost::new();
    let run = go(
        vec![
            Step::calls(vec![
                (
                    "c1",
                    "propose_plan",
                    json!({"summary": "s", "steps": ["one"]}),
                ),
                ("c2", WRITE, json!({"document": "d1", "operations": []})),
            ]),
            Step::say("ok"),
            Step::say("ok"),
        ],
        &host,
        &edit(),
    );
    assert!(
        host.called().is_empty(),
        "the change was not on offer when chosen"
    );
    let results = last_results(&run);
    assert!(!results[0].1);
    assert!(results[1].1);
}

#[test]
fn a_rejected_plan_keeps_the_changes_closed_and_tells_the_model_why() {
    let host = FakeHost::new();
    host.answers_plan(PlanOutcome::Rejected(Some("too many steps".to_owned())));
    let run = go(
        vec![
            plan_call("c1", json!(["a", "b", "c"])),
            batch_call("c2"),
            Step::say("I will not."),
        ],
        &host,
        &edit(),
    );
    let results = last_results(&run);
    assert!(results[0].1 && results[0].0.contains("too many steps"));
    assert!(!tool_names(&run, 1).contains(&WRITE.to_owned()));
    assert!(host.called().is_empty());
    assert!(run.events.contains(&Event::State(TaskState::Planning)));
}

#[test]
fn a_second_plan_after_a_rejection_can_open_the_changes() {
    let host = FakeHost::new();
    host.answers_plan(PlanOutcome::Rejected(None));
    let run = go(
        vec![
            plan_call("c1", json!(["a", "b", "c"])),
            plan_call("c2", json!(["a"])),
            batch_call("c3"),
            Step::say("done"),
            Step::say("verified"),
        ],
        &host,
        &edit(),
    );
    assert_eq!(host.called().len(), 1);
    assert_eq!(host.plans.lock().map(|p| p.len()).unwrap_or(0), 2);
    assert_eq!(run.report.state, TaskState::Completed);
}

#[test]
fn a_plan_with_no_steps_or_bad_steps_is_refused_before_the_user_sees_it() {
    for bad in [json!([]), json!([""]), json!([1, 2]), json!("one")] {
        let host = FakeHost::new();
        let run = go(vec![plan_call("c1", bad), Step::say("ok")], &host, &edit());
        assert!(host.plans.lock().map(|p| p.is_empty()).unwrap_or(false));
        assert!(last_results(&run)[0].1);
    }
}

#[test]
fn after_a_change_the_model_is_asked_once_to_check_it() {
    let host = FakeHost::new();
    let run = go(
        vec![
            plan_call("c1", json!(["Move the line"])),
            batch_call("c2"),
            Step::say("Moved."),
            Step::say("Checked: the line is where it should be."),
        ],
        &host,
        &edit(),
    );
    assert_eq!(run.report.model_calls, 4);
    assert!(run.events.contains(&Event::State(TaskState::Verifying)));
    assert_eq!(
        run.report.final_text.as_deref(),
        Some("Checked: the line is where it should be.")
    );
    let last = run
        .provider
        .requests()
        .pop()
        .map(|r| format!("{:?}", r.messages))
        .unwrap_or_default();
    assert!(last.contains("Check your work now"));
}

#[test]
fn no_check_is_asked_when_nothing_was_changed() {
    let host = FakeHost::new();
    let run = go(
        vec![summary_call("c1"), Step::say("Nothing needs changing.")],
        &host,
        &edit(),
    );
    assert_eq!(run.report.model_calls, 2);
    assert!(!run.events.contains(&Event::State(TaskState::Verifying)));
}

#[test]
fn a_failed_change_does_not_count_as_a_change_to_check() {
    let host = FakeHost::new();
    host.answers_tool(
        WRITE,
        crate::ToolOutcome::Failed(api_error(od_api::ErrorCode::StaleRevision, "changed")),
    );
    let run = go(
        vec![
            plan_call("c1", json!(["Move"])),
            batch_call("c2"),
            Step::say("It failed."),
        ],
        &host,
        &edit(),
    );
    assert_eq!(run.report.model_calls, 3);
    assert_eq!(run.report.state, TaskState::Completed);
}

#[test]
fn a_later_plan_that_is_rejected_closes_the_changes_again() {
    let host = FakeHost::new();
    host.answers_plan(PlanOutcome::Approved);
    host.answers_plan(PlanOutcome::Rejected(None));
    let run = go(
        vec![
            plan_call("c1", json!(["one"])),
            batch_call("c2"),
            plan_call("c3", json!(["two", "three"])),
            batch_call("c4"),
            Step::say("stopping"),
            Step::say("verified"),
        ],
        &host,
        &edit(),
    );
    assert_eq!(host.called().len(), 1, "the second change was never run");
    assert!(!tool_names(&run, 3).contains(&WRITE.to_owned()));
}
