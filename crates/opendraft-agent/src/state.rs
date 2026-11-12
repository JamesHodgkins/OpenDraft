//! What a task is doing now, and which moves between those are allowed. The
//! loop asks `TaskState::may_become` before every move; a move the table
//! does not list is a fault in the harness and ends the task rather than
//! being carried out.

use crate::budget::BudgetKind;

/// Why a task failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The model service failed and retrying did not help.
    Provider(String),
    /// The model service declined to answer.
    Refused(String),
    /// A limit was reached; the conversation so far is kept.
    Budget(BudgetKind),
    /// The model's answer could not be used (for example, cut short).
    Model(String),
    /// A fault in the harness itself.
    Internal(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskState {
    Idle,
    /// Before the plan is approved: only reading and proposing are offered.
    Planning,
    /// A plan is in front of the user.
    AwaitingApproval,
    CallingModel,
    RunningTool,
    /// A question is in front of the user.
    WaitingForUser,
    /// The model is checking its own work.
    Verifying,
    Paused,
    Completed,
    Failed(Failure),
    Cancelled,
}

impl TaskState {
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed(_) | Self::Cancelled)
    }

    /// Whether the table lists a move from this state to `next`.
    pub fn may_become(&self, next: &TaskState) -> bool {
        use TaskState::*;
        // Any live state can end by cancel or failure.
        if matches!(next, Cancelled | Failed(_)) {
            return !self.is_finished();
        }
        match self {
            Idle => matches!(next, Planning | CallingModel),
            Planning => matches!(
                next,
                CallingModel | RunningTool | AwaitingApproval | WaitingForUser
            ),
            CallingModel => matches!(
                next,
                CallingModel
                    | RunningTool
                    | AwaitingApproval
                    | WaitingForUser
                    | Verifying
                    | Paused
                    | Completed
            ),
            RunningTool => matches!(
                next,
                RunningTool | CallingModel | AwaitingApproval | WaitingForUser
            ),
            AwaitingApproval => matches!(next, Planning | CallingModel | RunningTool),
            WaitingForUser => matches!(next, CallingModel | RunningTool),
            Verifying => matches!(
                next,
                RunningTool | CallingModel | AwaitingApproval | WaitingForUser | Completed
            ),
            Paused => matches!(next, CallingModel),
            Completed | Failed(_) | Cancelled => false,
        }
    }
}
