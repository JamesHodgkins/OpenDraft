//! The chat behind the Assistant panel: its conversations, the one task that
//! may be running, and what the screen should show. All of it lives on the UI
//! thread; the task runs on its own thread (`worker.rs`) and reports through
//! a channel that `poll` drains once a frame.
//!
//! The key is held here, in memory, for the session. It is handed to the
//! provider factory and nowhere else: not logged, not in any entry, and this
//! type has no `Debug` so it cannot be printed by accident.

pub(super) mod words;
mod worker;

use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;

use od_agent::{
    edit, qa_review, CancelToken, Event, Message, MistralProvider, Provider, TaskConfig, Workflow,
};

use super::ApprovalLevel;
use crate::automation::{document_id, AutomationCore};
use crate::state::AppState;
use worker::{start, Job, Update};
pub use worker::{Ask, Pending, Reply, Waker};

/// Makes the model for a task from the session's key, or says why it cannot.
pub type ProviderFactory =
    Arc<dyn Fn(Option<&str>) -> Result<Arc<dyn Provider>, String> + Send + Sync>;

/// The real model: Mistral, once there is a key.
pub fn mistral() -> ProviderFactory {
    Arc::new(|key| match key {
        Some(key) if !key.trim().is_empty() => {
            let provider: Arc<dyn Provider> = Arc::new(MistralProvider::new(key));
            Ok(provider)
        }
        _ => Err("Add your Mistral key to start.".to_owned()),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    Running,
    Done,
    Failed,
}

/// One line of the transcript.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    You(String),
    Said(String),
    /// Something the assistant did, in plain words.
    Did {
        text: String,
        progress: Progress,
    },
    /// Something the program says: a stop, an error, what you answered.
    Notice(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Reads and reports; nothing that changes the drawing is offered.
    Review,
    Edit,
}

struct Running {
    updates: Receiver<Update>,
    cancel: CancelToken,
    /// The tool in hand, for the status line.
    tool: Option<String>,
}

#[derive(Default)]
pub struct Chat {
    pub title: String,
    pub entries: Vec<Entry>,
    /// What it is doing now; empty when idle.
    pub status: String,
    /// A question waiting for the person.
    pub pending: Option<Pending>,
    /// What the model has been told and has said, to continue from.
    messages: Vec<Message>,
    running: Option<Running>,
}

impl Chat {
    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }
}

pub struct Assistant {
    chats: Vec<Chat>,
    current: usize,
    tasks: u64,
    key: Option<String>,
    pub level: ApprovalLevel,
    pub kind: Kind,
    /// The unsent text in the input box.
    pub draft: String,
    /// Whether the list of earlier chats is showing in place of the chat.
    pub history_open: bool,
    factory: ProviderFactory,
    wake: Waker,
}

impl Assistant {
    pub fn new(factory: ProviderFactory, wake: Waker, key: Option<String>) -> Self {
        Self {
            chats: vec![Chat::default()],
            current: 0,
            tasks: 0,
            key: key.filter(|k| !k.trim().is_empty()),
            level: ApprovalLevel::ReviewEach,
            kind: Kind::Review,
            draft: String::new(),
            history_open: false,
            factory,
            wake,
        }
    }

    pub fn chat(&self) -> &Chat {
        &self.chats[self.current]
    }

    pub fn chat_mut(&mut self) -> &mut Chat {
        &mut self.chats[self.current]
    }

    pub fn has_key(&self) -> bool {
        self.key.is_some()
    }

    /// Keep the key for this session only.
    pub fn set_key(&mut self, key: &str) {
        self.key = Some(key.trim().to_owned()).filter(|k| !k.is_empty());
    }

    /// The conversations of this run, newest first, as (index, title).
    pub fn history(&self) -> Vec<(usize, &str)> {
        let mut list: Vec<(usize, &str)> = self
            .chats
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.entries.is_empty())
            .map(|(i, c)| (i, c.title.as_str()))
            .collect();
        list.reverse();
        list
    }

    pub fn current_index(&self) -> usize {
        self.current
    }

    /// Show an earlier conversation, to read or to continue.
    pub fn select(&mut self, index: usize) {
        if index < self.chats.len() {
            self.current = index;
            self.history_open = false;
        }
    }

    /// Start a fresh conversation (the model remembers nothing of the old
    /// one). One that is still working is stopped.
    pub fn new_chat(&mut self) {
        self.stop();
        self.history_open = false;
        if self.chats[self.current].entries.is_empty() {
            return;
        }
        self.chats.push(Chat::default());
        self.current = self.chats.len() - 1;
    }

    /// Stop the task of the conversation on screen, and any question it had
    /// waiting.
    pub fn stop(&mut self) {
        let chat = &mut self.chats[self.current];
        if let Some(running) = &chat.running {
            running.cancel.cancel();
        }
        chat.pending = None;
    }

    /// Send `text` in the conversation on screen. `false` if it could not be
    /// sent (nothing typed, a task still running, no key): the reason, where
    /// there is one, is in the transcript and the text stays in the box.
    pub fn send(&mut self, text: &str, state: &AppState, core: &Arc<AutomationCore>) -> bool {
        let text = text.trim();
        if text.is_empty() || self.chat().is_running() {
            return false;
        }
        let provider = match (self.factory)(self.key.as_deref()) {
            Ok(provider) => provider,
            Err(reason) => {
                self.chat_mut().entries.push(Entry::Notice(reason));
                return false;
            }
        };
        self.tasks += 1;
        let mut config = TaskConfig::new(&format!("chat{}", self.tasks));
        config.history = self.chat().messages.clone();
        let workflow: Workflow = match self.kind {
            Kind::Review => qa_review(),
            Kind::Edit => edit(),
        };
        let cancel = CancelToken::new();
        let job = Job {
            provider,
            core: Arc::clone(core),
            workflow,
            level: self.level,
            prompt: format!("{}\n\n{text}", context(state, core)),
            config,
            cancel: cancel.clone(),
            wake: Arc::clone(&self.wake),
        };
        let chat = &mut self.chats[self.current];
        match start(job) {
            Ok(updates) => {
                if chat.entries.is_empty() {
                    chat.title = text.chars().take(40).collect();
                }
                chat.entries.push(Entry::You(text.to_owned()));
                chat.status = "Thinking...".to_owned();
                chat.running = Some(Running {
                    updates,
                    cancel,
                    tool: None,
                });
                true
            }
            Err(reason) => {
                chat.entries.push(Entry::Notice(reason));
                false
            }
        }
    }

    /// Answer the question on screen.
    pub fn answer(&mut self, reply: Reply) {
        let chat = &mut self.chats[self.current];
        if let Some(pending) = chat.pending.take() {
            chat.entries.push(Entry::Notice(said(&pending.ask, &reply)));
            let _ = pending.reply.send(reply);
        }
    }

    /// Take in what the tasks have reported. Once a frame.
    pub fn poll(&mut self) {
        let mut key_refused = false;
        for chat in &mut self.chats {
            let mut updates = Vec::new();
            let mut gone = false;
            if let Some(running) = &chat.running {
                loop {
                    match running.updates.try_recv() {
                        Ok(update) => updates.push(update),
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            gone = true;
                            break;
                        }
                    }
                }
            }
            for update in updates {
                key_refused |= chat.apply(update);
            }
            if gone && chat.running.is_some() {
                chat.entries.push(Entry::Notice(
                    "The assistant stopped unexpectedly.".to_owned(),
                ));
                chat.running = None;
                chat.pending = None;
                chat.status.clear();
            }
        }
        // A refused key is no use to keep: the panel asks for another.
        if key_refused {
            self.key = None;
        }
    }
}

