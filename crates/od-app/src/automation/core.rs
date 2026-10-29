//! The automation core: who may do what, the bounded queue of requests, and
//! the one place a request is checked and run on the UI thread. It has no
//! transport. The socket listener (`host.rs`) and an in-app connection
//! (`InProcess`) are both just ways to get a request here, so a request from
//! either goes through the same checks, limits, deadlines and caps, and no
//! caller can reach `AppState` another way.
//!
//! Connection threads and in-app callers never touch `AppState`: they queue
//! a `Job` with a one-shot reply channel and give up when its deadline
//! passes; `AutomationCore::poll` runs the jobs, once per frame, on the UI
//! thread.

use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use od_api::limits::{QUEUED_REQUESTS_MAX, REQUEST_DEADLINE_MS};
use od_api::{ops, ApiError, ErrorCode, Request, Response, Scope, API_VERSION};
use od_commands::Registry;
use serde_json::Value;

use super::grants::{Denial, Grant};
use super::{dispatch, Caller};
use crate::state::AppState;

/// Requests handled, and time spent on them, in one frame.
const FRAME_REQUESTS: usize = 16;
const FRAME_BUDGET: Duration = Duration::from_millis(8);
/// Longest a connection's self-chosen name is kept.
const CLIENT_NAME_MAX: usize = 80;

pub(super) struct Job {
    pub(super) grant_id: u64,
    pub(super) request: Request,
    pub(super) reply: mpsc::Sender<Response>,
    pub(super) expires: Instant,
    /// Set by the caller when it stops waiting: do not run it.
    pub(super) abandoned: Arc<AtomicBool>,
}

pub(super) struct Inner {
    instance_id: String,
    jobs: SyncSender<Job>,
    pub(super) grants: Mutex<HashMap<u64, Grant>>,
    next_grant: AtomicU64,
    /// Called whenever a request is queued, so the UI looks at it soon.
    waker: Box<dyn Fn() + Send + Sync>,
}

/// What a connection holds: it can take a grant, submit requests and give
/// the grant back. It cannot run anything or change a grant's scopes.
#[derive(Clone)]
pub struct Link {
    pub(super) inner: Arc<Inner>,
}

impl Link {
    pub fn instance_id(&self) -> &str {
        &self.inner.instance_id
    }

    /// A new read-only grant for a connection; its id and scopes.
    pub(super) fn register(
        &self,
        client_name: &str,
        in_process: bool,
    ) -> Option<(u64, Vec<Scope>)> {
        let id = self.inner.next_grant.fetch_add(1, Ordering::AcqRel);
        let grant = Grant::new(
            client_name.chars().take(CLIENT_NAME_MAX).collect(),
            in_process,
        );
        let scopes = grant.scopes.clone();
        self.inner.grants.lock().ok()?.insert(id, grant);
        Some((id, scopes))
    }

    pub(super) fn release(&self, grant_id: u64) {
        if let Ok(mut grants) = self.inner.grants.lock() {
            grants.remove(&grant_id);
        }
    }

    /// Give back every grant of an outside program (the socket has stopped).
    pub(super) fn release_outside(&self) {
        if let Ok(mut grants) = self.inner.grants.lock() {
            grants.retain(|_, grant| grant.in_process);
        }
    }

    /// Check a request and queue it for the UI thread, then wait for the
    /// answer or the deadline. The checks here refuse early what could never
    /// run; the ones that matter are made again when it runs, against the
    /// grant as it is then.
    pub(super) fn submit(&self, grant_id: u64, request: Request) -> Response {
        let id = request.request_id.clone();
        let fail = |code, message: &str| Response::failure(&id, ApiError::new(code, message));
        if request.api_version != API_VERSION.to_string() {
            return fail(ErrorCode::InvalidArguments, "Unsupported api_version.");
        }
        let Some(required) = ops::required_scope(&request.operation) else {
            return fail(ErrorCode::InvalidArguments, "Unknown operation.");
        };
        if request.scope != required {
            return fail(
                ErrorCode::InvalidArguments,
                "The scope does not match the operation.",
            );
        }
        let denial = match self.inner.grants.lock() {
            Ok(grants) => grants
                .get(&grant_id)
                .map_or(Some(Denial::Missing), |grant| {
                    grant
                        .permit(
                            required,
                            &request.operation,
                            &request.arguments,
                            Instant::now(),
                        )
                        .err()
                }),
            Err(_) => Some(Denial::Missing),
        };
        if let Some(denial) = denial {
            return Response::failure(
                &id,
                denial.error(
                    "This connection may only read. Ask the user to tick Allow changes for it in OpenDraft (Settings > Automation).",
                ),
            );
        }
        let wait = Duration::from_millis(request.deadline_ms.clamp(1, REQUEST_DEADLINE_MS));
        let (reply, answer) = mpsc::channel();
        let abandoned = Arc::new(AtomicBool::new(false));
        let job = Job {
            grant_id,
            request,
            reply,
            expires: Instant::now() + wait,
            abandoned: Arc::clone(&abandoned),
        };
        match self.inner.jobs.try_send(job) {
            Ok(()) => (self.inner.waker)(),
            Err(TrySendError::Full(_)) => {
                return Response::failure(
                    &id,
                    ApiError::new(ErrorCode::RateLimited, "Too many requests are waiting.")
                        .retryable()
                        .detail("queue_limit", QUEUED_REQUESTS_MAX),
                );
            }
            Err(TrySendError::Disconnected(_)) => {
                return fail(ErrorCode::InstanceGone, "OpenDraft is shutting down.");
            }
        }
        match answer.recv_timeout(wait) {
            Ok(response) => response,
            Err(RecvTimeoutError::Timeout) => {
                abandoned.store(true, Ordering::Release);
                Response::failure(
                    &id,
                    ApiError::new(
                        ErrorCode::DeadlineExceeded,
                        "OpenDraft did not answer in time.",
                    )
                    .retryable(),
                )
            }
            Err(RecvTimeoutError::Disconnected) => {
                fail(ErrorCode::InstanceGone, "OpenDraft dropped the request.")
            }
        }
    }
}

