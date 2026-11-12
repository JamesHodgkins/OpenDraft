//! The harness's own vocabulary. No model vendor's type appears here: an
//! adapter lowers these to a vendor's wire form and raises the answer back,
//! so nothing else in the crate changes when a provider does.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A picture handed to the model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    pub media_type: String,
    pub base64: String,
    /// What it shows, in words (also what a log or a transcript says).
    pub label: String,
}

/// What a tool result is made of.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Content {
    Text(String),
    Image(Image),
}

/// The model asking for a tool. `id` is the provider's, echoed back in the
/// result; `arguments` is a JSON value, whatever the wire form was.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// The answer to one call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: String,
    pub content: Vec<Content>,
    pub is_error: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Part {
    Text(String),
    Image(Image),
    ToolCall(ToolCall),
    ToolResult(ToolResult),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
    /// The answers to the calls in the assistant message before it.
    ToolResults,
}

/// One turn of the conversation. The system prompt is not a message: it
/// travels beside them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub parts: Vec<Part>,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            parts: vec![Part::Text(text.into())],
        }
    }

    pub fn assistant(parts: Vec<Part>) -> Self {
        Self {
            role: Role::Assistant,
            parts,
        }
    }

    /// The text parts joined, for showing and for the final answer.
    pub fn text(&self) -> String {
        let texts: Vec<&str> = self
            .parts
            .iter()
            .filter_map(|p| match p {
                Part::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        texts.join("\n")
    }

    pub fn tool_calls(&self) -> Vec<&ToolCall> {
        self.parts
            .iter()
            .filter_map(|p| match p {
                Part::ToolCall(c) => Some(c),
                _ => None,
            })
            .collect()
    }
}

/// A tool as the model is told of it.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// A JSON Schema object for the arguments.
    pub schema: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelRequest {
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub max_output_tokens: u32,
}

/// Why the model stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    Refused,
    Other(String),
}

/// What a provider reports it used, where it reports it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub input: Option<u64>,
    pub output: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelResponse {
    pub message: Message,
    pub usage: Usage,
    pub stop: StopReason,
}
