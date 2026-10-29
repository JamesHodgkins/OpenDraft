//! How much the user lets the assistant do without asking again.

/// The most changes an auto-approved task may make, whatever the setting
/// says: a limit the code holds, so a mistaken setting cannot lift it.
pub const AUTO_APPROVE_CEILING: u32 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalLevel {
    /// Every change is shown first, and happens only if the user says yes to
    /// that exact request.
    ReviewEach,
    /// The user approves a plan; the task may then make as many changes as
    /// the plan has steps, and no more.
    ApprovePlan,
    /// The plan is not put to the user: the task may make up to `limit`
    /// changes (never more than [`AUTO_APPROVE_CEILING`]).
    AutoApprove { limit: u32 },
}

impl ApprovalLevel {
    /// The most changes a task may make once its plan is through, if the
    /// level gives it a count.
    pub fn changes_allowed(self, plan_steps: usize) -> Option<u32> {
        match self {
            Self::ReviewEach => None,
            Self::ApprovePlan => Some(u32::try_from(plan_steps).unwrap_or(u32::MAX)),
            Self::AutoApprove { limit } => Some(limit.min(AUTO_APPROVE_CEILING)),
        }
    }
}
