//! The harness's view of the app: every look and every change goes through
//! the in-app connection to the automation core, which checks it exactly as
//! it checks an outside program. What this adds is the user: a request the
//! connection is not yet allowed to make is shown to them first, and only if
//! they say yes does the core get a permission for that exact request.
//!
//! The harness cannot do any of that itself. It holds a `dyn Host`, not the
//! core, so it cannot approve a change, widen a task or reach the drawing
//! another way.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use od_agent::{Answer, CancelToken, Host, Plan, PlanOutcome, Question, ToolCall, ToolOutcome};
use od_api::{catalogue, ops, ApiError, ErrorCode, Scope};
use serde_json::{json, Value};

use super::level::ApprovalLevel;
use super::prompter::{ChangeRequest, Decision, Prompter};
use crate::automation::{AutomationCore, Coverage, InProcess};

/// How often an unfinished operation is looked at while waiting for it.
const LOOK_EVERY: Duration = Duration::from_millis(250);

pub struct AppHost {
    core: Arc<AutomationCore>,
    connection: InProcess,
    prompter: Arc<dyn Prompter>,
    level: ApprovalLevel,
}

impl AppHost {
    /// The assistant's connection to `core`, for one task at `level`. The
    /// task starts able to ask the user to pick objects and nothing more.
    pub fn new(
        core: Arc<AutomationCore>,
        prompter: Arc<dyn Prompter>,
        level: ApprovalLevel,
    ) -> Option<Self> {
        let connection = core.connect_in_process("Assistant")?;
        core.begin_task(connection.grant_id(), &[Scope::UiSelection], None)?;
        Some(Self {
            core,
            connection,
            prompter,
            level,
        })
    }

    pub(super) fn id(&self) -> u64 {
        self.connection.grant_id()
    }

    /// Hold the scopes for changing the drawing, or take them back.
    fn allow_changes(&self, changes: Option<u32>) {
        let scopes: &[Scope] = match changes {
            Some(_) => &[Scope::UiSelection, Scope::DrawingWrite],
            None => &[Scope::UiSelection],
        };
        self.core.begin_task(self.id(), scopes, changes);
    }

    /// Make sure the request may run: it is covered already, or the user
    /// says yes to it. `Err` is what the model is told, or that the task
    /// ended.
    fn clear(
        &self,
        tool: &str,
        operation: &str,
        arguments: &Value,
        cancel: &CancelToken,
    ) -> Result<(), ToolOutcome> {
        let denied = |code, message: &str| Err(ToolOutcome::Failed(ApiError::new(code, message)));
        match self.core.coverage(self.id(), operation, arguments) {
            Coverage::Covered => Ok(()),
            Coverage::TaskFull(limit) => denied(
                ErrorCode::ScopeDenied,
                &format!(
                    "The approved plan has {limit} changes and they are all made. \
                     Propose a new plan for anything more."
                ),
            ),
            Coverage::NoSuchConnection | Coverage::Unknown => denied(
                ErrorCode::InternalError,
                "The assistant's connection is gone.",
            ),
            Coverage::NeedsApproval => {
                // A prompt for the user takes a task scope, never an
                // approval, and so does anything the level would not ask
                // about one request at a time.
                let approvable = !matches!(
                    ops::required_scope(operation),
                    Some(Scope::DrawingRead | Scope::UiSelection) | None
                );
                if !approvable || self.level != ApprovalLevel::ReviewEach {
                    return denied(
                        ErrorCode::ScopeDenied,
                        "This is not allowed yet. Changes need a plan the user has approved.",
                    );
                }
                let request = ChangeRequest {
                    tool: tool.to_owned(),
                    operation: operation.to_owned(),
                    arguments: arguments.clone(),
                };
                match self.prompter.approve_change(&request, cancel) {
                    Decision::Approved => self
                        .core
                        .approve_once(self.id(), operation, arguments)
                        .or_else(|_| {
                            denied(
                                ErrorCode::ScopeDenied,
                                "The approval could not be recorded.",
                            )
                        }),
                    Decision::Declined => Err(ToolOutcome::Declined),
                    Decision::Cancelled => Err(ToolOutcome::Cancelled),
                }
            }
        }
    }

    /// Ask the app once and turn its answer into an outcome.
    fn ask(&self, operation: &str, arguments: Value) -> Result<Value, ApiError> {
        let response = self.connection.call_operation(operation, arguments);
        match (response.ok, response.result, response.error) {
            (true, result, _) => Ok(result.unwrap_or(Value::Null)),
            (false, _, Some(error)) => Err(error),
            (false, _, None) => Err(ApiError::new(
                ErrorCode::InternalError,
                "The app refused without saying why.",
            )),
        }
    }

    /// A request that starts something the user must finish (a selection)
    /// is waited for here, where the model cannot be handed a half-made
    /// answer or left to poll for it. Stop withdraws the prompt.
    fn settle(&self, started: Value, cancel: &CancelToken) -> ToolOutcome {
        let Some(id) = started.get("operation_id").cloned() else {
            return ToolOutcome::Done(started);
        };
        let mut status = started;
        while matches!(
            status["state"].as_str(),
            Some("running" | "awaiting_consent")
        ) {
            if cancel.is_cancelled() {
                let _ = self.ask(ops::OPERATIONS_CANCEL, json!({"operation_id": id}));
                return ToolOutcome::Cancelled;
            }
            thread::sleep(LOOK_EVERY);
            status = match self.ask(ops::OPERATIONS_GET, json!({"operation_id": id})) {
                Ok(status) => status,
                Err(error) => return ToolOutcome::Failed(error),
            };
        }
        ToolOutcome::Done(status)
    }
}

impl Host for AppHost {
    fn call(&self, call: &ToolCall, cancel: &CancelToken) -> ToolOutcome {
        let Some(tool) = catalogue::find(&call.name) else {
            return ToolOutcome::Failed(ApiError::new(
                ErrorCode::InvalidArguments,
                "No such tool.",
            ));
        };
        if let Err(outcome) = self.clear(tool.name, tool.operation, &call.arguments, cancel) {
            return outcome;
        }
        if cancel.is_cancelled() {
            return ToolOutcome::Cancelled;
        }
        match self.ask(tool.operation, call.arguments.clone()) {
            Ok(result) if tool.operation == ops::SELECTION_REQUEST => self.settle(result, cancel),
            Ok(result) => ToolOutcome::Done(result),
            Err(error) => ToolOutcome::Failed(error),
        }
    }

    fn propose_plan(&self, plan: &Plan, cancel: &CancelToken) -> PlanOutcome {
        let outcome = match self.level {
            ApprovalLevel::AutoApprove { .. } => PlanOutcome::Approved,
            level => self.prompter.approve_plan(plan, level, cancel),
        };
        match outcome {
            PlanOutcome::Approved => {
                // Only the level decides what an approved plan allows.
                self.allow_changes(self.level.changes_allowed(plan.steps.len()));
            }
            PlanOutcome::Rejected(_) | PlanOutcome::Cancelled => {
                self.allow_changes(None);
                self.core.forget_approvals(self.id());
            }
        }
        outcome
    }

    fn ask_user(&self, question: &Question, cancel: &CancelToken) -> Answer {
        self.prompter.ask(question, cancel)
    }

    fn finish(&self) {
        self.core.end_task(self.id());
        self.core.forget_approvals(self.id());
    }
}
