//! The in-app assistant's harness: the loop between a model and the tools
//! of the drawing.
//!
//! This crate knows no model vendor and no drawing. It is given a
//! [`Provider`] (the model) and a [`Host`] (the app), and runs a task:
//! offer the tools of a [`Workflow`], pass the model's calls to the host,
//! shape the answers, and stop at a limit, a failure or the user's Stop.
//! It depends on `od-api` and nothing else of ours (a test in `od-app`
//! holds it to that), so it can reach a drawing only through the operations
//! outside programs use, with the same grants, revisions and limits. It
//! cannot approve a change itself: the host asks the user.

mod budget;
mod dispatch;
mod fake;
mod host;
mod mistral;
mod model;
mod provider;
mod shape;
mod state;
mod task;
mod types;
mod workflow;

#[cfg(test)]
mod tests;

pub use budget::{BudgetKind, Budgets, Meter};
pub use fake::{FakeProvider, Step};
pub use host::{Answer, Host, Plan, PlanOutcome, Question, ToolOutcome};
pub use mistral::{MistralProvider, DEFAULT_MODEL};
pub use provider::{CancelToken, Provider, ProviderError};
pub use state::{Failure, TaskState};
pub use task::{run_task, Event, Report, Retry, TaskConfig};
pub use types::{
    Content, Image, Message, ModelRequest, ModelResponse, Part, Role, StopReason, ToolCall,
    ToolResult, ToolSpec, Usage,
};
pub use workflow::{changes_drawing, edit, qa_review, Workflow, ASK_USER, PROPOSE_PLAN};
