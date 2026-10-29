//! One task: a user's request, the loop between the model and the tools until
//! the model is done or something stops it, and the report of how it ended.
//!
//! The loop decides nothing about the drawing. Every look and every change
//! goes through the `Host`, and so through the checks every outside program
//! meets; what the loop adds are the limits, the order (plan before change),
//! and cancel.

use std::sync::Arc;
use std::time::Duration;

use crate::budget::{Budgets, Meter};
use crate::host::Host;
use crate::provider::{CancelToken, Provider};
use crate::state::{Failure, TaskState};
use crate::types::{Message, ModelRequest, Part, StopReason};
use crate::workflow::Workflow;

/// Tries at a model call that fails in a way worth trying again.
#[derive(Clone, Debug)]
pub struct Retry {
    pub tries: u32,
    /// The wait before the second try; it doubles after each.
    pub base: Duration,
}

impl Default for Retry {
    fn default() -> Self {
        Self {
            tries: 3,
            base: Duration::from_secs(1),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TaskConfig {
    /// Names the task in the retry keys of its changes.
    pub task_id: String,
    pub budgets: Budgets,
    pub retry: Retry,
}

impl TaskConfig {
    pub fn new(task_id: &str) -> Self {
        Self {
            task_id: task_id.to_owned(),
            budgets: Budgets::default(),
            retry: Retry::default(),
        }
    }
}

/// What a screen needs to follow a task as it runs.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    State(TaskState),
    /// The model said something.
    Said(String),
    ToolStarted {
        name: String,
    },
    ToolFinished {
        name: String,
        ok: bool,
    },
}

/// How a task ended, with the conversation kept.
#[derive(Clone, Debug)]
pub struct Report {
    pub state: TaskState,
    pub messages: Vec<Message>,
    /// The model's last words, when the task completed.
    pub final_text: Option<String>,
    pub model_calls: u32,
    pub tool_calls: u32,
    pub tokens: u64,
}

pub(crate) struct Task<'a> {
    pub provider: Arc<dyn Provider>,
    pub host: &'a dyn Host,
    pub workflow: &'a Workflow,
    pub cancel: &'a CancelToken,
    pub config: TaskConfig,
    pub meter: Meter,
    pub messages: Vec<Message>,
    pub state: TaskState,
    pub on_event: &'a mut dyn FnMut(Event),
    /// Whether changes to the drawing are on offer (a plan was approved).
    pub writes_allowed: bool,
    /// Whether the task has changed the drawing.
    pub wrote: bool,
    pub verified: bool,
    pub final_text: Option<String>,
}

/// Run a task to its end. The host is told when it is over, whatever way it
/// ended, so what the task was given is taken back.
pub fn run_task(
    provider: Arc<dyn Provider>,
    host: &dyn Host,
    workflow: &Workflow,
    prompt: &str,
    config: TaskConfig,
    cancel: &CancelToken,
    on_event: &mut dyn FnMut(Event),
) -> Report {
    let mut task = Task {
        provider,
        host,
        workflow,
        cancel,
        meter: Meter::new(config.budgets.clone()),
        config,
        messages: vec![Message::user(prompt)],
        state: TaskState::Idle,
        on_event,
        writes_allowed: !workflow.plan_first,
        wrote: false,
        verified: false,
        final_text: None,
    };
    task.drive();
    host.finish();
    Report {
        state: task.state,
        messages: task.messages,
        final_text: task.final_text,
        model_calls: task.meter.model_calls(),
        tool_calls: task.meter.tool_calls(),
        tokens: task.meter.tokens(),
    }
}

impl Task<'_> {
    /// Move to `next` if the table allows it; a move it does not list is a
    /// fault, and ends the task.
    pub(crate) fn go(&mut self, next: TaskState) {
        if self.state.is_finished() || self.state == next {
            return;
        }
        let next = if self.state.may_become(&next) {
            next
        } else {
            TaskState::Failed(Failure::Internal(format!(
                "moved from {:?} to {:?}",
                self.state, next
            )))
        };
        self.state = next.clone();
        (self.on_event)(Event::State(next));
    }

    pub(crate) fn end(&mut self, state: TaskState) {
        self.go(state);
    }

    fn drive(&mut self) {
        if self.workflow.plan_first {
            self.go(TaskState::Planning);
        }
        while !self.state.is_finished() {
            if self.cancel.is_cancelled() {
                self.end(TaskState::Cancelled);
                return;
            }
            if self.state != TaskState::Verifying {
                self.go(TaskState::CallingModel);
            }
            let request = ModelRequest {
                system: self.workflow.system.clone(),
                messages: self.messages.clone(),
                tools: self.workflow.offered(self.writes_allowed),
                max_output_tokens: self.config.budgets.max_output_tokens,
            };
            let response = match self.ask_model(&request) {
                Ok(response) => response,
                Err(end) => {
                    self.end(end);
                    return;
                }
            };
            self.meter.add_usage(response.usage);
            let message = response.message;
            let text = message.text();
            let has_calls = !message.tool_calls().is_empty();
            self.messages.push(message.clone());
            if !text.is_empty() {
                (self.on_event)(Event::Said(text.clone()));
            }
            match response.stop {
                StopReason::Refused => {
                    self.end(TaskState::Failed(Failure::Refused(
                        "the model declined to answer".to_owned(),
                    )));
                    return;
                }
                StopReason::MaxTokens => {
                    self.end(TaskState::Failed(Failure::Model(
                        "the answer was cut short".to_owned(),
                    )));
                    return;
                }
                StopReason::Other(why) if !has_calls => {
                    self.end(TaskState::Failed(Failure::Model(format!(
                        "the model stopped for a reason this program does not know: {why}"
                    ))));
                    return;
                }
                _ => {}
            }
            if has_calls {
                self.run_calls(&message);
                continue;
            }
            match self.workflow.verify {
                Some(prompt) if self.wrote && !self.verified => {
                    self.verified = true;
                    self.messages.push(Message::user(prompt));
                    self.go(TaskState::Verifying);
                }
                _ => {
                    self.final_text = Some(text);
                    self.end(TaskState::Completed);
                }
            }
        }
    }

    /// The answers to a turn's calls, as one message.
    pub(crate) fn push_results(&mut self, results: Vec<Part>) {
        self.messages.push(Message {
            role: crate::types::Role::ToolResults,
            parts: results,
        });
    }
}
