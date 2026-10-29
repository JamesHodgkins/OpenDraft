//! The chat behind the panel, driven the way the app drives it: a frame loop
//! that answers the core's queue and polls the chat, and a test that plays
//! the person at the screen.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use od_agent::{Answer, FakeProvider, PlanOutcome, Provider, ProviderError, Step};
use serde_json::json;

use super::support::{add_a_line, core};
use super::{ApprovalLevel, Ask, Assistant, Decision, Entry, Kind, Progress, Reply};
use crate::automation::AutomationCore;
use crate::state::AppState;

/// A chat whose model plays the next script each time one is asked for, and
/// remembers the models it made so a test can read what they were sent.
pub(super) struct Rig {
    pub(super) chat: Assistant,
    scripts: Arc<Mutex<VecDeque<Vec<Step>>>>,
    made: Arc<Mutex<Vec<Arc<FakeProvider>>>>,
    pub(super) core: Arc<AutomationCore>,
    pub(super) state: AppState,
}

pub(super) fn rig(key: Option<&str>) -> Rig {
    let scripts: Arc<Mutex<VecDeque<Vec<Step>>>> = Arc::default();
    let made: Arc<Mutex<Vec<Arc<FakeProvider>>>> = Arc::default();
    let (s, m) = (scripts.clone(), made.clone());
    let factory: super::chat::ProviderFactory = Arc::new(move |key| {
        key.ok_or_else(|| "Add your Mistral key to start.".to_owned())?;
        let steps = s
            .lock()
            .ok()
            .and_then(|mut q| q.pop_front())
            .unwrap_or_default();
        let provider = Arc::new(FakeProvider::new(steps));
        if let Ok(mut made) = m.lock() {
            made.push(provider.clone());
        }
        let provider: Arc<dyn Provider> = provider;
        Ok(provider)
    });
    Rig {
        chat: Assistant::new(factory, Arc::new(|| {}), key.map(str::to_owned)),
        scripts,
        made,
        core: core(),
        state: AppState::new(),
    }
}

impl Rig {
    pub(super) fn script(&self, steps: Vec<Step>) {
        if let Ok(mut q) = self.scripts.lock() {
            q.push_back(steps);
        }
    }

    pub(super) fn send(&mut self, text: &str) -> bool {
        self.chat.send(text, &self.state, &self.core)
    }

    /// Run frames until `done` says so (or five seconds pass).
    pub(super) fn pump(&mut self, mut done: impl FnMut(&mut Assistant) -> bool) {
        let registry = crate::commands::build_registry();
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            self.core.poll(&mut self.state, &registry);
            self.chat.poll();
            if done(&mut self.chat) {
                return;
            }
            assert!(Instant::now() < until, "the chat did not settle");
            thread::sleep(Duration::from_millis(1));
        }
    }

    pub(super) fn finish(&mut self) {
        self.pump(|chat| !chat.chat().is_running());
    }

    pub(super) fn entries(&self) -> Vec<Entry> {
        self.chat.chat().entries.clone()
    }

    pub(super) fn requests(&self, task: usize) -> Vec<od_agent::ModelRequest> {
        self.made
            .lock()
            .map(|m| m[task].requests())
            .unwrap_or_default()
    }
}

fn look_and_say(text: &str) -> Vec<Step> {
    vec![
        Step::call(
            "c1",
            "opendraft_drawing_summary",
            json!({"document_id": "x"}),
        ),
        Step::say(text),
    ]
}

#[test]
fn a_message_runs_a_task_and_the_transcript_shows_what_happened() {
    let mut rig = rig(Some("k"));
    rig.script(look_and_say("Nothing wrong."));
    assert!(rig.send("check it"));
    assert!(rig.chat.chat().is_running());
    rig.finish();
    let entries = rig.entries();
    assert_eq!(entries[0], Entry::You("check it".to_owned()));
    assert!(matches!(
        &entries[1],
        Entry::Did {
            progress: Progress::Failed | Progress::Done,
            ..
        }
    ));
    assert_eq!(
        entries.last(),
        Some(&Entry::Said("Nothing wrong.".to_owned()))
    );
    assert!(rig.chat.chat().status.is_empty());
    assert_eq!(rig.chat.chat().title, "check it");
}

#[test]
fn a_finished_tool_reads_in_the_past_tense() {
    let mut rig = rig(Some("k"));
    rig.script(vec![
        Step::call("c1", "opendraft_documents_list", json!({})),
        Step::say("One drawing."),
    ]);
    rig.send("what is open");
    rig.finish();
    assert!(rig.entries().contains(&Entry::Did {
        text: "Looked at the open drawings".to_owned(),
        progress: Progress::Done,
    }));
}