/// The in-app assistant's connection: no socket, the same grant, queue and
/// checks as any other. Dropping it gives the grant back. Its grant starts
/// read-only, and only the owner of the core can widen it.
// Groundwork for the in-app assistant, which is not built yet; the tests
// drive it.
#[allow(dead_code)]
pub struct InProcess {
    link: Link,
    grant_id: u64,
    next_request: AtomicU64,
}

#[allow(dead_code)]
impl InProcess {
    pub fn grant_id(&self) -> u64 {
        self.grant_id
    }

    /// Send one request and wait for its answer (or its deadline).
    pub fn call(&self, request: Request) -> Response {
        self.link.submit(self.grant_id, request)
    }

    /// Call `operation` with `arguments`: the scope is the operation's own.
    pub fn call_operation(&self, operation: &str, arguments: Value) -> Response {
        let n = self.next_request.fetch_add(1, Ordering::AcqRel);
        self.call(Request {
            api_version: API_VERSION.to_string(),
            request_id: format!("app_{n}"),
            deadline_ms: REQUEST_DEADLINE_MS,
            scope: ops::required_scope(operation).unwrap_or(Scope::DrawingRead),
            operation: operation.to_owned(),
            arguments,
        })
    }
}

impl Drop for InProcess {
    fn drop(&mut self) {
        self.link.release(self.grant_id);
    }
}

pub struct AutomationCore {
    pub(super) link: Link,
    // Behind a lock only so the core can be shared with the assistant's
    // thread; `poll`, on the UI thread, is the one reader.
    jobs: Mutex<Receiver<Job>>,
}

