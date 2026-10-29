//! How the user is asked. The assistant's thread blocks in these while the
//! user answers, so each takes the task's cancel token and gives up when it
//! is set. The panel will implement it; the tests script it.

use od_agent::{Answer, CancelToken, Plan, PlanOutcome, Question};
use serde_json::Value;

use super::ApprovalLevel;

/// A change the assistant wants to make, exactly as it would be sent.
#[derive(Clone, Debug, PartialEq)]
pub struct ChangeRequest {
    pub tool: String,
    pub operation: String,
    pub arguments: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Approved,
    Declined,
    /// The task was stopped while the question was up.
    Cancelled,
}

pub trait Prompter: Send + Sync {
    /// Show this exact request and ask whether it may happen once.
    fn approve_change(&self, request: &ChangeRequest, cancel: &CancelToken) -> Decision;

    /// Show a plan. At `ApprovePlan` a yes also allows as many changes as it
    /// has steps; at `ReviewEach` it only lets the work go on.
    fn approve_plan(&self, plan: &Plan, level: ApprovalLevel, cancel: &CancelToken) -> PlanOutcome;

    /// A question from the model: a choice, a number or words.
    fn ask(&self, question: &Question, cancel: &CancelToken) -> Answer;
}
