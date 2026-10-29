//! Our request in Mistral's chat-completions form. Nothing here is a Mistral
//! type: the body is built as JSON from our own types, so the rest of the
//! crate never sees the vendor's shapes.

use serde_json::{json, Value};

use crate::types::{Content, Image, Message, ModelRequest, Part, Role, ToolSpec};

/// The request body.
pub fn body(request: &ModelRequest, model: &str, temperature: f64) -> Value {
    let mut messages = vec![json!({"role": "system", "content": request.system})];
    for message in &request.messages {
        lower_message(message, &request.messages, &mut messages);
    }
    let mut body = json!({
        "model": model,
        "messages": messages,
        "max_tokens": request.max_output_tokens,
        "temperature": temperature,
    });
    if !request.tools.is_empty() {
        body["tools"] = Value::Array(request.tools.iter().map(tool).collect());
        body["tool_choice"] = json!("auto");
    }
    body
}

fn tool(spec: &ToolSpec) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": spec.name,
            "description": spec.description,
            "parameters": spec.schema,
        }
    })
}

fn image_part(image: &Image) -> Value {
    json!({
        "type": "image_url",
        "image_url": format!("data:{};base64,{}", image.media_type, image.base64),
    })
}

fn lower_message(message: &Message, all: &[Message], out: &mut Vec<Value>) {
    match message.role {
        Role::User => out.push(json!({"role": "user", "content": user_content(&message.parts)})),
        Role::Assistant => out.push(assistant(&message.parts)),
        Role::ToolResults => tool_results(&message.parts, all, out),
    }
}

/// A plain string when there is only text, parts when there is a picture.
fn user_content(parts: &[Part]) -> Value {
    let has_image = parts.iter().any(|p| matches!(p, Part::Image(_)));
    if !has_image {
        return Value::String(texts(parts));
    }
    Value::Array(
        parts
            .iter()
            .filter_map(|p| match p {
                Part::Text(t) => Some(json!({"type": "text", "text": t})),
                Part::Image(i) => Some(image_part(i)),
                _ => None,
            })
            .collect(),
    )
}

fn texts(parts: &[Part]) -> String {
    let texts: Vec<&str> = parts
        .iter()
        .filter_map(|p| match p {
            Part::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    texts.join("\n")
}

fn assistant(parts: &[Part]) -> Value {
    let calls: Vec<Value> = parts
        .iter()
        .filter_map(|p| match p {
            Part::ToolCall(c) => Some(json!({
                "id": c.id,
                "type": "function",
                // The wire form is a JSON string, not an object.
                "function": {"name": c.name, "arguments": c.arguments.to_string()},
            })),
            _ => None,
        })
        .collect();
    let mut message = json!({"role": "assistant", "content": texts(parts)});
    if !calls.is_empty() {
        message["tool_calls"] = Value::Array(calls);
    }
    message
}

/// Each answer is a `tool` message with text. A picture is not put in it
/// (their documentation shows tool messages carrying text only): it follows
/// as a user message saying which call it answers.
fn tool_results(parts: &[Part], all: &[Message], out: &mut Vec<Value>) {
    let mut pictures: Vec<Value> = Vec::new();
    for part in parts {
        let Part::ToolResult(result) = part else {
            continue;
        };
        let text: Vec<&str> = result
            .content
            .iter()
            .filter_map(|c| match c {
                Content::Text(t) => Some(t.as_str()),
                Content::Image(_) => None,
            })
            .collect();
        out.push(json!({
            "role": "tool",
            "name": call_name(all, &result.call_id),
            "tool_call_id": result.call_id,
            "content": text.join("\n"),
        }));
        for content in &result.content {
            if let Content::Image(image) = content {
                pictures.push(json!({
                    "type": "text",
                    "text": format!(
                        "The picture requested in call {}: {}",
                        result.call_id, image.label
                    ),
                }));
                pictures.push(image_part(image));
            }
        }
    }
    if !pictures.is_empty() {
        out.push(json!({"role": "user", "content": pictures}));
    }
}

/// The name of the tool a call id belongs to, from the conversation.
fn call_name(all: &[Message], call_id: &str) -> String {
    all.iter()
        .flat_map(|m| m.parts.iter())
        .find_map(|p| match p {
            Part::ToolCall(c) if c.id == call_id => Some(c.name.clone()),
            _ => None,
        })
        .unwrap_or_default()
}
