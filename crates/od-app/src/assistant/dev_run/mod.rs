//! Trying the assistant by hand, with no window: a prompt, a drawing file,
//! the real harness, the real host and core, and either a scripted model or
//! Mistral. It is a test, so it can print and read the keyboard, and it
//! cannot reach a release build. Run it with
//!
//!   cargo test -p od-app --bin opendraft assistant::dev_run::dev_run -- --ignored --nocapture
//!
//! and choose with environment variables (all optional):
//!
//!   OD_DEV_FILE      a drawing to open (.odx, .dxf, .dwg); a blank one if unset
//!   OD_DEV_PROMPT    what to ask
//!   OD_DEV_WORKFLOW  `review` (the default, reads only) or `edit`
//!   OD_DEV_LEVEL     `each` (the default), `plan` or `auto`
//!   OD_DEV_PROVIDER  `fake` (a scripted model) or `mistral`; by default
//!                    Mistral if a key is found (see `key.rs`), else fake
//!   OD_DEV_TRANSCRIPT a folder for a text file of every call and result, and
//!                    each picture the model was shown
//!   OD_DEV_SAVE      where to save the drawing afterwards (an edit is made
//!                    in memory only; open this file to see it)
//!
//! You are asked y or n at each prompt. A request to pick objects is
//! cancelled, because there is no canvas to pick on.

mod console;
mod key;
mod transcript;

use std::io::BufRead;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use od_agent::{
    edit, qa_review, run_task, CancelToken, Event, FakeProvider, MistralProvider, Provider, Report,
    Step, TaskConfig, TaskState, Workflow,
};
use serde_json::json;

use super::support::{add_a_line, core, drive};
use super::{AppHost, ApprovalLevel, Reach};
use crate::automation::document_id;
use crate::state::AppState;
use console::{say, Console, Sink};

pub struct DevRun {
    pub file: Option<PathBuf>,
    pub prompt: String,
    pub edit: bool,
    pub level: ApprovalLevel,
    /// A key for Mistral, or the scripted model.
    pub key: Option<String>,
    pub save_to: Option<PathBuf>,
}

pub struct Outcome {
    pub report: Report,
    pub lines: usize,
}

