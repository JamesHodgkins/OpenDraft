//! The client against a local server: what it sends, how each kind of answer
//! is sorted, that the key stays where it should, and that the whole loop
//! runs over it.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::server::{answer, closed_port, serve, Reply};
use crate::tests::support::FakeHost;
use crate::{
    qa_review, run_task, CancelToken, Message, MistralProvider, ModelRequest, Provider,
    ProviderError, StopReason, TaskConfig, TaskState,
};

const KEY: &str = "sk-test-secret-key";

fn request() -> ModelRequest {
    ModelRequest {
        system: "s".into(),
        messages: vec![Message::user("hello")],
        tools: Vec::new(),
        max_output_tokens: 100,
    }
}

fn ok_body(text: &str) -> String {
    json!({"choices": [{"message": {"role": "assistant", "content": text}, "finish_reason": "stop"}],
           "usage": {"prompt_tokens": 1, "completion_tokens": 1}})
    .to_string()
}

fn provider(url: &str) -> MistralProvider {
    MistralProvider::new(KEY)
        .with_base_url(url)
        .with_timeout(Duration::from_secs(5))
}

fn complete(p: &MistralProvider) -> Result<crate::ModelResponse, ProviderError> {
    p.complete(&request(), &CancelToken::new())
}

#[test]
fn it_posts_to_the_chat_endpoint_with_the_key_as_a_bearer_token() {
    let server = serve(vec![answer(200, &ok_body("hi"))]);
    let r = complete(&provider(&server.url)).unwrap();
    assert_eq!(r.message.text(), "hi");
    assert_eq!(r.stop, StopReason::EndTurn);
    let seen = server.seen.lock().unwrap()[0].clone();
    assert_eq!(seen.request_line, "POST /v1/chat/completions HTTP/1.1");
    assert_eq!(
        seen.header("authorization"),
        Some("Bearer sk-test-secret-key")
    );
    assert_eq!(seen.header("content-type"), Some("application/json"));
    let body: Value = serde_json::from_str(&seen.body).unwrap();
    assert_eq!(body["model"], crate::DEFAULT_MODEL);
    assert_eq!(body["messages"][1]["content"], "hello");
    assert!(
        !seen.body.contains(KEY),
        "the key is a header, never in the body"
    );
}

#[test]
fn a_rejected_key_is_fatal_and_the_message_never_holds_the_key() {
    let body = json!({"message": format!("Unauthorized: {KEY} is not valid")}).to_string();
    let server = serve(vec![answer(401, &body)]);
    let Err(ProviderError::Fatal(message)) = complete(&provider(&server.url)) else {
        panic!("expected a fatal failure");
    };
    assert!(message.contains("did not accept the key"), "{message}");
    assert!(!message.contains(KEY), "{message}");
}

#[test]
fn busy_and_broken_answers_are_worth_trying_again() {
    for status in [429, 500, 503] {
        let server = serve(vec![answer(status, "{\"message\":\"slow down\"}")]);
        let result = complete(&provider(&server.url));
        assert!(
            matches!(result, Err(ProviderError::Transient(_))),
            "{status}: {result:?}"
        );
    }
}

#[test]
fn a_bad_request_is_fatal_and_says_why() {
    let server = serve(vec![answer(
        400,
        "{\"object\":\"error\",\"message\":\"Invalid model\"}",
    )]);
    let Err(ProviderError::Fatal(message)) = complete(&provider(&server.url)) else {
        panic!("expected a fatal failure");
    };
    assert!(
        message.contains("Invalid model") && message.contains("400"),
        "{message}"
    );
}

#[test]
fn an_unreadable_success_is_fatal() {
    let server = serve(vec![answer(200, "<html>oops</html>")]);
    assert!(matches!(
        complete(&provider(&server.url)),
        Err(ProviderError::Fatal(_))
    ));
}

#[test]
fn nobody_listening_is_worth_trying_again() {
    let result = complete(&provider(&closed_port()));
    assert!(
        matches!(result, Err(ProviderError::Transient(_))),
        "{result:?}"
    );
}

#[test]
fn a_service_that_never_answers_times_out() {
    let server = serve(vec![Reply::Silence(Duration::from_secs(3))]);
    let p = provider(&server.url).with_timeout(Duration::from_millis(300));
    let started = Instant::now();
    let result = complete(&p);
    assert!(
        matches!(result, Err(ProviderError::Transient(_))),
        "{result:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn stop_before_the_call_sends_nothing() {
    let server = serve(vec![answer(200, &ok_body("hi"))]);
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(provider(&server.url).complete(&request(), &cancel).is_err());
    assert!(server.seen.lock().unwrap().is_empty());
}

#[test]
fn debug_output_hides_the_key() {
    let shown = format!("{:?}", MistralProvider::new(KEY));
    assert!(!shown.contains(KEY) && shown.contains("hidden"), "{shown}");
}

#[test]
fn answers_can_be_recorded_for_fixtures_and_hold_no_key() {
    let dir = std::env::temp_dir().join(format!("od-agent-record-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let server = serve(vec![answer(200, &ok_body("recorded"))]);
    let p = provider(&server.url).recording_to(dir.clone());
    complete(&p).unwrap();
    let kept = std::fs::read_to_string(dir.join("answer-1.json")).unwrap();
    assert!(kept.contains("recorded") && !kept.contains(KEY));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_whole_loop_runs_over_the_client() {
    let call = json!({"choices": [{"message": {"role": "assistant", "content": "", "tool_calls": [
        {"id": "D681PevKs", "type": "function",
         "function": {"name": "opendraft_drawing_summary", "arguments": "{\"document_id\":\"d1\"}"}}]},
        "finish_reason": "tool_calls"}]})
    .to_string();
    let server = serve(vec![
        answer(200, &call),
        answer(200, &ok_body("Three entities.")),
    ]);
    let host = FakeHost::new();
    let report = run_task(
        Arc::new(provider(&server.url)),
        &host,
        &qa_review(),
        "check it",
        TaskConfig::new("t1"),
        &CancelToken::new(),
        &mut |_| {},
    );
    assert_eq!(report.state, TaskState::Completed);
    assert_eq!(report.final_text.as_deref(), Some("Three entities."));
    assert_eq!(host.called().len(), 1);
    // The second request carried the tool's result back, named and id'd.
    let second: Value = serde_json::from_str(&server.seen.lock().unwrap()[1].body).unwrap();
    let tool = second["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "tool")
        .unwrap();
    assert_eq!(tool["tool_call_id"], "D681PevKs");
    assert_eq!(tool["name"], "opendraft_drawing_summary");
}

#[test]
fn a_transient_failure_is_retried_by_the_loop_over_the_client() {
    let server = serve(vec![answer(503, "{}"), answer(200, &ok_body("fine"))]);
    let host = FakeHost::new();
    let mut config = TaskConfig::new("t1");
    config.retry.base = Duration::from_millis(1);
    let report = run_task(
        Arc::new(provider(&server.url)),
        &host,
        &qa_review(),
        "go",
        config,
        &CancelToken::new(),
        &mut |_| {},
    );
    assert_eq!(report.state, TaskState::Completed);
    assert_eq!(report.model_calls, 2);
}