#[test]
fn the_model_is_told_which_drawing_is_in_view() {
    let mut rig = rig(Some("k"));
    rig.script(vec![Step::say("ok")]);
    rig.send("hello");
    rig.finish();
    let first = rig.requests(0)[0].messages[0].text();
    let id = crate::automation::document_id(rig.core.instance_id(), rig.state.docs[0].uid);
    assert!(first.contains(&id), "{first}");
    assert!(
        first.ends_with("hello"),
        "the user's words come last: {first}"
    );
}

#[test]
fn a_second_message_continues_the_same_conversation() {
    let mut rig = rig(Some("k"));
    rig.script(vec![Step::say("First answer.")]);
    rig.script(vec![Step::say("Second answer.")]);
    rig.send("one");
    rig.finish();
    rig.send("two");
    rig.finish();
    let messages = rig.requests(1)[0].messages.clone();
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[1].text(), "First answer.");
    assert!(messages[2].text().ends_with("two"));
}

#[test]
fn a_new_chat_remembers_nothing_and_the_old_one_stays_in_history() {
    let mut rig = rig(Some("k"));
    rig.script(vec![Step::say("A.")]);
    rig.script(vec![Step::say("B.")]);
    rig.send("first chat");
    rig.finish();
    rig.chat.new_chat();
    assert!(rig.chat.chat().entries.is_empty());
    rig.send("second chat");
    rig.finish();
    assert_eq!(
        rig.requests(1)[0].messages.len(),
        1,
        "no memory of the first"
    );
    let history: Vec<&str> = rig.chat.history().into_iter().map(|(_, t)| t).collect();
    assert_eq!(history, ["second chat", "first chat"]);
    rig.chat.select(0);
    assert_eq!(
        rig.chat.chat().entries[0],
        Entry::You("first chat".to_owned())
    );
}

#[test]
fn an_empty_new_chat_does_not_pile_up() {
    let mut rig = rig(Some("k"));
    rig.chat.new_chat();
    rig.chat.new_chat();
    assert_eq!(rig.chat.history().len(), 0);
    assert_eq!(rig.chat.current_index(), 0);
}

#[test]
fn without_a_key_nothing_is_sent_and_the_reason_is_shown() {
    let mut rig = rig(None);
    assert!(!rig.send("hello"));
    assert!(!rig.chat.chat().is_running());
    assert_eq!(
        rig.entries(),
        [Entry::Notice("Add your Mistral key to start.".to_owned())]
    );
    rig.chat.set_key("sk-x");
    rig.script(vec![Step::say("hi")]);
    assert!(rig.send("hello"));
    rig.finish();
}

#[test]
fn nothing_is_sent_for_empty_text_or_while_a_task_runs() {
    let mut rig = rig(Some("k"));
    assert!(!rig.send("   "));
    rig.script(vec![Step::Hang]);
    assert!(rig.send("go"));
    assert!(!rig.send("again"));
    rig.chat.stop();
    rig.finish();
}

#[test]
fn stop_ends_the_task_and_says_so() {
    let mut rig = rig(Some("k"));
    rig.script(vec![Step::Hang]);
    rig.send("go");
    rig.pump(|chat| chat.chat().status == "Thinking...");
    rig.chat.stop();
    rig.finish();
    assert_eq!(
        rig.entries().last(),
        Some(&Entry::Notice("Stopped.".to_owned()))
    );
}

#[test]
fn a_failure_is_explained_in_plain_words() {
    let mut rig = rig(Some("k"));
    rig.script(vec![Step::Fail(ProviderError::Fatal("bad key".into()))]);
    rig.send("go");
    rig.finish();
    assert_eq!(
        rig.entries().last(),
        Some(&Entry::Notice(
            "The model service failed: bad key".to_owned()
        ))
    );
}

#[test]
fn the_status_line_names_what_the_assistant_is_doing() {
    let mut rig = rig(Some("k"));
    rig.script(vec![
        Step::call("c1", "opendraft_drawing_audit", json!({"document_id": "x"})),
        Step::say("done"),
    ]);
    rig.send("audit");
    let mut seen = Vec::new();
    rig.pump(|chat| {
        let status = chat.chat().status.clone();
        if !status.is_empty() && seen.last() != Some(&status) {
            seen.push(status);
        }
        !chat.chat().is_running()
    });
    assert!(seen.contains(&"Thinking...".to_owned()), "{seen:?}");
}

pub(super) fn edit_rig(level: ApprovalLevel) -> Rig {
    let mut rig = rig(Some("k"));
    rig.chat.kind = Kind::Edit;
    rig.chat.level = level;
    rig
}

pub(super) fn plan_then_draw(rig: &Rig) -> Vec<Step> {
    vec![
        Step::call(
            "p",
            "propose_plan",
            json!({"summary": "Add a line", "steps": ["Add a line"]}),
        ),
        Step::call(
            "c",
            "opendraft_entities_apply_batch",
            add_a_line(&rig.core, &rig.state, 0.0),
        ),
        Step::say("Added."),
        Step::say("Checked."),
    ]
}

