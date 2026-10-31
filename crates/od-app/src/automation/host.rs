//! The socket server: the local socket in front of the app's core. It
//! listens, publishes where it is, and starts a thread per connection; each
//! connection authenticates, then hands its requests to the core's queue.
//! Everything that decides what a request may do is in `core.rs`, which the
//! in-app assistant uses too. The core outlives the server: turning the
//! setting off stops the server and ends its connections, and leaves the
//! in-app assistant's connection alone.

use std::io::{self};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use interprocess::local_socket::{prelude::*, ListenerOptions, Name, Stream};
use od_api::instance::{self, Descriptor};
use od_api::API_VERSION;

use super::core::{AutomationCore, Link};

/// Most connections served at once; each has a thread.
const MAX_CONNECTIONS: usize = 8;
/// How long a new connection has to send its hello before it is dropped.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// What the connection threads share: where to send requests, and the
/// secret a client must show.
pub(super) struct Socket {
    pub(super) link: Link,
    pub(super) token: String,
    pub(super) handshake_timeout: Duration,
    connections: AtomicUsize,
    shutdown: AtomicBool,
}

impl Socket {
    /// Whether the server has been stopped: a connection ends at its next
    /// request.
    pub(super) fn is_stopped(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }
}

pub struct SocketServer {
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

impl SocketServer {
    /// Start listening and publish the descriptor in `dir`, feeding `core`.
    pub fn start(dir: &Path, core: &AutomationCore) -> io::Result<Self> {
        Self::start_with(dir, core, HANDSHAKE_TIMEOUT)
    }

    /// As [`start`](Self::start), with a connection that sends no hello for
    /// `handshake_timeout` dropped.
    pub(super) fn start_with(
        dir: &Path,
        core: &AutomationCore,
        handshake_timeout: Duration,
    ) -> io::Result<Self> {
        sweep_stale(dir);
        let instance_id = core.instance_id().to_owned();
        let token = random_hex(32)?;
        // Its own name each time: the core outlives the server, and a
        // server just stopped may not have let go of the last name yet.
        let endpoint = endpoint_for(dir, &format!("{instance_id}-{}", random_hex(3)?));
        instance::create_private_dir(dir)?;
        let listener = ListenerOptions::new()
            .name(socket_name(&endpoint)?)
            .create_sync()?;
        let socket = Arc::new(Socket {
            link: core.link(),
            token: token.clone(),
            handshake_timeout,
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
            socket,
            dir: dir.to_owned(),
            endpoint,
        })
    }
}

impl Drop for SocketServer {
    fn drop(&mut self) {
        self.socket.shutdown.store(true, Ordering::Release);
        instance::remove(&self.dir, self.socket.link.instance_id());
        // Its connections are over; the in-app assistant's is not.
        self.socket.link.release_outside();
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
