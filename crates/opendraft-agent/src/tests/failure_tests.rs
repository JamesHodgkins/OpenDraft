//! When the model service fails, hangs or is stopped, and when the user
//! presses Stop: the task ends the right way and gives back what it held.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;

use super::support::*;
use crate::{
    qa_review, run_task, BudgetKind, CancelToken, Failure, FakeProvider, ProviderError, Step,
    TaskState, ToolOutcome,
};

#[test]
fn a_transient_failure_is_tried_again_and_the_task_goes_on() {
    let host = FakeHost::new();
    let run = go(
        vec![
            Step::Fail(ProviderError::Transient("rate limited".into())),
            Step::say("fine"),
        ],
        &host,
        &qa_review(),
    );
    assert_eq!(run.report.state, TaskState::Completed);
    assert_eq!(run.report.model_calls, 2, "a try is a call, for the budget");
}

#[test]
fn a_transient_failure_that_keeps_failing_ends_the_task_after_the_tries() {
    let host = FakeHost::new();
    let run = go(
        vec![
            Step::Fail(ProviderError::Transient("down".into())),
            Step::Fail(ProviderError::Transient("down".into())),
            Step::Fail(ProviderError::Transient("down".into())),
            Step::say("never reached"),
        ],
        &host,
        &qa_review(),
    );
    assert_eq!(
        run.report.state,
        TaskState::Failed(Failure::Provider("down".into()))
    );
    assert_eq!(run.provider.remaining(), 1);
    assert_eq!(host.finish_count(), 1);
}

#[test]
fn a_fatal_failure_is_not_tried_again() {
    let host = FakeHost::new();
    let run = go(
        vec![
            Step::Fail(ProviderError::Fatal("bad key".into())),
            Step::say("never"),
        ],
        &host,
        &qa_review(),
    );
    assert_eq!(
        run.report.state,
        TaskState::Failed(Failure::Provider("bad key".into()))
    );
    assert_eq!(run.report.model_calls, 1);
}

#[test]
fn a_refusal_is_its_own_failure() {
    let host = FakeHost::new();
    let run = go(
        vec![Step::Fail(ProviderError::Refused("policy".into()))],
        &host,
        &qa_review(),
    );
    assert_eq!(
        run.report.state,
        TaskState::Failed(Failure::Refused("policy".into()))
    );
}

#[test]
fn a_model_that_never_answers_times_out_then_tries_again() {
    let host = FakeHost::new();
    let config = with_budgets(|b| b.model_call = Duration::from_millis(60));
    let run = run(
        vec![Step::Hang, Step::say("late but fine")],
        &host,
        &qa_review(),
        config,
        &CancelToken::new(),
    );
    assert_eq!(run.report.state, TaskState::Completed);
    assert_eq!(run.report.model_calls, 2);
}

#[test]
fn a_model_that_always_hangs_ends_the_task_with_a_failure_not_a_hang() {
    let host = FakeHost::new();
    let mut config = with_budgets(|b| b.model_call = Duration::from_millis(40));
    config.retry.tries = 2;
    let started = Instant::now();
    let run = run(
        vec![Step::Hang, Step::Hang],
        &host,
        &qa_review(),
        config,
        &CancelToken::new(),
    );
    assert!(matches!(
        run.report.state,
        TaskState::Failed(Failure::Provider(_))
    ));
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn the_time_limit_ends_a_task_even_while_the_model_is_silent() {
    let host = FakeHost::new();
    let config = with_budgets(|b| {
        b.elapsed = Duration::from_millis(60);
        b.model_call = Duration::from_secs(30);
    });
    let started = Instant::now();
    let run = run(
        vec![Step::Hang],
        &host,
        &qa_review(),
        config,
        &CancelToken::new(),
    );
    assert_eq!(
        run.report.state,
        TaskState::Failed(Failure::Budget(BudgetKind::Time))
    );
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn stop_while_the_model_is_thinking_ends_the_task_at_once() {
    let host = FakeHost::new();
    let cancel = CancelToken::new();
    let stopper = cancel.clone();
    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(80));
        stopper.cancel();
    });
    let started = Instant::now();
    let run = run(vec![Step::Hang], &host, &qa_review(), quick(), &cancel);
    let _ = handle.join();
    assert_eq!(run.report.state, TaskState::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(host.finish_count(), 1, "what the task held is given back");
}

