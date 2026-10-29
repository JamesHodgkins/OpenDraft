//! What a connection may do. There are three kinds of permission, and they
//! are only ever created by the owner of the core (the app, on the user's
//! say), never by the connection that uses them:
//!
//! - **standing scopes**: the switches the user ticks for an outside program
//!   (Allow changes and the rest), held until they are unticked;
//! - **a task's scopes**: held while an assistant's task runs, with a count of
//!   the changes it has made and an optional limit, revoked when it ends;
//! - **approvals**: each is for one exact request, the operation with the
//!   very arguments the user was shown, and is used up when that request runs.
//!
//! So whatever the assistant is told or tells itself, a change reaches the
//! drawing only if a grant the user gave covers it at that moment.

use std::sync::Arc;
use std::time::{Duration, Instant};

use od_api::{idempotency, ops, ApiError, ErrorCode, Scope};
use serde_json::Value;

use super::core::AutomationCore;

/// How long an approval waits to be used.
const APPROVAL_TTL: Duration = Duration::from_secs(300);
/// Most approvals waiting for one connection.
const APPROVALS_MAX: usize = 8;
/// What a task can be given. Files keep their own consent and folders, and
/// undoing what others did is the user's to allow, so neither is here.
const TASK_SCOPES: [Scope; 3] = [
    Scope::DrawingWrite,
    Scope::UiSelection,
    Scope::CommandInvoke,
];

/// What one connection may do, until it closes or the user revokes it.
#[derive(Clone, Debug)]
pub struct Grant {
    pub client_name: String,
    /// The standing scopes.
    pub scopes: Vec<Scope>,
    /// The in-app assistant, not a program outside: it has no socket, and the
    /// Settings page for outside connections leaves it out (its permissions
    /// are chosen where it is used).
    pub in_process: bool,
    once: Vec<Arc<Approval>>,
    task: Option<Task>,
}

/// Permission for one request: this operation with these arguments.
#[derive(Debug)]
struct Approval {
    operation: String,
    content: Value,
    until: Instant,
}

/// Scopes held for a task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub scopes: Vec<Scope>,
    /// Most changes to the drawing the task may make, if limited.
    pub max_changes: Option<u32>,
    /// Changes made so far.
    pub changes: u32,
}

/// How a request was allowed, so that what it used up can be used up.
pub(super) enum Way {
    Standing,
    Once(usize),
    Task,
}

/// Why a request was not allowed.
pub(super) enum Denial {
    /// No grant covers it.
    Missing,
    /// A task covers it, but it has made all the changes it was allowed.
    TaskFull { limit: u32 },
}

impl Denial {
    /// The error to answer with; `missing` says what to do when nothing
    /// covers it.
    pub(super) fn error(&self, missing: &str) -> ApiError {
        match self {
            Denial::Missing => ApiError::new(ErrorCode::ScopeDenied, missing),
            Denial::TaskFull { limit } => ApiError::new(
                ErrorCode::ScopeDenied,
                "This task has made all the changes it was allowed. Ask the user to allow more in OpenDraft.",
            )
            .detail("limit", limit),
        }
    }
}

/// The arguments without the retry key: the same change is the same change
/// however many times, and under whatever key, it is sent.
fn content_of(arguments: &Value) -> Value {
    let mut content = arguments.clone();
    if let Some(map) = content.as_object_mut() {
        map.remove(idempotency::KEY_FIELD);
    }
    content
}

impl Grant {
    /// A grant that may only read.
    pub(super) fn new(client_name: String, in_process: bool) -> Self {
        Self {
            client_name,
            scopes: vec![Scope::DrawingRead],
            in_process,
            once: Vec::new(),
            task: None,
        }
    }

    /// Whether the scope is held, switched on by the user or by a task.
    pub fn holds(&self, scope: Scope) -> bool {
        self.scopes.contains(&scope)
            || self
                .task
                .as_ref()
                .is_some_and(|t| t.scopes.contains(&scope))
    }

    /// How this request would be allowed now, or why not; nothing is used up.
    /// A standing scope first, then an approval for exactly this request,
    /// then a task.
    pub(super) fn permit(
        &self,
        required: Scope,
        operation: &str,
        arguments: &Value,
        now: Instant,
    ) -> Result<Way, Denial> {
        if self.scopes.contains(&required) {
            return Ok(Way::Standing);
        }
        if !self.once.is_empty() {
            let content = content_of(arguments);
            let found = self
                .once
                .iter()
                .position(|a| a.until > now && a.operation == operation && a.content == content);
            if let Some(index) = found {
                return Ok(Way::Once(index));
            }
        }
        match &self.task {
            Some(task) if task.scopes.contains(&required) => match (required, task.max_changes) {
                (Scope::DrawingWrite, Some(limit)) if task.changes >= limit => {
                    Err(Denial::TaskFull { limit })
                }
                _ => Ok(Way::Task),
            },
            _ => Err(Denial::Missing),
        }
    }

