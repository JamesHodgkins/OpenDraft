//! The automation host: the local socket in front of the core. It listens,
//! publishes where it is, and starts a thread per connection; each
//! connection authenticates, then hands its requests to the core's queue.
//! Everything that decides what a request may do is in `core.rs`, which the
//! in-app assistant uses too.

use std::io::{self};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use interprocess::local_socket::{prelude::*, ListenerOptions, Name, Stream};
use od_api::instance::{self, Descriptor};
use od_api::API_VERSION;
use od_commands::Registry;

use super::core::{AutomationCore, Grant, Link};
use crate::state::AppState;

/// Most connections served at once; each has a thread.
const MAX_CONNECTIONS: usize = 8;

/// What the connection threads share: where to send requests, and the
/// secret a client must show.
pub(super) struct Socket {
    pub(super) link: Link,
    pub(super) token: String,
    connections: AtomicUsize,
    shutdown: AtomicBool,
}

pub struct AutomationHost {
    core: AutomationCore,
    socket: Arc<Socket>,
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
        let core = AutomationCore::new(instance_id.clone(), waker);
        let socket = Arc::new(Socket {
            link: core.link(),
            token: token.clone(),
            connections: AtomicUsize::new(0),
            shutdown: AtomicBool::new(false),
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
        let accepting = Arc::clone(&socket);
        std::thread::Builder::new()
            .name("automation-accept".into())
            .spawn(move || accept_loop(&accepting, &listener))?;
        Ok(Self {
            core,
            socket,
            dir: dir.to_owned(),
            endpoint,
        })
    }

    /// The core behind the socket: the same one an in-app connection uses.
    #[allow(dead_code)]
    pub fn core(&self) -> &AutomationCore {
        &self.core
    }

    #[cfg(test)]
    pub fn instance_id(&self) -> &str {
        self.core.instance_id()
    }

    /// The connections now open, oldest first.
    pub fn grants(&self) -> Vec<(u64, Grant)> {
        self.core.grants()
    }

    /// See [`AutomationCore::set_write`].
    pub fn set_write(&self, grant_id: u64, allow: bool) {
        self.core.set_write(grant_id, allow);
    }

    /// See [`AutomationCore::set_files`].
    pub fn set_files(&self, grant_id: u64, allow: bool) {
        self.core.set_files(grant_id, allow);
    }

    /// See [`AutomationCore::set_commands`].
    pub fn set_commands(&self, grant_id: u64, allow: bool) {
        self.core.set_commands(grant_id, allow);
    }

    /// Answer the queued requests, as many as fit in this frame's budget.
    /// Call once per frame from the UI thread.
    pub fn poll(&self, state: &mut AppState, registry: &Registry<AppState>) {
        self.core.poll(state, registry);
    }
}

impl Drop for AutomationHost {
    fn drop(&mut self) {
        self.socket.shutdown.store(true, Ordering::Release);
        instance::remove(&self.dir, self.core.instance_id());
        // Wake the accept loop so it sees the flag and ends.
        if let Ok(name) = socket_name(&self.endpoint) {
            let _ = Stream::connect(name);
        }
    }
}

fn accept_loop(socket: &Arc<Socket>, listener: &interprocess::local_socket::Listener) {
    for incoming in listener.incoming() {
        if socket.shutdown.load(Ordering::Acquire) {
            return;
        }
        let Ok(stream) = incoming else {
            continue;
        };
        if socket.connections.fetch_add(1, Ordering::AcqRel) >= MAX_CONNECTIONS {
            socket.connections.fetch_sub(1, Ordering::AcqRel);
            continue;
        }
        let socket = Arc::clone(socket);
        let spawned = std::thread::Builder::new()
            .name("automation-conn".into())
            .spawn({
                let socket = Arc::clone(&socket);
                move || {
                    let mut stream = stream;
                    super::connection::serve(&socket, &mut stream);
                    socket.connections.fetch_sub(1, Ordering::AcqRel);
                }
            });
        if spawned.is_err() {
            socket.connections.fetch_sub(1, Ordering::AcqRel);
        }
    }
}