#[test]
fn stop_before_the_first_call_asks_the_model_nothing() {
    let host = FakeHost::new();
    let cancel = CancelToken::new();
    cancel.cancel();
    let run = run(vec![Step::say("x")], &host, &qa_review(), quick(), &cancel);
    assert_eq!(run.report.state, TaskState::Cancelled);
    assert_eq!(run.report.model_calls, 0);
    assert_eq!(host.finish_count(), 1);
}

#[test]
fn stop_during_a_tool_answers_every_call_so_the_conversation_stays_whole() {
    let host = FakeHost::new();
    let cancel = CancelToken::new();
    host.cancels_on_first_call(&cancel);
    host.answers_tool("opendraft_drawing_summary", ToolOutcome::Cancelled);
    let run = run(
        vec![
            Step::calls(vec![
                ("c1", "opendraft_drawing_summary", json!({})),
                ("c2", "opendraft_drawing_audit", json!({"document": "d1"})),
            ]),
            Step::say("never"),
        ],
        &host,
        &qa_review(),
        quick(),
        &cancel,
    );
    assert_eq!(run.report.state, TaskState::Cancelled);
    assert_eq!(host.called().len(), 1, "the second call was not run");
    let last = run
        .report
        .messages
        .last()
        .map(|m| m.parts.len())
        .unwrap_or(0);
    assert_eq!(last, 2, "both calls have an answer");
    assert_eq!(host.finish_count(), 1);
}

#[test]
fn an_answer_that_arrives_after_stop_is_dropped() {
    let host = FakeHost::new();
    let cancel = CancelToken::new();
    let stopper = cancel.clone();
    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(30));
        stopper.cancel();
    });
    let provider = Arc::new(FakeProvider::new(vec![Step::After(
        Duration::from_millis(200),
        Box::new(Step::call("c1", "opendraft_drawing_summary", json!({}))),
    )]));
    let report = run_task(
        provider,
        &host,
        &qa_review(),
        "go",
        quick(),
        &cancel,
        &mut |_| {},
    );
    let _ = handle.join();
    assert_eq!(report.state, TaskState::Cancelled);
    assert!(
        host.called().is_empty(),
        "no tool ran for an answer nobody waited for"
    );
}

#[test]
fn an_answer_cut_short_is_a_failure_not_a_half_call() {
    let host = FakeHost::new();
    let run = go(
        vec![Step::reply(
            vec![crate::Part::ToolCall(crate::ToolCall {
                id: "c1".into(),
                name: "opendraft_drawing_summary".into(),
                arguments: json!({}),
            })],
            crate::StopReason::MaxTokens,
        )],
        &host,
        &qa_review(),
    );
    assert!(matches!(
        run.report.state,
        TaskState::Failed(Failure::Model(_))
    ));
    assert!(host.called().is_empty());
}

#[test]
fn a_refusing_stop_reason_fails_the_task() {
    let host = FakeHost::new();
    let run = go(
        vec![Step::reply(
            vec![crate::Part::Text("no".into())],
            crate::StopReason::Refused,
        )],
        &host,
        &qa_review(),
    );
    assert!(matches!(
        run.report.state,
        TaskState::Failed(Failure::Refused(_))
    ));
}

#[test]
fn stop_is_felt_even_when_the_model_call_cannot_be_interrupted() {
    // A real HTTP call ignores the token until it returns; the loop must stop
    // waiting for it, and the late answer is dropped.
    let host = FakeHost::new();
    let cancel = CancelToken::new();
    let stopper = cancel.clone();
    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        stopper.cancel();
    });
    let started = Instant::now();
    let run = run(
        vec![Step::After(
            Duration::from_secs(2),
            Box::new(Step::say("too late")),
        )],
        &host,
        &qa_review(),
        quick(),
        &cancel,
    );
    let _ = handle.join();
    assert_eq!(run.report.state, TaskState::Cancelled);
    assert!(started.elapsed() < Duration::from_millis(1500));
    assert_eq!(run.report.final_text, None);
}
