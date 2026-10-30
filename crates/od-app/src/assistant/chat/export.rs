//! A chat as plain text, for pasting where it can be read: what was said, the
//! plan, and every tool call with its arguments and its result, which the
//! panel's own transcript puts in plain words. It is how a bad run is shown
//! to whoever has to find out what went wrong.

use od_agent::{Content, Message, Part, Role};

/// Longest result kept whole; the rest is cut, and the cut is marked.
const RESULT_MAX: usize = 6000;

pub(super) fn text(header: &str, messages: &[Message]) -> String {
    let mut out = String::from(header);
    for message in messages {
        match message.role {
            Role::User => {
                out.push_str("\n--- You\n");
                out.push_str(&message.text());
                out.push('\n');
            }
            Role::Assistant => {
                out.push_str("\n--- Assistant\n");
                let said = message.text();
                if !said.is_empty() {
                    out.push_str(&said);
                    out.push('\n');
                }
                for call in message.tool_calls() {
                    out.push_str(&format!("-> {} {}\n", call.name, call.arguments));
                }
            }
            Role::ToolResults => {
                out.push_str("\n--- Results\n");
                for part in &message.parts {
                    if let Part::ToolResult(result) = part {
                        let name = call_name(messages, &result.call_id);
                        let flag = if result.is_error { " (error)" } else { "" };
                        out.push_str(&format!("<- {name}{flag} {}\n", body(&result.content)));
                    }
                }
            }
        }
    }
    out
}

fn body(content: &[Content]) -> String {
    let mut text = String::new();
    for part in content {
        match part {
            Content::Text(t) => text.push_str(t),
            Content::Image(image) => text.push_str(&format!("[picture: {}]", image.label)),
        }
    }
    if text.len() > RESULT_MAX {
        let mut end = RESULT_MAX;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str(" [cut]");
    }
    text
}

fn call_name(messages: &[Message], call_id: &str) -> String {
    messages
        .iter()
        .flat_map(|m| m.parts.iter())
        .find_map(|p| match p {
            Part::ToolCall(c) if c.id == call_id => Some(c.name.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "a tool".to_owned())
}

#[cfg(test)]
mod tests {
    use od_agent::{Image, ToolCall, ToolResult};
    use serde_json::json;

    use super::*;

    fn conversation() -> Vec<Message> {
        vec![
            Message::user("The drawing in view is X.\n\ndraw a flange"),
            Message::assistant(vec![
                Part::Text("Planning.".to_owned()),
                Part::ToolCall(ToolCall {
                    id: "c1".to_owned(),
                    name: "opendraft_entities_apply_batch".to_owned(),
                    arguments: json!({"operations": []}),
                }),
            ]),
            Message {
                role: Role::ToolResults,
                parts: vec![Part::ToolResult(ToolResult {
                    call_id: "c1".to_owned(),
                    content: vec![
                        Content::Text("{\"changed\":true}".to_owned()),
                        Content::Image(Image {
                            media_type: "image/png".to_owned(),
                            base64: "AAAA".to_owned(),
                            label: "the sheet".to_owned(),
                        }),
                    ],
                    is_error: false,
                })],
            },
        ]
    }

    #[test]
    fn a_chat_reads_in_order_with_calls_arguments_and_results() {
        let said = text("HEADER\n", &conversation());
        let at = |needle: &str| {
            said.find(needle)
                .unwrap_or_else(|| panic!("{needle} in {said}"))
        };
        assert!(said.starts_with("HEADER"));
        assert!(at("--- You") < at("--- Assistant"));
        assert!(at("-> opendraft_entities_apply_batch {\"operations\":[]}") < at("--- Results"));
        assert!(said
            .contains("<- opendraft_entities_apply_batch {\"changed\":true}[picture: the sheet]"));
        assert!(!said.contains("AAAA"), "a picture's data is not pasted");
    }

    #[test]
    fn an_error_result_is_marked() {
        let mut messages = conversation();
        if let Part::ToolResult(r) = &mut messages[2].parts[0] {
            r.is_error = true;
        }
        assert!(text("", &messages).contains("<- opendraft_entities_apply_batch (error)"));
    }

    #[test]
    fn a_long_result_is_cut_and_says_so() {
        let mut messages = conversation();
        if let Part::ToolResult(r) = &mut messages[2].parts[0] {
            r.content = vec![Content::Text("é".repeat(RESULT_MAX))];
        }
        let said = text("", &messages);
        assert!(said.contains("[cut]"));
        assert!(said.len() < RESULT_MAX + 400);
    }
}
