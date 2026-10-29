//! Calls the real Mistral service, so it is ignored unless asked for:
//!
//!   cargo test -p od-agent --test mistral_live -- --ignored --nocapture
//!
//! with the key in `MISTRAL_API_KEY` or in `~/.opendraft/test.env`.
//!
//! It costs a few cents. It keeps each raw answer in
//! `target/mistral-recorded/` (or `MISTRAL_RECORD_DIR`) so they can be turned
//! into fixtures; those hold what the model said about a made-up drawing and
//! never the key. These tests settle what the documentation leaves open: the
//! shape of a tool call as sent back, whether a picture in a user message is
//! accepted, and the finish reasons and usage fields in real answers.

use std::path::PathBuf;
use std::sync::Arc;

use od_agent::{
    qa_review, run_task, Answer, CancelToken, Host, Image, Message, MistralProvider, ModelRequest,
    Part, Plan, PlanOutcome, Provider, Question, Role, TaskConfig, TaskState, ToolCall,
    ToolOutcome,
};
use serde_json::json;

/// The key: `MISTRAL_API_KEY` if set, else the file named by
/// `OPENDRAFT_ENV_FILE`, else `~/.opendraft/test.env` (or `text.env`), which
/// lives outside the repository. In a file, a line `MISTRAL_API_KEY=...`
/// (quotes and `export` allowed); failing that, a file whose only line is the
/// key itself. The key is never printed.
fn key() -> String {
    if let Some(key) = std::env::var("MISTRAL_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
    {
        return key;
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
    let candidates: Vec<PathBuf> = std::env::var_os("OPENDRAFT_ENV_FILE")
        .map(PathBuf::from)
        .into_iter()
        .chain(home.into_iter().flat_map(|h| {
            let dir = PathBuf::from(h).join(".opendraft");
            [dir.join("test.env"), dir.join("text.env")]
        }))
        .collect();
    for path in candidates {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Some(key) = key_in(&text) {
                return key;
            }
        }
    }
    panic!("set MISTRAL_API_KEY, or put it in ~/.opendraft/test.env as MISTRAL_API_KEY=...");
}

fn key_in(text: &str) -> Option<String> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let clean = |v: &str| v.trim().trim_matches(|c| c == '"' || c == '\'').to_owned();
    for line in &lines {
        let line = line.strip_prefix("export ").unwrap_or(line);
        if let Some(value) = line.strip_prefix("MISTRAL_API_KEY=") {
            let value = clean(value);
            return (!value.is_empty()).then_some(value);
        }
    }
    match lines.as_slice() {
        [only] if !only.contains('=') && !only.contains(' ') => Some(clean(only)),
        _ => None,
    }
}

fn provider() -> MistralProvider {
    let dir = std::env::var_os("MISTRAL_RECORD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/mistral-recorded")
        });
    MistralProvider::new(&key()).recording_to(dir)
}

/// A drawing that does not exist: every tool answers from a script.
struct Canned;

impl Host for Canned {
    fn call(&self, call: &ToolCall, _cancel: &CancelToken) -> ToolOutcome {
        ToolOutcome::Done(match call.name.as_str() {
            "opendraft_documents_list" => json!({"documents": [
                {"document_id": "doc_1", "title": "Plan.odx", "modified": false, "revision": "4"}]}),
            _ => json!({"entities": 12, "by_kind": {"line": 9, "circle": 3}, "layers": 2}),
        })
    }

    fn propose_plan(&self, _plan: &Plan, _cancel: &CancelToken) -> PlanOutcome {
        PlanOutcome::Cancelled
    }

    fn ask_user(&self, _question: &Question, _cancel: &CancelToken) -> Answer {
        Answer::Cancelled
    }

    fn finish(&self) {}
}

#[test]
#[ignore = "calls the real Mistral service and needs a key"]
fn the_model_looks_at_a_drawing_with_the_tools_and_reports() {
    let report = run_task(
        Arc::new(provider()),
        &Canned,
        &qa_review(),
        "How many entities does the open drawing have, and on how many layers? Use the tools.",
        TaskConfig::new("live1"),
        &CancelToken::new(),
        &mut |event| println!("{event:?}"),
    );
    println!("{:#?}", report.state);
    println!("{}", report.final_text.clone().unwrap_or_default());
    assert_eq!(report.state, TaskState::Completed);
    assert!(report.tool_calls >= 1, "the model should have used a tool");
    assert!(report.final_text.is_some_and(|t| t.contains("12")));
}

#[test]
#[ignore = "calls the real Mistral service and needs a key"]
fn a_picture_in_a_user_message_is_accepted() {
    // A 1x1 red PNG: small, but a real one.
    let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";
    let request = ModelRequest {
        system: "Answer in one short sentence.".to_owned(),
        messages: vec![Message {
            role: Role::User,
            parts: vec![
                Part::Text("What colour is this picture?".to_owned()),
                Part::Image(Image {
                    media_type: "image/png".to_owned(),
                    base64: png.to_owned(),
                    label: "a test picture".to_owned(),
                }),
            ],
        }],
        tools: Vec::new(),
        max_output_tokens: 100,
    };
    let answer = provider().complete(&request, &CancelToken::new());
    println!("{answer:#?}");
    let answer = answer.expect("the service should accept a picture in a user message");
    assert!(!answer.message.text().is_empty());
}
