//! The automation host: listens on a local socket, authenticates each
//! connection and hands its requests to the UI thread through a bounded
//! queue. Connection threads never touch `AppState`; they own a one-shot
//! reply channel per request and give up when its deadline passes.

use std::collections::HashMap;
use std::io::{self};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use interprocess::local_socket::{prelude::*, ListenerOptions, Name, Stream};
use od_api::instance::{self, Descriptor};
use od_api::limits::QUEUED_REQUESTS_MAX;
use od_api::{ApiError, ErrorCode, Request, Response, Scope, API_VERSION};

use super::{dispatch, Caller};
use crate::state::AppState;
use od_commands::Registry;

/// Most connections served at once; each has a thread.
const MAX_CONNECTIONS: usize = 8;
/// Requests handled, and time spent on them, in one frame.
const FRAME_REQUESTS: usize = 16;
const FRAME_BUDGET: Duration = Duration::from_millis(8);

pub(super) struct Job {
    pub(super) grant_id: u64,
    pub(super) request: Request,
    pub(super) reply: mpsc::Sender<Response>,
    pub(super) expires: Instant,
    /// Set by the connection when it stops waiting: do not run it.
    pub(super) abandoned: Arc<AtomicBool>,
}

/// What one connection may do, until it closes or the user revokes it.
#[derive(Clone, Debug)]
pub struct Grant {
    pub client_name: String,
    pub scopes: Vec<Scope>,
}

pub(super) struct Shared {
    pub(super) instance_id: String,
    pub(super) token: String,
    pub(super) jobs: SyncSender<Job>,
    pub(super) grants: Mutex<HashMap<u64, Grant>>,
    pub(super) next_grant: AtomicU64,
    connections: AtomicUsize,
    shutdown: AtomicBool,
    pub(super) waker: Box<dyn Fn() + Send + Sync>,
}

pub struct AutomationHost {
    shared: Arc<Shared>,
    jobs: Receiver<Job>,
    dir: PathBuf,
    endpoint: String,
}

fn random_hex(bytes: usize) -> io::Result<String> {
    let mut buffer = vec![0u8; bytes];
    getrandom::fill(&mut buffer).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(buffer.iter().map(|b| format!("{b:02x}")).collect())
}

pub(super) fn socket_name(endpoint: &str) -> io::Result<Name<'static>> {
    #[cfg(windows)]
    {
        use interprocess::local_socket::GenericNamespaced;
        endpoint.to_owned().to_ns_name::<GenericNamespaced>()
    }
    #[cfg(not(windows))]
    {
        use interprocess::local_socket::GenericFilePath;
        endpoint.to_owned().to_fs_name::<GenericFilePath>()
    }
}

/// Remove descriptors left by a run that crashed: nothing answers on them.
fn sweep_stale(dir: &Path) {
    for descriptor in instance::list(dir) {
        let alive = socket_name(&descriptor.endpoint)
            .and_then(Stream::connect)
            .is_ok();
        if !alive {
            instance::remove(dir, &descriptor.instance_id);
        }
    }
}

fn endpoint_for(dir: &Path, instance_id: &str) -> String {
    #[cfg(windows)]
    {
        let _ = dir;
        format!("od-{instance_id}")
    }
    #[cfg(not(windows))]
    {
        dir.join(format!("{instance_id}.sock"))
            .to_string_lossy()
            .into_owned()
    }
}

impl AutomationHost {
    /// Start listening and publish the descriptor in `dir`. `waker` is
    /// called whenever a request is queued, so the UI looks at it soon.
    pub fn start(dir: &Path, waker: Box<dyn Fn() + Send + Sync>) -> io::Result<Self> {
        sweep_stale(dir);
        let instance_id = random_hex(8)?;
        let token = random_hex(32)?;
        let endpoint = endpoint_for(dir, &instance_id);
        std::fs::create_dir_all(dir)?;
        let listener = ListenerOptions::new()
            .name(socket_name(&endpoint)?)
            .create_sync()?;
        let (jobs_tx, jobs) = mpsc::sync_channel(QUEUED_REQUESTS_MAX);
        let shared = Arc::new(Shared {
            instance_id: instance_id.clone(),
            token: token.clone(),
            jobs: jobs_tx,
            grants: Mutex::new(HashMap::new()),
            next_grant: AtomicU64::new(1),
            connections: AtomicUsize::new(0),
            shutdown: AtomicBool::new(false),
            waker,
        });
        instance::write(
            dir,
            &Descriptor {
                instance_id,
                pid: std::process::id(),
                app_version: env!("CARGO_PKG_VERSION").to_owned(),
                api_versions: vec![API_VERSION],
                endpoint: endpoint.clone(),
                token,
            },
        )?;
        let accepting = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("automation-accept".into())
            .spawn(move || accept_loop(&accepting, &listener))?;
        Ok(Self {
            shared,
            jobs,
            dir: dir.to_owned(),
            endpoint,
        })
    }

    #[cfg(test)]
    pub fn instance_id(&self) -> &str {
        &self.shared.instance_id
    }

