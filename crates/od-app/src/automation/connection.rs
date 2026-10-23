//! One client connection, start to finish: the token handshake, then
//! request after request until it closes. It never touches `AppState`; it
//! queues each request for the UI thread and waits for the reply.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, TrySendError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use od_api::frame::{read_frame, write_frame};
use od_api::handshake::{tokens_match, Hello, HelloReply};
use od_api::limits::{IPC_FRAME_BYTES, QUEUED_REQUESTS_MAX, REQUEST_DEADLINE_MS};
use od_api::{negotiate, ops, ApiError, ErrorCode, Request, Response, Scope, API_VERSION};

use super::host::{Grant, Job, Shared};

fn send<S: Write>(stream: &mut S, value: &impl serde::Serialize) -> bool {
    match serde_json::to_vec(value) {
        Ok(bytes) => write_frame(stream, &bytes).is_ok(),
        Err(_) => false,
    }
}

/// One connection, start to finish.
pub(super) fn serve<S: Read + Write>(shared: &Shared, stream: &mut S) {
    let Some(grant_id) = handshake(shared, stream) else {
        return;
    };
    while let Ok(frame) = read_frame(stream) {
        let response = respond(shared, grant_id, &frame);
        let sent = match serde_json::to_vec(&response) {
            Ok(bytes) if bytes.len() <= IPC_FRAME_BYTES => write_frame(stream, &bytes).is_ok(),
            _ => send(
                stream,
                &Response::failure(
                    &response.request_id,
                    ApiError::new(
                        ErrorCode::ResultTooLarge,
                        "The answer does not fit in one message; ask for less.",
                    )
                    .detail("limit_bytes", IPC_FRAME_BYTES),
                ),
            ),
        };
        if !sent {
            break;
        }
    }
    if let Ok(mut grants) = shared.grants.lock() {
        grants.remove(&grant_id);
    }
}

/// The first frame must be a valid hello with the right token. On success
/// the connection gets a read-only grant.
fn handshake<S: Read + Write>(shared: &Shared, stream: &mut S) -> Option<u64> {
    let frame = read_frame(stream).ok()?;
    let refuse = |stream: &mut S, error: ApiError| {
        send(
            stream,
            &HelloReply {
                ok: false,
                api_version: None,
                grant_id: None,
                scopes: Vec::new(),
                error: Some(error),
            },
        );
        None
    };
    let Ok(hello) = serde_json::from_slice::<Hello>(&frame) else {
        return refuse(
            stream,
            ApiError::new(ErrorCode::InvalidArguments, "Bad hello."),
        );
    };
    if !tokens_match(&hello.token, &shared.token) {
        return refuse(
            stream,
            ApiError::new(ErrorCode::ScopeDenied, "Wrong token."),
        );
    }
    let Some(version) = negotiate(&[API_VERSION], &hello.api_versions) else {
        return refuse(
            stream,
            ApiError::new(
                ErrorCode::InvalidArguments,
                "No automation API version in common; update OpenDraft or the MCP server.",
            )
            .detail("app_versions", API_VERSION),
        );
    };
    let grant_id = shared.next_grant.fetch_add(1, Ordering::AcqRel);
    let scopes = vec![Scope::DrawingRead];
    shared.grants.lock().ok()?.insert(
        grant_id,
        Grant {
            client_name: hello.client_name.chars().take(80).collect(),
            scopes: scopes.clone(),
        },
    );
    let reply = HelloReply {
        ok: true,
        api_version: Some(version),
        grant_id: Some(grant_id.to_string()),
        scopes,
        error: None,
    };
    if send(stream, &reply) {
        Some(grant_id)
    } else {
        shared.grants.lock().ok()?.remove(&grant_id);
        None
    }
}

fn respond(shared: &Shared, grant_id: u64, frame: &[u8]) -> Response {
    let Ok(request) = serde_json::from_slice::<Request>(frame) else {
        return Response::failure(
            "",
            ApiError::new(
                ErrorCode::InvalidArguments,
                "The request could not be read.",
            ),
        );
    };
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
    let granted = shared
        .grants
        .lock()
        .map(|g| {
            g.get(&grant_id)
                .is_some_and(|g| g.scopes.contains(&required))
        })
        .unwrap_or(false);
    if !granted {
        return fail(
            ErrorCode::ScopeDenied,
            "This connection may only read. Ask the user to tick Allow changes for it in OpenDraft (Settings > Automation).",
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
    match shared.jobs.try_send(job) {
        Ok(()) => (shared.waker)(),
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
