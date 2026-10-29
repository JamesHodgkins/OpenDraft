#![allow(clippy::unwrap_used, dead_code)] // shared by several test files
//! A stand-in for the running app: speaks the automation protocol on a
//! local socket and answers from canned data, recording what it was asked.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use interprocess::local_socket::{prelude::*, ListenerOptions, Name};
use od_api::frame::{read_frame, write_frame};
use od_api::handshake::{Hello, HelloReply};
use od_api::instance::{self, Descriptor};
use od_api::{ApiError, ErrorCode, Request, Response, Scope};
use serde_json::json;

mod sidecar;
// Each test file uses some of what is shared here.
#[allow(unused_imports)]
pub use sidecar::{scratch, Sidecar};

pub const TOKEN: &str = "fake-token";

/// A real 1 by 1 PNG, as base64.
pub const TINY_PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4//8/AwAI/AL+XJ/P2QAAAABJRU5ErkJggg==";

pub struct FakeApp {
    pub dir: PathBuf,
    pub seen: Arc<Mutex<Vec<Request>>>,
    stopped: Arc<AtomicBool>,
    instance_id: String,
}

fn name(endpoint: &str) -> std::io::Result<Name<'static>> {
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

fn answer(request: &Request) -> Response {
    let id = &request.request_id;
    match request.operation.as_str() {
        "documents.list" => Response::success(
            id,
            json!({"documents": [{"document_id": "doc_fake_1", "title": "Plan", "active": true,
                                   "modified": false, "available": true, "revision": "7"}]}),
        ),
        "view.render" if request.arguments["format"] == "png" => Response::success(
            id,
            json!({"revision": "7", "format": "png", "png_base64": TINY_PNG,
                   "width": 1.0, "height": 1.0, "vertices": 0}),
        ),
        "view.render" => Response::success(
            id,
            json!({"revision": "7", "format": "svg", "svg": "<svg/>", "width": 10.0, "height": 5.0, "vertices": 0}),
        ),
        _ => match request.arguments["document_id"].as_str() {
            Some("doc_blocked") => Response::failure(id, ApiError::block_editor_open()),
            Some("doc_missing") => Response::failure(
                id,
                ApiError::new(ErrorCode::DocumentNotFound, "No such drawing."),
            ),
            _ => Response::success(
                id,
                json!({"operation": request.operation, "arguments": request.arguments}),
            ),
        },
    }
}

fn serve<S: std::io::Read + std::io::Write>(
    stream: &mut S,
    seen: &Mutex<Vec<Request>>,
    stopped: &AtomicBool,
) {
    let Ok(frame) = read_frame(stream) else {
        return;
    };
    let Ok(hello) = serde_json::from_slice::<Hello>(&frame) else {
        return;
    };
    let ok = hello.token == TOKEN;
    let reply = HelloReply {
        ok,
        api_version: ok.then_some(1),
        grant_id: ok.then(|| "1".to_owned()),
        scopes: if ok { vec![Scope::DrawingRead] } else { vec![] },
        error: (!ok).then(|| ApiError::new(ErrorCode::ScopeDenied, "Wrong token.")),
    };
    let _ = write_frame(stream, &serde_json::to_vec(&reply).unwrap_or_default());
    if !ok {
        return;
    }
    while let Ok(frame) = read_frame(stream) {
        if stopped.load(Ordering::Acquire) {
            return;
        }
        let Ok(request) = serde_json::from_slice::<Request>(&frame) else {
            return;
        };
        let response = answer(&request);
        if let Ok(mut seen) = seen.lock() {
            seen.push(request);
        }
        if write_frame(stream, &serde_json::to_vec(&response).unwrap_or_default()).is_err() {
            return;
        }
    }
}

impl FakeApp {
    pub fn start(dir: &Path, instance_id: &str, token: &str) -> FakeApp {
        std::fs::create_dir_all(dir).unwrap();
        #[cfg(windows)]
        let endpoint = format!("od-fake-{instance_id}-{}", std::process::id());
        #[cfg(not(windows))]
        let endpoint = dir
            .join(format!("{instance_id}.sock"))
            .to_string_lossy()
            .into_owned();
        let listener = ListenerOptions::new()
            .name(name(&endpoint).unwrap())
            .create_sync()
            .unwrap();
        instance::write(
            dir,
            &Descriptor {
                instance_id: instance_id.to_owned(),
                pid: std::process::id(),
                app_version: "0.0.0-test".to_owned(),
                api_versions: vec![1],
                endpoint,
                token: token.to_owned(),
            },
        )
        .unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let stopped = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stopped);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let log = Arc::clone(&log);
                let flag = Arc::clone(&flag);
                std::thread::spawn(move || {
                    let mut stream = stream;
                    serve(&mut stream, &log, &flag);
                });
            }
        });
        FakeApp {
            dir: dir.to_owned(),
            seen,
            stopped,
            instance_id: instance_id.to_owned(),
        }
    }

    pub fn operations(&self) -> Vec<String> {
        self.seen
            .lock()
            .map(|s| s.iter().map(|r| r.operation.clone()).collect())
            .unwrap_or_default()
    }

    /// Stop answering: connections close on their next request.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
    }

    pub fn id(&self) -> &str {
        &self.instance_id
    }
}