fn open(state: &mut AppState, path: &std::path::Path) -> Result<(), String> {
    let loaded =
        od_files::load(path).map_err(|e| format!("could not open {}: {e}", path.display()))?;
    let title = path.file_name().map_or_else(
        || "drawing".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    let session = od_commands::Session::with_document(loaded.document);
    state.install_document(title, session);
    Ok(())
}

/// What the scripted model does: look, then (for an edit) plan and draw one
/// line, then say it is done.
fn script(edit: bool, core: &crate::automation::AutomationCore, state: &AppState) -> Vec<Step> {
    let doc = document_id(core.instance_id(), state.docs[state.active].uid);
    let mut steps = vec![
        Step::call("c1", "opendraft_documents_list", json!({})),
        Step::call(
            "c2",
            "opendraft_drawing_summary",
            json!({"document_id": doc}),
        ),
    ];
    if edit {
        steps.push(Step::call(
            "c3",
            "propose_plan",
            json!({"summary": "Draw one line", "steps": ["Add a line from 0,0 to 10,0"]}),
        ));
        let mut batch = add_a_line(core, state, 0.0);
        batch["document_id"] = json!(doc);
        batch["operations"][0]["entity"]["geometry"]["end"]["x"] = json!(10.0);
        steps.push(Step::call("c4", "opendraft_entities_apply_batch", batch));
        steps.push(Step::say("I drew the line."));
        steps.push(Step::say("Checked: the line is there."));
    } else {
        steps.push(Step::call(
            "c3",
            "opendraft_drawing_audit",
            json!({"document_id": doc}),
        ));
        steps.push(Step::say("That is what the summary and the audit show."));
    }
    steps
}

pub fn run(settings: DevRun, input: Box<dyn BufRead + Send>, out: Sink) -> Result<Outcome, String> {
    let core = core();
    let mut state = AppState::new();
    if let Some(path) = &settings.file {
        open(&mut state, path)?;
    }
    let console = Arc::new(Console {
        input: Mutex::new(input),
        out: out.clone(),
    });
    let host = AppHost::new(core.clone(), console, settings.level, Reach::All)
        .ok_or("could not connect the assistant")?;
    let workflow: Workflow = if settings.edit { edit() } else { qa_review() };
    let provider: Arc<dyn Provider> = match &settings.key {
        Some(key) => {
            say(&out, "model: Mistral");
            Arc::new(MistralProvider::new(key))
        }
        None => {
            say(&out, "model: scripted (no key found)");
            Arc::new(FakeProvider::new(script(settings.edit, &core, &state)))
        }
    };
    say(
        &out,
        &format!("workflow: {}   level: {:?}", workflow.name, settings.level),
    );
    say(&out, &format!("you: {}\n", settings.prompt));
    let events = out.clone();
    let cancel = CancelToken::new();
    let report = drive(
        &core,
        &mut state,
        |state| {
            // There is no canvas: a prompt to pick objects is turned away.
            if state.command_running() {
                state.cancel_command();
                say(
                    &events,
                    "  (a request to pick objects was cancelled: no canvas here)",
                );
            }
        },
        || {
            run_task(
                provider,
                &host,
                &workflow,
                &settings.prompt,
                TaskConfig::new("dev"),
                &cancel,
                &mut |event| show(&events, &event),
            )
        },
    );
    let lines = state.session().document().len();
    say(&out, &format!("\nended: {:?}", report.state));
    say(
        &out,
        &format!(
            "{} model call(s), {} tool call(s), {} tokens; {lines} object(s) in the drawing",
            report.model_calls, report.tool_calls, report.tokens
        ),
    );
    if let Some(path) = &settings.save_to {
        od_files::save(path, state.session().document()).map_err(|e| e.to_string())?;
        say(&out, &format!("saved to {}", path.display()));
    }
    Ok(Outcome { report, lines })
}

fn show(out: &Sink, event: &Event) {
    match event {
        Event::State(TaskState::Completed | TaskState::Idle) => {}
        Event::State(state) => say(out, &format!("  [{state:?}]")),
        Event::Said(text) => say(out, &format!("assistant: {text}")),
        Event::ToolStarted { name } => say(out, &format!("  -> {name}")),
        Event::ToolFinished { name, ok } => {
            say(
                out,
                &format!("  <- {name} {}", if *ok { "ok" } else { "failed" }),
            );
        }
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

#[test]
#[ignore = "a hand run: reads the keyboard, and calls Mistral if a key is found"]
fn dev_run() {
    let level = match env("OD_DEV_LEVEL").as_deref() {
        Some("plan") => ApprovalLevel::ApprovePlan,
        Some("auto") => ApprovalLevel::AutoApprove { limit: 5 },
        _ => ApprovalLevel::ReviewEach,
    };
    let key = match env("OD_DEV_PROVIDER").as_deref() {
        Some("fake") => None,
        _ => key::find(),
    };
    let settings = DevRun {
        file: env("OD_DEV_FILE").map(PathBuf::from),
        prompt: env("OD_DEV_PROMPT")
            .unwrap_or_else(|| "Summarise this drawing and check it for problems.".to_owned()),
        edit: env("OD_DEV_WORKFLOW").as_deref() == Some("edit"),
        level,
        key,
        save_to: env("OD_DEV_SAVE").map(PathBuf::from),
    };
    let out: Sink = Arc::new(Mutex::new(Box::new(std::io::stdout())));
    let input = Box::new(std::io::BufReader::new(std::io::stdin()));
    match run(settings, input, out) {
        Ok(outcome) => {
            if let Some(folder) = env("OD_DEV_TRANSCRIPT") {
                if let Err(message) =
                    transcript::write(&PathBuf::from(folder), &outcome.report.messages)
                {
                    panic!("{message}");
                }
            }
        }
        Err(message) => panic!("{message}"),
    }
}

#[cfg(test)]
mod tests;