impl Chat {
    /// Take in one update; `true` if it says the key was refused.
    fn apply(&mut self, update: Update) -> bool {
        match update {
            Update::Event(Event::State(state)) => {
                let tool = self.running.as_ref().and_then(|r| r.tool.as_deref());
                self.status = words::status(&state, tool);
            }
            Update::Event(Event::Said(text)) => self.entries.push(Entry::Said(text)),
            Update::Event(Event::ToolStarted { name }) => {
                self.entries.push(Entry::Did {
                    text: format!("{}...", words::doing(&name)),
                    progress: Progress::Running,
                });
                self.status = format!("{}...", words::doing(&name));
                if let Some(running) = &mut self.running {
                    running.tool = Some(name);
                }
            }
            Update::Event(Event::ToolFinished { name, ok }) => {
                let line = self.entries.iter_mut().rev().find(|e| {
                    matches!(
                        e,
                        Entry::Did {
                            progress: Progress::Running,
                            ..
                        }
                    )
                });
                if let Some(Entry::Did { text, progress }) = line {
                    *text = words::did(&name);
                    *progress = if ok { Progress::Done } else { Progress::Failed };
                }
            }
            Update::Ask(pending) => self.pending = Some(pending),
            Update::Done(report) => {
                self.messages = report.messages;
                if let Some(why) = words::ended(&report.state) {
                    self.entries.push(Entry::Notice(why));
                }
                self.running = None;
                self.pending = None;
                self.status.clear();
                if matches!(&report.state, od_agent::TaskState::Failed(od_agent::Failure::Provider(m)) if m.contains(od_agent::KEY_REFUSED))
                {
                    self.entries.push(Entry::Notice(
                        "That key was refused. Add it again to go on.".to_owned(),
                    ));
                    return true;
                }
            }
        }
        false
    }
}

/// What the model is told about the drawing in view, ahead of the user's
/// words. It is the user's own drawing's name and a wire id, not its content.
fn context(state: &AppState, core: &AutomationCore) -> String {
    if state.home {
        return "The user is on the Home page; no drawing is in view.".to_owned();
    }
    let doc = &state.docs[state.active];
    let title: String = doc
        .title
        .chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect();
    format!(
        "The drawing in view is \"{title}\" (document_id {}).",
        document_id(core.instance_id(), doc.uid)
    )
}

/// What was answered, for the transcript.
fn said(ask: &Ask, reply: &Reply) -> String {
    use od_agent::{Answer, PlanOutcome};

    use crate::assistant::Decision;
    match (ask, reply) {
        (Ask::Change(_), Reply::Change(Decision::Approved)) => {
            "You allowed that change.".to_owned()
        }
        (Ask::Change(_), Reply::Change(_)) => "You declined that change.".to_owned(),
        (Ask::Plan(..), Reply::Plan(PlanOutcome::Approved)) => "You approved the plan.".to_owned(),
        (Ask::Plan(..), Reply::Plan(PlanOutcome::Rejected(Some(why)))) => {
            format!("You did not approve the plan: {why}")
        }
        (Ask::Plan(..), Reply::Plan(_)) => "You did not approve the plan.".to_owned(),
        (Ask::Question(_), Reply::Answer(Answer::Text(text))) => format!("You answered: {text}"),
        _ => "You did not answer.".to_owned(),
    }
}

impl Drop for Assistant {
    fn drop(&mut self) {
        for chat in &self.chats {
            if let Some(running) = &chat.running {
                running.cancel.cancel();
            }
        }
    }
}
