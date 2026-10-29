//! Our types to Mistral's form and back. The answers here are written from
//! Mistral's documentation; `recorded_tests.rs` holds real ones, and covers
//! the common shapes. What only the documentation covers (chunked text,
//! arguments as an object) stays here.

use serde_json::{json, Value};

use crate::mistral::{lower, raise};
use crate::{
    Content, Image, Message, ModelRequest, Part, ProviderError, Role, StopReason, ToolCall,
    ToolResult, ToolSpec,
};

fn request(messages: Vec<Message>) -> ModelRequest {
    ModelRequest {
        system: "You are careful.".to_owned(),
        messages,
        tools: vec![ToolSpec {
            name: "opendraft_drawing_summary".to_owned(),
            description: "Summarise".to_owned(),
            schema: match json!({"type": "object", "properties": {}}) {
                Value::Object(m) => m,
                _ => Default::default(),
            },
        }],
        max_output_tokens: 777,
    }
}

fn call(id: &str, name: &str) -> Part {
    Part::ToolCall(ToolCall {
        id: id.to_owned(),
        name: name.to_owned(),
        arguments: json!({"document_id": "d1"}),
    })
}

fn picture() -> Image {
    Image {
        media_type: "image/png".to_owned(),
        base64: "AAAA".to_owned(),
        label: "the sheet".to_owned(),
    }
}

#[test]
fn a_request_has_the_system_prompt_first_the_tools_and_the_limits() {
    let body = lower::body(&request(vec![Message::user("hello")]), "m1", 0.2);
    assert_eq!(body["model"], "m1");
    assert_eq!(body["max_tokens"], 777);
    assert_eq!(body["tool_choice"], "auto");
    assert_eq!(
        body["messages"][0],
        json!({"role": "system", "content": "You are careful."})
    );
    assert_eq!(
        body["messages"][1],
        json!({"role": "user", "content": "hello"})
    );
    let tool = &body["tools"][0];
    assert_eq!(tool["type"], "function");
    assert_eq!(tool["function"]["name"], "opendraft_drawing_summary");
    assert_eq!(tool["function"]["parameters"]["type"], "object");
}

#[test]
fn no_tools_means_no_tool_fields() {
    let mut r = request(vec![Message::user("hi")]);
    r.tools.clear();
    let body = lower::body(&r, "m", 0.2);
    assert!(body.get("tools").is_none() && body.get("tool_choice").is_none());
}

#[test]
fn a_tool_call_goes_out_with_its_arguments_as_a_json_string() {
    let messages = vec![
        Message::user("go"),
        Message::assistant(vec![
            Part::Text("Looking.".into()),
            call("abc123XYZ", "opendraft_drawing_summary"),
        ]),
    ];
    let body = lower::body(&request(messages), "m", 0.2);
    let assistant = &body["messages"][2];
    assert_eq!(assistant["role"], "assistant");
    assert_eq!(assistant["content"], "Looking.");
    let function = &assistant["tool_calls"][0]["function"];
    assert_eq!(assistant["tool_calls"][0]["id"], "abc123XYZ");
    let arguments = function["arguments"].as_str().expect("a string");
    assert_eq!(
        serde_json::from_str::<Value>(arguments).unwrap(),
        json!({"document_id": "d1"})
    );
}

#[test]
fn a_result_is_a_tool_message_naming_the_tool_it_answers() {
    let messages = vec![
        Message::user("go"),
        Message::assistant(vec![call("c1", "opendraft_drawing_summary")]),
        Message {
            role: Role::ToolResults,
            parts: vec![Part::ToolResult(ToolResult {
                call_id: "c1".into(),
                content: vec![Content::Text("{\"entities\":3}".into())],
                is_error: false,
            })],
        },
    ];
    let body = lower::body(&request(messages), "m", 0.2);
    assert_eq!(
        body["messages"][3],
        json!({"role": "tool", "name": "opendraft_drawing_summary", "tool_call_id": "c1", "content": "{\"entities\":3}"})
    );
}