    /// Use up what allowed a request that is about to run. Called under the
    /// same lock as `permit`, so the approval it found is still there.
    pub(super) fn spend(&mut self, way: Way, required: Scope) {
        match way {
            Way::Standing => {}
            Way::Once(index) => {
                if index < self.once.len() {
                    self.once.remove(index);
                }
            }
            Way::Task => {
                if required == Scope::DrawingWrite {
                    if let Some(task) = self.task.as_mut() {
                        task.changes += 1;
                    }
                }
            }
        }
    }
}

/// Whether a request would be let through now, asked of a connection's
/// grant without using anything up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coverage {
    Covered,
    /// Nothing covers it; the user could approve this exact request.
    NeedsApproval,
    /// A task covers it, but it has made all the changes it was allowed.
    TaskFull(u32),
    NoSuchConnection,
    /// Not an operation the app knows.
    Unknown,
}

/// Why an approval was not recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalError {
    NoSuchConnection,
    /// A read needs none, a prompt for the user is not a change to approve
    /// (it takes a standing or task scope), and an unknown operation has no
    /// scope to give.
    NotApprovable,
    /// Too many are waiting.
    TooMany,
}

// Groundwork for the in-app assistant, which is not built yet; the tests
// drive it.
#[allow(dead_code)]
impl AutomationCore {
    /// The user approved this exact request: let this connection make it
    /// once, within a few minutes. The retry key is not part of the request,
    /// so the same change may be sent again under another.
    pub fn approve_once(
        &self,
        grant_id: u64,
        operation: &str,
        arguments: &Value,
    ) -> Result<(), ApprovalError> {
        self.approve_for(grant_id, operation, arguments, APPROVAL_TTL)
    }

    pub(super) fn approve_for(
        &self,
        grant_id: u64,
        operation: &str,
        arguments: &Value,
        lasts: Duration,
    ) -> Result<(), ApprovalError> {
        match ops::required_scope(operation) {
            Some(Scope::DrawingRead | Scope::UiSelection) | None => {
                return Err(ApprovalError::NotApprovable);
            }
            Some(_) => {}
        }
        let mut grants = self
            .link
            .inner
            .grants
            .lock()
            .map_err(|_| ApprovalError::NoSuchConnection)?;
        let grant = grants
            .get_mut(&grant_id)
            .ok_or(ApprovalError::NoSuchConnection)?;
        let now = Instant::now();
        grant.once.retain(|a| a.until > now);
        if grant.once.len() >= APPROVALS_MAX {
            return Err(ApprovalError::TooMany);
        }
        grant.once.push(Arc::new(Approval {
            operation: operation.to_owned(),
            content: content_of(arguments),
            until: now + lasts,
        }));
        Ok(())
    }

    /// Whether `operation` with `arguments` would be let through for this
    /// connection now. For the app side of the assistant, which asks the user
    /// before approving what is not covered; the answer can change before the
    /// request runs, and `execute` decides again.
    pub fn coverage(&self, grant_id: u64, operation: &str, arguments: &Value) -> Coverage {
        let Some(required) = ops::required_scope(operation) else {
            return Coverage::Unknown;
        };
        let Ok(grants) = self.link.inner.grants.lock() else {
            return Coverage::NoSuchConnection;
        };
        let Some(grant) = grants.get(&grant_id) else {
            return Coverage::NoSuchConnection;
        };
        match grant.permit(required, operation, arguments, Instant::now()) {
            Ok(_) => Coverage::Covered,
            Err(Denial::Missing) => Coverage::NeedsApproval,
            Err(Denial::TaskFull { limit }) => Coverage::TaskFull(limit),
        }
    }

    /// Withdraw every approval not yet used.
    pub fn forget_approvals(&self, grant_id: u64) {
        if let Ok(mut grants) = self.link.inner.grants.lock() {
            if let Some(grant) = grants.get_mut(&grant_id) {
                grant.once.clear();
            }
        }
    }

    /// Hold scopes for a task, replacing any task it had: the ones allowed
    /// for a task (changing the drawing, prompting the user, and the safe
    /// commands) and no others. Returns what is now held, or `None` if there
    /// is no such connection.
    pub fn begin_task(
        &self,
        grant_id: u64,
        scopes: &[Scope],
        max_changes: Option<u32>,
    ) -> Option<Vec<Scope>> {
        let held: Vec<Scope> = TASK_SCOPES
            .into_iter()
            .filter(|s| scopes.contains(s))
            .collect();
        let mut grants = self.link.inner.grants.lock().ok()?;
        let grant = grants.get_mut(&grant_id)?;
        grant.task = Some(Task {
            scopes: held.clone(),
            max_changes,
            changes: 0,
        });
        Some(held)
    }

    /// The task is over: take its scopes back.
    pub fn end_task(&self, grant_id: u64) {
        if let Ok(mut grants) = self.link.inner.grants.lock() {
            if let Some(grant) = grants.get_mut(&grant_id) {
                grant.task = None;
            }
        }
    }

    /// The task's scopes and how many changes it has made.
    pub fn task(&self, grant_id: u64) -> Option<Task> {
        self.link
            .inner
            .grants
            .lock()
            .ok()?
            .get(&grant_id)?
            .task
            .clone()
    }
}
