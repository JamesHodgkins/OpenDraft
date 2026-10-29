//! One client connection, start to finish: the token handshake, then
//! request after request until it closes. It never touches `AppState`; it
//! reads a frame, hands the request to the core's queue and writes the
//! answer back. What a request may do is the core's to decide.

use std::io::{Read, Write};

use od_api::frame::{read_frame, write_frame};
use od_api::handshake::{tokens_match, Hello, HelloReply};
use od_api::limits::IPC_FRAME_BYTES;
use od_api::{negotiate, ApiError, ErrorCode, Request, Response, API_VERSION};

use super::host::Socket;

fn send<S: Write>(stream: &mut S, value: &impl serde::Serialize) -> bool {
    match serde_json::to_vec(value) {
        Ok(bytes) => write_frame(stream, &bytes).is_ok(),
        Err(_) => false,
    }
}

/// One connection, start to finish.
pub(super) fn serve<S: Read + Write>(socket: &Socket, stream: &mut S) {
    let Some(grant_id) = handshake(socket, stream) else {
        return;
    };
    while let Ok(frame) = read_frame(stream) {
        if socket.is_stopped() {
            break;
        }
        let response = respond(socket, grant_id, &frame);
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
    socket.link.release(grant_id);
}

/// The first frame must be a valid hello with the right token. On success
/// the connection gets a read-only grant.
fn handshake<S: Read + Write>(socket: &Socket, stream: &mut S) -> Option<u64> {
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
    if !tokens_match(&hello.token, &socket.token) {
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
    let (grant_id, scopes) = socket.link.register(&hello.client_name, false)?;
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
        socket.link.release(grant_id);
        None
    }
}

fn respond(socket: &Socket, grant_id: u64, frame: &[u8]) -> Response {
    let Ok(request) = serde_json::from_slice::<Request>(frame) else {
        return Response::failure(
            "",
            ApiError::new(
                ErrorCode::InvalidArguments,
                "The request could not be read.",
            ),
        );
    };
    socket.link.submit(grant_id, request)
}
