//! Mistral's answer as our response. Written to accept what the service
//! sends in more than one form (text as a string or as chunks, arguments as
//! a string or an object) and to refuse nothing it can read.

use serde_json::Value;

use crate::provider::ProviderError;
use crate::types::{Message, ModelResponse, Part, StopReason, ToolCall, Usage};

pub fn response(body: &Value) -> Result<ModelResponse, ProviderError> {
    let choice = body
        .get("choices")
        .and_then(|c| c.get(0))
        .ok_or_else(|| ProviderError::Fatal("the model service sent no answer".to_owned()))?;
    let message = choice
        .get("message")
        .ok_or_else(|| ProviderError::Fatal("the model service sent no message".to_owned()))?;
    let mut parts = Vec::new();
    let text = text_of(message.get("content"));
    if !text.is_empty() {
        parts.push(Part::Text(text));
    }
    if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
        for (n, call) in calls.iter().enumerate() {
            parts.push(Part::ToolCall(tool_call(call, n)));
        }
    }
    Ok(ModelResponse {
        message: Message::assistant(parts),
        usage: Usage {
            input: body["usage"]["prompt_tokens"].as_u64(),
            output: body["usage"]["completion_tokens"].as_u64(),
        },
        stop: stop(choice.get("finish_reason").and_then(Value::as_str)),
    })
}

/// A string, or a list of chunks of which the text ones count.
fn text_of(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(chunks)) => chunks
            .iter()
            .filter_map(|chunk| match chunk {
                Value::String(text) => Some(text.as_str()),
                chunk if chunk["type"] == "text" => chunk["text"].as_str(),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn tool_call(call: &Value, position: usize) -> ToolCall {
    let function = &call["function"];
    let arguments = match &function["arguments"] {
        Value::String(raw) if raw.trim().is_empty() => Value::Object(Default::default()),
        // Unreadable arguments are kept as the text they were, so the loop
        // tells the model they were not an object rather than guessing.
        Value::String(raw) => {
            serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.clone()))
        }
        Value::Null => Value::Object(Default::default()),
        other => other.clone(),
    };
    ToolCall {
        id: call["id"]
            .as_str()
            .map_or_else(|| format!("call{position}"), str::to_owned),
        name: function["name"].as_str().unwrap_or_default().to_owned(),
        arguments,
    }
}

fn stop(reason: Option<&str>) -> StopReason {
    match reason {
        Some("stop") => StopReason::EndTurn,
        Some("tool_calls") => StopReason::ToolUse,
        Some("length" | "model_length") => StopReason::MaxTokens,
        Some(other) => StopReason::Other(other.to_owned()),
        None => StopReason::Other("no finish reason".to_owned()),
    }
}
