//! The connection to the running app: blocking, one request at a time,
//! reconnecting once if the app went away between calls.

use std::path::Path;
use std::sync::Mutex;

use interprocess::local_socket::{prelude::*, Name, Stream};
use od_api::frame::{read_frame, write_frame};
use od_api::handshake::{Hello, HelloReply};
use od_api::instance::{self, Descriptor};
use od_api::{ApiError, ErrorCode, Request, Response, Scope, API_VERSION};
use serde_json::Value;

pub fn socket_name(endpoint: &str) -> std::io::Result<Name<'static>> {
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

/// An app instance found on this machine.
#[derive(Clone, Debug)]
pub struct Found {
    pub descriptor: Descriptor,
    /// Something is listening; a stale descriptor from a crash is not.
    pub reachable: bool,
}

pub fn discover(dir: &Path) -> Vec<Found> {
    instance::list(dir)
        .into_iter()
        .map(|descriptor| {
            let reachable = socket_name(&descriptor.endpoint)
                .and_then(Stream::connect)
                .is_ok();
            Found {
                descriptor,
                reachable,
            }
        })
        .collect()
}

fn failure(code: ErrorCode, message: &str) -> ApiError {
    ApiError::new(code, message)
}

struct Connection {
    stream: Stream,
    next_request: u64,
}

/// What the app said back to the hello.
pub struct Probe {
    pub api_version: u32,
    pub scopes: Vec<Scope>,
}

/// Connect, say hello, and report what was agreed. For `--check`.
pub fn probe(descriptor: &Descriptor) -> Result<Probe, ApiError> {
    let (_, reply) = connect(descriptor)?;
    Ok(Probe {
        api_version: reply.api_version.unwrap_or(0),
        scopes: reply.scopes,
    })
}

fn connect(descriptor: &Descriptor) -> Result<(Connection, HelloReply), ApiError> {
    let gone = || failure(ErrorCode::InstanceGone, "OpenDraft is not reachable.");
    let name = socket_name(&descriptor.endpoint).map_err(|_| gone())?;
    let mut stream = Stream::connect(name).map_err(|_| gone())?;
    let hello = Hello {
        token: descriptor.token.clone(),
        api_versions: vec![API_VERSION],
        client_name: "opendraft-mcp".to_owned(),
    };
    let bytes = serde_json::to_vec(&hello)
        .map_err(|_| failure(ErrorCode::InternalError, "Could not encode the hello."))?;
    write_frame(&mut stream, &bytes).map_err(|_| gone())?;
    let reply = read_frame(&mut stream).map_err(|_| gone())?;
    let reply: HelloReply = serde_json::from_slice(&reply)
        .map_err(|_| failure(ErrorCode::InternalError, "The app's hello was unreadable."))?;
    if !reply.ok {
        return Err(reply
            .error
            .unwrap_or_else(|| failure(ErrorCode::InternalError, "The app refused the hello.")));
    }
    Ok((
        Connection {
            stream,
            next_request: 1,
        },
        reply,
    ))
}

impl Connection {
    fn call(&mut self, operation: &str, arguments: &Value) -> Result<Response, std::io::Error> {
        let request = Request {
            api_version: API_VERSION.to_string(),
            request_id: format!("req_{}", self.next_request),
            deadline_ms: od_api::limits::REQUEST_DEADLINE_MS,
            scope: od_api::ops::required_scope(operation).unwrap_or(Scope::DrawingRead),
            operation: operation.to_owned(),
            arguments: arguments.clone(),
        };
        self.next_request += 1;
        let bytes = serde_json::to_vec(&request).map_err(std::io::Error::other)?;
        write_frame(&mut self.stream, &bytes)
            .map_err(|e| std::io::Error::other(format!("{e:?}")))?;
        let reply =
            read_frame(&mut self.stream).map_err(|e| std::io::Error::other(format!("{e:?}")))?;
        serde_json::from_slice(&reply).map_err(std::io::Error::other)
    }
}

/// The app this server talks to, chosen on first use.
pub struct Client {
    dir: std::path::PathBuf,
    wanted: Option<String>,
    connection: Mutex<Option<(String, Connection)>>,
}

impl Client {
    pub fn new(dir: std::path::PathBuf, wanted: Option<String>) -> Self {
        Self {
            dir,
            wanted,
            connection: Mutex::new(None),
        }
    }

    pub fn instances(&self) -> Vec<Found> {
        discover(&self.dir)
    }

    /// The one instance to use: the one asked for, or the only one running.
    fn pick(&self) -> Result<Descriptor, ApiError> {
        let found = self.instances();
        if let Some(wanted) = &self.wanted {
            return found
                .into_iter()
                .find(|f| f.descriptor.instance_id == *wanted && f.reachable)
                .map(|f| f.descriptor)
                .ok_or_else(|| {
                    failure(
                        ErrorCode::InstanceGone,
                        "The chosen instance is not running.",
                    )
                });
        }
        let mut live = found.into_iter().filter(|f| f.reachable);
        match (live.next(), live.next()) {
            (None, _) => Err(failure(
                ErrorCode::AppNotRunning,
                "OpenDraft is not running, or its MCP server is turned off (Settings > Automation).",
            )),
            (Some(only), None) => Ok(only.descriptor),
            (Some(_), Some(_)) => Err(failure(
                ErrorCode::InstanceRequired,
                "Several OpenDraft instances are running; start opendraft-mcp with --instance <id> (see opendraft_instances_list).",
            )),
        }
    }

    /// Run one operation, reconnecting once if the connection had dropped.
    /// Returns the app's result, or its error as an [`ApiError`].
    pub fn call(&self, operation: &str, arguments: &Value) -> Result<Value, ApiError> {
        let mut slot = self.connection.lock().map_err(|_| {
            failure(
                ErrorCode::InternalError,
                "The connection lock was poisoned.",
            )
        })?;
        // A read can safely be asked again on a new connection. A change
        // cannot: its reply may be what was lost, so it is never repeated
        // here (a caller that wants to retry sends an idempotency_key).
        let repeatable = od_api::ops::required_scope(operation) == Some(Scope::DrawingRead);
        for attempt in 0..2 {
            if slot.is_none() {
                let descriptor = self.pick()?;
                *slot = Some((descriptor.instance_id.clone(), connect(&descriptor)?.0));
            }
            let Some((_, connection)) = slot.as_mut() else {
                break;
            };
            match connection.call(operation, arguments) {
                Ok(response) if response.ok => return Ok(response.result.unwrap_or(Value::Null)),
                Ok(response) => {
                    return Err(response.error.unwrap_or_else(|| {
                        failure(
                            ErrorCode::InternalError,
                            "The app failed without saying why.",
                        )
                    }))
                }
                Err(_) => {
                    *slot = None;
                    if attempt == 1 || !repeatable {
                        break;
                    }
                }
            }
        }
        if repeatable {
            return Err(failure(
                ErrorCode::InstanceGone,
                "Lost the connection to OpenDraft.",
            ));
        }
        Err(failure(
            ErrorCode::InstanceGone,
            "Lost the connection to OpenDraft while making a change. It may or may not have been made: read the drawing's revision before trying again, or repeat the request with the same idempotency_key.",
        )
        .detail("outcome_unknown", "true"))
    }
}