/// A name for this run of the app: 16 hex digits, unlikely to repeat.
pub fn fresh_instance_id() -> String {
    let mut bytes = [0u8; 8];
    if getrandom::fill(&mut bytes).is_err() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        bytes = (now as u64 ^ u64::from(std::process::id())).to_le_bytes();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl AutomationCore {
    /// A core for the app run called `instance_id`. `waker` is called
    /// whenever a request is queued.
    pub fn new(instance_id: String, waker: Box<dyn Fn() + Send + Sync>) -> Self {
        let (jobs_tx, jobs) = mpsc::sync_channel(QUEUED_REQUESTS_MAX);
        Self {
            link: Link {
                inner: Arc::new(Inner {
                    instance_id,
                    jobs: jobs_tx,
                    grants: Mutex::new(HashMap::new()),
                    next_grant: AtomicU64::new(1),
                    waker,
                }),
            },
            jobs: Mutex::new(jobs),
        }
    }

    /// What a transport holds to feed this core.
    pub fn link(&self) -> Link {
        self.link.clone()
    }

    /// An in-app connection with a read-only grant of its own.
    #[allow(dead_code)]
    pub fn connect_in_process(&self, client_name: &str) -> Option<InProcess> {
        let (grant_id, _) = self.link.register(client_name, true)?;
        Some(InProcess {
            link: self.link.clone(),
            grant_id,
            next_request: AtomicU64::new(1),
        })
    }

    pub fn instance_id(&self) -> &str {
        self.link.instance_id()
    }

    /// The connections now open, oldest first.
    pub fn grants(&self) -> Vec<(u64, Grant)> {
        let grants = self.link.inner.grants.lock().map(|g| g.clone());
        let mut list: Vec<(u64, Grant)> = grants.unwrap_or_default().into_iter().collect();
        list.sort_by_key(|(id, _)| *id);
        list
    }

    /// Replace the scopes in `group` on a grant: give them all, or none.
    fn set_group(&self, grant_id: u64, group: &[Scope], allow: bool) {
        if let Ok(mut grants) = self.link.inner.grants.lock() {
            if let Some(grant) = grants.get_mut(&grant_id) {
                grant.scopes.retain(|s| !group.contains(s));
                if allow {
                    grant.scopes.extend_from_slice(group);
                }
            }
        }
    }

    /// Let a connection change drawings (or take that back). Applies to the
    /// next request it makes; the user's switch, never the client's.
    pub fn set_write(&self, grant_id: u64, allow: bool) {
        self.set_group(grant_id, &[Scope::DrawingWrite, Scope::UiSelection], allow);
    }

    /// Let a connection open, save and export files, in the folders the user
    /// approved only (or take that back).
    pub fn set_files(&self, grant_id: u64, allow: bool) {
        self.set_group(grant_id, &[Scope::FileRead, Scope::FileWrite], allow);
    }

    /// Let a connection run the commands marked safe (view and mode
    /// commands), or take that back.
    pub fn set_commands(&self, grant_id: u64, allow: bool) {
        self.set_group(grant_id, &[Scope::CommandInvoke], allow);
    }

    /// Answer the queued requests, as many as fit in this frame's budget.
    /// Call once per frame from the UI thread.
    pub fn poll(&self, state: &mut AppState, registry: &Registry<AppState>) {
        state.file_ops.expire();
        state.observe_events();
        super::command_runs::sync(state);
        self.withdraw_orphaned_requests(state);
        let start = Instant::now();
        for _ in 0..FRAME_REQUESTS {
            let job = match self.jobs.lock() {
                Ok(jobs) => jobs.try_recv(),
                Err(_) => return,
            };
            let Ok(job) = job else {
                return;
            };
            if job.abandoned.load(Ordering::Acquire) || job.expires < Instant::now() {
                continue;
            }
            let response = self.execute(state, registry, &job);
            let _ = job.reply.send(response);
            if start.elapsed() > FRAME_BUDGET {
                break;
            }
        }
        // Budget spent with work possibly left: come back next frame.
        (self.link.inner.waker)();
    }

    /// A request for the user to select objects is of no use to a caller
    /// that has gone, and must not outlive the user's permission for it: take
    /// the prompt away from the user.
    fn withdraw_orphaned_requests(&self, state: &mut AppState) {
        let allowed: Vec<u64> = self
            .link
            .inner
            .grants
            .lock()
            .map(|grants| {
                grants
                    .iter()
                    .filter(|(_, grant)| grant.holds(Scope::UiSelection))
                    .map(|(id, _)| *id)
                    .collect()
            })
            .unwrap_or_default();
        super::selection_request::withdraw(
            state,
            |grant| !allowed.contains(&grant),
            "The assistant that asked you to select objects has gone or is no longer allowed to ask.",
        );
    }

    /// Run one request on the UI thread: the one place anything is checked
    /// against the grant as it is now, and the one way to `dispatch`. The
    /// scope needed is the operation's own, from its table; whatever the
    /// request says about its scope is not believed.
    pub(super) fn execute(
        &self,
        state: &mut AppState,
        registry: &Registry<AppState>,
        job: &Job,
    ) -> Response {
        let id = &job.request.request_id;
        let Some(required) = ops::required_scope(&job.request.operation) else {
            return Response::failure(
                id,
                ApiError::new(ErrorCode::InvalidArguments, "Unknown operation."),
            );
        };
        // Allowed, and what allowed it used up, in one step under the lock:
        // a one-shot approval cannot be used by two requests.
        let admitted = match self.link.inner.grants.lock() {
            Ok(mut grants) => match grants.get_mut(&job.grant_id) {
                Some(grant) => grant
                    .permit(
                        required,
                        &job.request.operation,
                        &job.request.arguments,
                        Instant::now(),
                    )
                    .map(|way| {
                        grant.spend(way, required);
                        (
                            grant.client_name.clone(),
                            grant.scopes.contains(&Scope::HistoryAny),
                        )
                    }),
                None => Err(Denial::Missing),
            },
            Err(_) => Err(Denial::Missing),
        };
        let (client, history_any) = match admitted {
            Ok(allowed) => allowed,
            Err(denial) => {
                return Response::failure(
                    id,
                    denial.error(
                        "This connection no longer has that access. Ask the user to tick Allow changes for it in OpenDraft (Settings > Automation).",
                    ),
                );
            }
        };
        let caller = Caller {
            grant_id: job.grant_id,
            client: &client,
            history_any,
        };
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            dispatch(
                state,
                registry,
                self.instance_id(),
                &caller,
                &job.request.operation,
                job.request.arguments.clone(),
            )
        }));
        match outcome {
            Ok(Ok(result)) => Response::success(id, result),
            Ok(Err(error)) => Response::failure(id, error),
            Err(_) => Response::failure(
                id,
                ApiError::new(
                    ErrorCode::InternalError,
                    "The request failed inside the app.",
                ),
            ),
        }
    }
}
