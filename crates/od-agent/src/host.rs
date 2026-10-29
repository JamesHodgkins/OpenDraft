//! What the harness gets from the app. It cannot see the drawing or ask a
//! user anything itself: it is given a `Host`, and the app implements it.
//!
//! The harness can only ask and be told. In particular it cannot approve
//! anything: when a request needs the user's permission, the host asks the
//! user and, only if they agree, gives the core that one permission before
//! running the request. A harness fault or a fooled model therefore cannot
//! make a change the user did not allow.

use od_api::ApiError;
use serde_json::Value;

use crate::provider::CancelToken;
use crate::types::ToolCall;

/// How a tool call ended.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolOutcome {
    /// It ran; this is the operation's result.
    Done(Value),
    /// The app refused or could not do it. `scope_denied` is here too: the
    /// model is told, and can propose a plan or ask.
    Failed(ApiError),
    /// The user was asked and said no.
    Declined,
    /// The task was cancelled while the call waited.
    Cancelled,
}

/// A plan the model proposes before it changes anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub summary: String,
    /// Each step is one change; the app caps the task at this many.
    pub steps: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanOutcome {
    Approved,
    /// The user did not approve; they may say why.
    Rejected(Option<String>),
    Cancelled,
}

/// A clarifying question from the model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Question {
    pub text: String,
    /// Suggested answers; the user may type their own.
    pub choices: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Text(String),
    Cancelled,
}

pub trait Host {
    /// Run a catalogue tool. Blocks, and may wait for the user. `call` is
    /// already checked against the tools on offer, and carries the retry
    /// key where the tool takes one.
    fn call(&self, call: &ToolCall, cancel: &CancelToken) -> ToolOutcome;

    /// Show the user a plan and wait. On approval the host gives the task
    /// its permission, capped at the plan's steps.
    fn propose_plan(&self, plan: &Plan, cancel: &CancelToken) -> PlanOutcome;

    fn ask_user(&self, question: &Question, cancel: &CancelToken) -> Answer;

    /// The task has ended, however it ended: take back what it was given.
    fn finish(&self);
}