#[test]
fn a_picture_follows_the_tool_message_as_a_user_message_naming_the_call() {
    let messages = vec![
        Message::assistant(vec![call("c1", "opendraft_view_render")]),
        Message {
            role: Role::ToolResults,
            parts: vec![Part::ToolResult(ToolResult {
                call_id: "c1".into(),
                content: vec![Content::Text("{}".into()), Content::Image(picture())],
                is_error: false,
            })],
        },
    ];
    let body = lower::body(&request(messages), "m", 0.2);
    let tool = &body["messages"][2];
    assert_eq!(tool["role"], "tool");
    assert_eq!(tool["content"], "{}", "no picture inside the tool message");
    let user = &body["messages"][3];
    assert_eq!(user["role"], "user");
    assert!(user["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("call c1"));
    assert_eq!(user["content"][1]["type"], "image_url");
    assert_eq!(
        user["content"][1]["image_url"],
        "data:image/png;base64,AAAA"
    );
}

#[test]
fn a_user_message_with_a_picture_is_a_list_of_parts() {
    let message = Message {
        role: Role::User,
        parts: vec![Part::Text("look".into()), Part::Image(picture())],
    };
    let body = lower::body(&request(vec![message]), "m", 0.2);
    assert_eq!(
        body["messages"][1]["content"][0],
        json!({"type": "text", "text": "look"})
    );
    assert_eq!(body["messages"][1]["content"][1]["type"], "image_url");
}

fn answer(message: Value, finish: &str) -> Value {
    json!({"choices": [{"index": 0, "message": message, "finish_reason": finish}],
           "usage": {"prompt_tokens": 11, "completion_tokens": 7, "total_tokens": 18}})
}

#[test]
fn a_text_answer_is_raised_with_its_usage() {
    let r = raise::response(&answer(
        json!({"role": "assistant", "content": "Done."}),
        "stop",
    ))
    .unwrap();
    assert_eq!(r.message.text(), "Done.");
    assert_eq!(r.stop, StopReason::EndTurn);
    assert_eq!((r.usage.input, r.usage.output), (Some(11), Some(7)));
}

#[test]
fn a_tool_call_answer_has_its_arguments_parsed() {
    let r = raise::response(&answer(
        json!({"role": "assistant", "content": "", "tool_calls": [
            {"id": "D681PevKs", "type": "function",
             "function": {"name": "opendraft_drawing_summary", "arguments": "{\"document_id\": \"d1\"}"}}]}),
        "tool_calls",
    ))
    .unwrap();
    assert_eq!(r.stop, StopReason::ToolUse);
    let calls = r.message.tool_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id, "D681PevKs");
    assert_eq!(calls[0].arguments, json!({"document_id": "d1"}));
    assert_eq!(r.message.text(), "");
}

#[test]
fn arguments_that_are_already_an_object_or_empty_are_accepted() {
    let r = raise::response(&answer(
        json!({"content": null, "tool_calls": [
            {"id": "a", "function": {"name": "x", "arguments": {"k": 1}}},
            {"id": "b", "function": {"name": "y", "arguments": ""}},
            {"id": "c", "function": {"name": "z"}}]}),
        "tool_calls",
    ))
    .unwrap();
    let calls = r.message.tool_calls();
    assert_eq!(calls[0].arguments, json!({"k": 1}));
    assert_eq!(calls[1].arguments, json!({}));
    assert_eq!(calls[2].arguments, json!({}));
}

#[test]
fn unreadable_arguments_stay_text_so_the_loop_can_refuse_them() {
    let r = raise::response(&answer(
        json!({"tool_calls": [{"id": "a", "function": {"name": "x", "arguments": "{\"k\": "}}]}),
        "tool_calls",
    ))
    .unwrap();
    assert_eq!(r.message.tool_calls()[0].arguments, json!("{\"k\": "));
}

#[test]
fn text_may_arrive_as_chunks() {
    let r = raise::response(&answer(
        json!({"content": [{"type": "text", "text": "One "}, {"type": "reference", "reference_ids": [1]}, {"type": "text", "text": "two"}]}),
        "stop",
    ))
    .unwrap();
    assert_eq!(r.message.text(), "One two");
}

#[test]
fn a_call_with_no_id_gets_one() {
    let r = raise::response(&answer(
        json!({"tool_calls": [{"function": {"name": "x", "arguments": "{}"}}]}),
        "tool_calls",
    ))
    .unwrap();
    assert_eq!(r.message.tool_calls()[0].id, "call0");
}

#[test]
fn finish_reasons_map_and_unknown_ones_are_kept() {
    let reason = |f: &str| {
        raise::response(&answer(json!({"content": "x"}), f))
            .unwrap()
            .stop
    };
    assert_eq!(reason("stop"), StopReason::EndTurn);
    assert_eq!(reason("length"), StopReason::MaxTokens);
    assert_eq!(reason("model_length"), StopReason::MaxTokens);
    assert_eq!(reason("error"), StopReason::Other("error".into()));
}

#[test]
fn an_answer_with_no_choice_is_a_fatal_failure() {
    for body in [json!({}), json!({"choices": []}), json!({"choices": [{}]})] {
        assert!(
            matches!(raise::response(&body), Err(ProviderError::Fatal(_))),
            "{body}"
        );
    }
}