#[test]
fn a_plan_is_put_to_the_person_and_their_yes_lets_the_change_through() {
    let mut rig = edit_rig(ApprovalLevel::ApprovePlan);
    let steps = plan_then_draw(&rig);
    rig.script(steps);
    rig.send("draw a line");
    rig.pump(|chat| chat.chat().pending.is_some());
    let Some(Ask::Plan(plan, level)) = rig.chat.chat().pending.as_ref().map(|p| p.ask.clone())
    else {
        panic!("expected a plan to approve");
    };
    assert_eq!(plan.steps, ["Add a line"]);
    assert_eq!(level, ApprovalLevel::ApprovePlan);
    assert_eq!(rig.state.session().document().len(), 0, "nothing yet");
    rig.chat.answer(Reply::Plan(PlanOutcome::Approved));
    rig.finish();
    assert_eq!(rig.state.session().document().len(), 1);
    assert!(rig
        .entries()
        .contains(&Entry::Notice("You approved the plan.".to_owned())));
}

#[test]
fn a_declined_change_draws_nothing() {
    let mut rig = edit_rig(ApprovalLevel::ReviewEach);
    let steps = plan_then_draw(&rig);
    rig.script(steps);
    rig.send("draw a line");
    rig.pump(|chat| chat.chat().pending.is_some());
    rig.chat.answer(Reply::Plan(PlanOutcome::Approved));
    rig.pump(|chat| {
        matches!(
            chat.chat().pending.as_ref().map(|p| &p.ask),
            Some(Ask::Change(_))
        )
    });
    rig.chat.answer(Reply::Change(Decision::Declined));
    rig.finish();
    assert_eq!(rig.state.session().document().len(), 0);
    assert!(rig
        .entries()
        .contains(&Entry::Notice("You declined that change.".to_owned())));
}

#[test]
fn a_question_from_the_model_goes_to_the_person_and_back() {
    let mut rig = edit_rig(ApprovalLevel::ReviewEach);
    rig.script(vec![
        Step::call("q", "ask_user", json!({"question": "Which wall?"})),
        Step::say("The left wall, then."),
    ]);
    rig.send("check a wall");
    rig.pump(|chat| {
        matches!(
            chat.chat().pending.as_ref().map(|p| &p.ask),
            Some(Ask::Question(_))
        )
    });
    assert_eq!(rig.chat.chat().status, "Waiting for your answer");
    rig.chat
        .answer(Reply::Answer(Answer::Text("left".to_owned())));
    rig.finish();
    assert!(rig.requests(0)[1]
        .messages
        .iter()
        .any(|m| format!("{m:?}").contains("left")));
}

#[test]
fn stop_while_a_question_is_waiting_withdraws_it_and_ends_the_task() {
    let mut rig = edit_rig(ApprovalLevel::ReviewEach);
    let steps = plan_then_draw(&rig);
    rig.script(steps);
    rig.send("draw");
    rig.pump(|chat| chat.chat().pending.is_some());
    rig.chat.stop();
    assert!(rig.chat.chat().pending.is_none());
    rig.finish();
    assert_eq!(rig.state.session().document().len(), 0);
    assert_eq!(
        rig.entries().last(),
        Some(&Entry::Notice("Stopped.".to_owned()))
    );
}

#[test]
fn a_task_that_runs_out_of_its_budget_says_what_ran_out() {
    use od_agent::{BudgetKind, Failure, TaskState};
    let said =
        super::chat::words::ended(&TaskState::Failed(Failure::Budget(BudgetKind::ToolCalls)));
    assert_eq!(
        said.as_deref(),
        Some("Stopped: it reached the limit on tool calls.")
    );
    assert_eq!(super::chat::words::ended(&TaskState::Completed), None);
}

#[test]
fn a_refused_key_is_dropped_so_the_panel_asks_for_another() {
    let mut rig = rig(Some("sk-bad"));
    rig.script(vec![Step::Fail(ProviderError::Fatal(format!(
        "{} (status 401)",
        od_agent::KEY_REFUSED
    )))]);
    rig.send("hello");
    rig.finish();
    assert!(!rig.chat.has_key());
    assert!(rig.entries().contains(&Entry::Notice(
        "That key was refused. Add it again to go on.".to_owned()
    )));
}

#[test]
fn another_failure_keeps_the_key() {
    let mut rig = rig(Some("sk-ok"));
    rig.script(vec![Step::Fail(ProviderError::Fatal("bad request".into()))]);
    rig.send("hello");
    rig.finish();
    assert!(rig.chat.has_key());
}
