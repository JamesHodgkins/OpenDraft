//! The limits: a runaway loop spends the user's key, so each one ends the
//! task with a named reason and keeps what was said so far.

use serde_json::json;

use super::support::*;
use crate::{qa_review, BudgetKind, Content, Failure, Part, Step, TaskState, ToolOutcome};

fn endless_looking(n: usize) -> Vec<Step> {
    (0..n).map(|i| summary_call(&format!("c{i}"))).collect()
}

#[test]
fn the_model_call_limit_ends_the_task_and_keeps_the_conversation() {
    let host = FakeHost::new();
    let config = with_budgets(|b| b.model_calls = 3);
    let run = run(
        endless_looking(10),
        &host,
        &qa_review(),
        config,
        &crate::CancelToken::new(),
    );
    assert_eq!(
        run.report.state,
        TaskState::Failed(Failure::Budget(BudgetKind::ModelCalls))
    );
    assert_eq!(run.report.model_calls, 3);
    assert!(run.report.messages.len() >= 6, "what was said is kept");
    assert_eq!(host.finish_count(), 1);
}

#[test]
fn the_tool_call_limit_ends_the_task() {
    let host = FakeHost::new();
    let config = with_budgets(|b| b.tool_calls = 2);
    let run = run(
        endless_looking(10),
        &host,
        &qa_review(),
        config,
        &crate::CancelToken::new(),
    );
    assert_eq!(
        run.report.state,
        TaskState::Failed(Failure::Budget(BudgetKind::ToolCalls))
    );
    assert_eq!(host.called().len(), 2);
    // The call that hit the limit still has an answer in the conversation.
    let last = run.report.messages.last();
    assert!(matches!(
        last.map(|m| m.role),
        Some(crate::Role::ToolResults)
    ));
}

#[test]
fn the_token_limit_counts_what_the_provider_reports() {
    let host = FakeHost::new();
    // The fake reports 120 tokens a call.
    let config = with_budgets(|b| b.tokens = Some(200));
    let run = run(
        endless_looking(10),
        &host,
        &qa_review(),
        config,
        &crate::CancelToken::new(),
    );
    assert_eq!(
        run.report.state,
        TaskState::Failed(Failure::Budget(BudgetKind::Tokens))
    );
    assert_eq!(run.report.model_calls, 2);
    assert_eq!(run.report.tokens, 240);
}

#[test]
fn a_large_result_is_cut_where_the_model_can_see_it_was() {
    let host = FakeHost::new();
    host.answers_tool(
        "opendraft_drawing_summary",
        ToolOutcome::Done(json!({"blob": "x".repeat(5000)})),
    );
    let config = with_budgets(|b| b.result_bytes_each = 300);
    let run = run(
        vec![summary_call("c1"), Step::say("ok")],
        &host,
        &qa_review(),
        config,
        &crate::CancelToken::new(),
    );
    let text = &last_results(&run)[0].0;
    assert!(text.len() < 500);
    assert!(text.contains("[cut: "), "{text}");
    assert_eq!(run.report.state, TaskState::Completed);
}

#[test]
fn a_cut_never_splits_a_character() {
    let cut = crate::shape::cut("é".repeat(100), 51);
    assert!(cut.starts_with(&"é".repeat(25)));
    assert!(cut.contains("[cut: 50 of 200 bytes"));
}

#[test]
fn too_much_drawing_data_in_all_ends_the_task() {
    let host = FakeHost::new();
    host.answers_tool(
        "opendraft_drawing_summary",
        ToolOutcome::Done(json!({"blob": "x".repeat(400)})),
    );
    host.answers_tool(
        "opendraft_drawing_summary",
        ToolOutcome::Done(json!({"blob": "y".repeat(400)})),
    );
    let config = with_budgets(|b| b.result_bytes = 600);
    let run = run(
        endless_looking(4),
        &host,
        &qa_review(),
        config,
        &crate::CancelToken::new(),
    );
    assert_eq!(
        run.report.state,
        TaskState::Failed(Failure::Budget(BudgetKind::ResultBytes))
    );
}

fn picture_result() -> ToolOutcome {
    ToolOutcome::Done(json!({"png_base64": "iVBORw0KGgo=", "width": 10, "height": 10}))
}

fn images_in(run: &Run) -> usize {
    run.report
        .messages
        .iter()
        .flat_map(|m| m.parts.iter())
        .filter_map(|p| match p {
            Part::ToolResult(r) => Some(r),
            _ => None,
        })
        .flat_map(|r| r.content.iter())
        .filter(|c| matches!(c, Content::Image(_)))
        .count()
}

#[test]
fn a_picture_comes_through_as_a_picture_and_not_as_text() {
    let host = FakeHost::new();
    host.answers_tool("opendraft_view_render", picture_result());
    let run = go(
        vec![
            crate::Step::call("c1", "opendraft_view_render", json!({"document": "d1"})),
            Step::say("I see it."),
        ],
        &host,
        &qa_review(),
    );
    assert_eq!(images_in(&run), 1);
    assert!(!last_results(&run)[0].0.contains("iVBOR"));
    assert!(last_results(&run)[0].0.contains("width"));
}

#[test]
fn pictures_beyond_the_limit_are_not_passed_on_and_the_model_is_told() {
    let host = FakeHost::new();
    for _ in 0..3 {
        host.answers_tool("opendraft_view_render", picture_result());
    }
    let config = with_budgets(|b| b.pictures = 2);
    let steps = (0..3)
        .map(|i| {
            Step::call(
                &format!("c{i}"),
                "opendraft_view_render",
                json!({"document": "d1"}),
            )
        })
        .chain([Step::say("done")])
        .collect();
    let run = run(
        steps,
        &host,
        &qa_review(),
        config,
        &crate::CancelToken::new(),
    );
    assert_eq!(images_in(&run), 2);
    let results = last_results(&run);
    assert!(
        results[2].0.contains("picture not shown"),
        "{:?}",
        results[2]
    );
    assert_eq!(run.report.state, TaskState::Completed);
}