    /// The connections now open, oldest first, for the settings window.
    pub fn grants(&self) -> Vec<(u64, Grant)> {
        let grants = self.shared.grants.lock().map(|g| g.clone());
        let mut list: Vec<(u64, Grant)> = grants.unwrap_or_default().into_iter().collect();
        list.sort_by_key(|(id, _)| *id);
        list
    }

    /// Let a connection change drawings (or take that back). Applies to the
    /// next request it makes; the user's switch, never the client's.
    pub fn set_write(&self, grant_id: u64, allow: bool) {
        if let Ok(mut grants) = self.shared.grants.lock() {
            if let Some(grant) = grants.get_mut(&grant_id) {
                let writes = [Scope::DrawingWrite, Scope::UiSelection];
                grant.scopes.retain(|s| !writes.contains(s));
                if allow {
                    grant.scopes.extend(writes);
                }
            }
        }
    }

    /// Let a connection open, save and export files, in the folders the user
    /// approved only (or take that back).
    pub fn set_files(&self, grant_id: u64, allow: bool) {
        if let Ok(mut grants) = self.shared.grants.lock() {
            if let Some(grant) = grants.get_mut(&grant_id) {
                let files = [Scope::FileRead, Scope::FileWrite];
                grant.scopes.retain(|s| !files.contains(s));
                if allow {
                    grant.scopes.extend(files);
                }
            }
        }
    }

    /// Let a connection run the commands marked safe (view and mode
    /// commands), or take that back.
    pub fn set_commands(&self, grant_id: u64, allow: bool) {
        if let Ok(mut grants) = self.shared.grants.lock() {
            if let Some(grant) = grants.get_mut(&grant_id) {
                grant.scopes.retain(|s| *s != Scope::CommandInvoke);
                if allow {
                    grant.scopes.push(Scope::CommandInvoke);
                }
            }
        }
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
            let Ok(job) = self.jobs.try_recv() else {
                return;
            };
            if job.abandoned.load(Ordering::Acquire) || job.expires < Instant::now() {
                continue;
            }
            let response = self.run(state, registry, &job);
            let _ = job.reply.send(response);
            if start.elapsed() > FRAME_BUDGET {
                break;
            }
        }
        // Budget spent with work possibly left: come back next frame.
        (self.shared.waker)();
    }

    /// A request for the user to select objects is of no use to a caller
    /// that has gone, and must not outlive the user's permission for it: take
    /// the prompt away from the user.
    fn withdraw_orphaned_requests(&self, state: &mut AppState) {
        let allowed: Vec<u64> = self
            .shared
            .grants
            .lock()
            .map(|grants| {
                grants
                    .iter()
                    .filter(|(_, grant)| grant.scopes.contains(&Scope::UiSelection))
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

    fn run(&self, state: &mut AppState, registry: &Registry<AppState>, job: &Job) -> Response {
        let id = &job.request.request_id;
        let allowed = self
            .shared
            .grants
            .lock()
            .map(|g| {
                g.get(&job.grant_id)
                    .is_some_and(|grant| grant.scopes.contains(&job.request.scope))
            })
            .unwrap_or(false);
        if !allowed {
            return Response::failure(
                id,
                ApiError::new(
                    ErrorCode::ScopeDenied,
                    "This connection no longer has that access. Ask the user to tick Allow changes for it in OpenDraft (Settings > Automation).",
                ),
            );
        }
        let instance = &self.shared.instance_id;
        let client = self
            .shared
            .grants
            .lock()
            .ok()
            .and_then(|g| g.get(&job.grant_id).map(|grant| grant.client_name.clone()))
            .unwrap_or_default();
        let caller = Caller {
            grant_id: job.grant_id,
            client: &client,
            history_any: self.shared.grants.lock().is_ok_and(|g| {
                g.get(&job.grant_id)
                    .is_some_and(|grant| grant.scopes.contains(&Scope::HistoryAny))
            }),
        };
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            dispatch(
                state,
                registry,
                instance,
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

impl Drop for AutomationHost {
    fn drop(&mut self) {
        self.shared.shutdown.store(true, Ordering::Release);
        instance::remove(&self.dir, &self.shared.instance_id);
        // Wake the accept loop so it sees the flag and ends.
        if let Ok(name) = socket_name(&self.endpoint) {
            let _ = Stream::connect(name);
        }
    }
}

fn accept_loop(shared: &Arc<Shared>, listener: &interprocess::local_socket::Listener) {
    for incoming in listener.incoming() {
        if shared.shutdown.load(Ordering::Acquire) {
            return;
        }
        let Ok(stream) = incoming else {
            continue;
        };
        if shared.connections.fetch_add(1, Ordering::AcqRel) >= MAX_CONNECTIONS {
            shared.connections.fetch_sub(1, Ordering::AcqRel);
            continue;
        }
        let shared = Arc::clone(shared);
        let spawned = std::thread::Builder::new()
            .name("automation-conn".into())
            .spawn({
                let shared = Arc::clone(&shared);
                move || {
                    let mut stream = stream;
                    super::connection::serve(&shared, &mut stream);
                    shared.connections.fetch_sub(1, Ordering::AcqRel);
                }
            });
        if spawned.is_err() {
            shared.connections.fetch_sub(1, Ordering::AcqRel);
        }
    }
}
