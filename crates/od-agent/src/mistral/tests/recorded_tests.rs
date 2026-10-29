//! Answers recorded from the real service on 2026-10-29 (mistral-large-latest,
//! by `tests/mistral_live.rs`), kept as they came. The wire tests next door
//! are written from the documentation; these are the check that the
//! documentation and the service agree.

use serde_json::{json, Value};

use crate::mistral::raise;
use crate::StopReason;

fn recorded(text: &str) -> Value {
    serde_json::from_str(text).expect("a recorded answer is JSON")
}

#[test]
fn a_recorded_tool_call_without_arguments() {
    let r = raise::response(&recorded(include_str!(
        "recorded/tool_call_no_arguments.json"
    )))
    .unwrap();
    assert_eq!(r.stop, StopReason::ToolUse);
    let calls = r.message.tool_calls();
    assert_eq!(calls.len(), 1);
    // Ids are nine letters and digits, as the documentation's example showed.
    assert_eq!(calls[0].id.len(), 9);
    assert!(calls[0].id.chars().all(|c| c.is_ascii_alphanumeric()));
    assert_eq!(calls[0].name, "opendraft_documents_list");
    assert_eq!(calls[0].arguments, json!({}));
    assert_eq!(r.message.text(), "", "empty content beside a call");
    assert_eq!((r.usage.input, r.usage.output), (Some(4289), Some(10)));
}

#[test]
fn a_recorded_tool_call_with_arguments() {
    let r = raise::response(&recorded(include_str!(
        "recorded/tool_call_with_arguments.json"
    )))
    .unwrap();
    let calls = r.message.tool_calls();
    assert_eq!(calls[0].name, "opendraft_drawing_summary");
    assert_eq!(calls[0].arguments, json!({"document_id": "doc_1"}));
}

#[test]
fn a_recorded_final_answer_has_tool_calls_null() {
    let r = raise::response(&recorded(include_str!(
        "recorded/final_text_tool_calls_null.json"
    )))
    .unwrap();
    assert_eq!(r.stop, StopReason::EndTurn);
    assert!(r.message.tool_calls().is_empty());
    assert!(r.message.text().contains("12 entities"));
    assert_eq!((r.usage.input, r.usage.output), (Some(4373), Some(30)));
}
