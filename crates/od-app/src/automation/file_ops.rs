//! `files.open`, `files.save`, `files.save_as`, `files.export`,
//! `operations.get` and `operations.cancel`.
//!
//! Every path goes through `od_api::paths::check` first (approved folders,
//! canonical form, extension). Reading or writing a file the drawing already
//! owns needs no question; replacing any other existing file waits for the
//! user's answer in the app. The work itself runs on the app's own file
//! worker, from a copy taken when it starts.

use std::path::Path;

use od_api::file_ops::{
    ExportArgs, ExportFormat, OpenArgs, OperationArgs, OperationState, SaveArgs, SaveAsArgs,
};
use od_api::limits::QUEUED_REQUESTS_MAX;
use od_api::paths::{check, Access, Checked};
use od_api::{ApiError, ErrorCode};
use od_core::LayoutId;

use super::file_state::{Kind, Plan};
use super::guard::Caller;
use super::{read::parse, resolve};
use crate::files::same_file;
use crate::state::AppState;

const OPEN_EXTENSIONS: &[&str] = &["odx", "dxf", "dwg"];
/// Operations one connection may have in flight.
const MAX_UNFINISHED: usize = 8;

fn invalid(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::InvalidArguments, message)
}

fn encode(value: &impl serde::Serialize) -> Result<serde_json::Value, ApiError> {
    serde_json::to_value(value)
        .map_err(|_| ApiError::new(ErrorCode::InternalError, "The result could not be encoded."))
}

fn status(state: &AppState, instance: &str, id: u64) -> Result<serde_json::Value, ApiError> {
    match state.file_ops.get(id) {
        Some(op) => encode(&op.status(instance)),
        None => Err(invalid("No such operation.")),
    }
}

pub(super) fn not_too_many(state: &AppState, caller: &Caller) -> Result<(), ApiError> {
    if state.file_ops.unfinished(caller.grant_id) >= MAX_UNFINISHED {
        return Err(ApiError::new(
            ErrorCode::OperationBusy,
            "This connection already has too many file operations going. Wait for some to finish.",
        )
        .retryable()
        .detail("limit", MAX_UNFINISHED.min(QUEUED_REQUESTS_MAX)));
    }
    Ok(())
}

fn roots(state: &AppState) -> Vec<std::path::PathBuf> {
    state.settings.automation_roots.clone()
}

/// The drawing a write is about, by index. No command may be running in it
/// for the revision check to mean anything, but saving does not need the
/// drawing to be idle: it works from a copy.
fn document_index(
    state: &AppState,
    instance: &str,
    id: &str,
    expected: Option<od_api::WireId>,
) -> Result<usize, ApiError> {
    let uid = resolve(state, instance, id)?.uid;
    let index = state
        .docs
        .iter()
        .position(|d| d.uid == uid)
        .ok_or_else(|| ApiError::new(ErrorCode::DocumentClosed, "The drawing was closed."))?;
    let actual = state.docs[index].session.revision();
    match expected {
        Some(e) if e.0 != actual => Err(ApiError::stale_revision(e.0, actual)),
        _ => Ok(index),
    }
}

/// Start the operation's job, or fail it. Called at once for work that
/// needs no question, and after the user says yes for the rest.
pub(super) fn start(state: &mut AppState, id: u64) {
    let Some(op) = state.file_ops.get_mut(id) else {
        return;
    };
    op.state = OperationState::Running;
    let (plan, path, expected) = (op.plan, op.path.clone(), op.expected_revision);
    let uid = match plan {
        Plan::Open | Plan::Command { .. } => None,
        Plan::Save { uid }
        | Plan::SaveAs { uid }
        | Plan::ExportDxf { uid }
        | Plan::ExportSheet { uid, .. } => Some(uid),
    };
    let mut index = None;
    if let Some(uid) = uid {
        let Some(found) = state.docs.iter().position(|d| d.uid == uid) else {
            state
                .file_ops
                .fail(id, ErrorCode::DocumentClosed, "The drawing was closed.");
            return;
        };
        let actual = state.docs[found].session.revision();
        if expected.is_some_and(|e| e != actual) {
            state.file_ops.fail(
                id,
                ErrorCode::StaleRevision,
                "The drawing changed after it was read, so it was not written.",
            );
            return;
        }
        index = Some(found);
    }
    let revision = index.map(|i| state.docs[i].session.revision());
    let job = match (plan, index) {
        (Plan::Command { .. }, _) => None,
        (Plan::Open, _) => state.begin_open(&path),
        (Plan::Save { .. } | Plan::SaveAs { .. }, Some(i)) => state.begin_save_job(i, &path),
        (Plan::ExportDxf { .. }, Some(i)) => Some(state.begin_export_of(i, &path)),
        (Plan::ExportSheet { layout, .. }, Some(i)) => {
            if state.docs[i].session.document().layout(layout).is_none() {
                state.file_ops.fail(
                    id,
                    ErrorCode::InvalidReference,
                    "That layout no longer exists.",
                );
                return;
            }
            Some(state.begin_plot_of(i, &path, layout))
        }
        _ => None,
    };
    match job {
        Some(job) => {
            if let Some(op) = state.file_ops.get_mut(id) {
                // An inline (test) worker may already have finished it.
                if op.state == OperationState::Running {
                    op.job = Some(job);
                }
                op.revision = revision;
            }
        }
        None => state.file_ops.fail(
            id,
            ErrorCode::OperationBusy,
            "That file or drawing is already being read or written.",
        ),
    }
}

/// Create the operation and either start it or put its question to the user.
fn begin(
    state: &mut AppState,
    caller: &Caller,
    kind: Kind,
    plan: Plan,
    checked: &Checked,
    expected: Option<u64>,
    needs_consent: bool,
) -> u64 {
    let id = state.file_ops.add(
        caller.grant_id,
        caller.client,
        kind,
        plan,
        checked.path.clone(),
        expected,
    );
    if let (Some(op), Plan::Save { uid } | Plan::SaveAs { uid }) =
        (state.file_ops.get_mut(id), plan)
    {
        op.document = Some(uid);
    }
    if needs_consent {
        if let Some(op) = state.file_ops.get_mut(id) {
            op.state = OperationState::AwaitingConsent;
        }
        state.message = "An assistant wants to replace a file: see the question.".to_owned();
    } else {
        start(state, id);
    }
    id
}

pub fn open(
    state: &mut AppState,
    instance: &str,
    caller: &Caller,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: OpenArgs = parse(arguments)?;
    not_too_many(state, caller)?;
    let checked = check(&roots(state), &args.path, Access::Read, OPEN_EXTENSIONS)?;
    let id = state.file_ops.add(
        caller.grant_id,
        caller.client,
        Kind::Open,
        Plan::Open,
        checked.path.clone(),
        None,
    );
    let already = state
        .docs
        .iter()
        .find(|d| {
            d.path
                .as_deref()
                .is_some_and(|p| same_file(p, &checked.path))
        })
        .map(|d| d.uid);
    match already {
        // Reported as it is: the user's tab is not switched for them.
        Some(uid) => {
            if let Some(op) = state.file_ops.get_mut(id) {
                op.document = Some(uid);
            }
            state
                .file_ops
                .succeed(id, vec!["It was already open.".to_owned()]);
        }
        None => start(state, id),
    }
    status(state, instance, id)
}

pub fn save(
    state: &mut AppState,
    instance: &str,
    caller: &Caller,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: SaveArgs = parse(arguments)?;
    not_too_many(state, caller)?;
    let index = document_index(state, instance, &args.document_id, args.expected_revision)?;
    let doc = &state.docs[index];
    let uid = doc.uid;
    let own = doc.path.clone().ok_or_else(|| {
        invalid("This drawing has no file yet, and a file is never invented: use save_as.")
    })?;
    let checked = check(
        &roots(state),
        &own.to_string_lossy(),
        Access::Write,
        &["odx"],
    )?;
    let id = begin(
        state,
        caller,
        Kind::Save,
        Plan::Save { uid },
        &checked,
        args.expected_revision.map(|r| r.0),
        false,
    );
    status(state, instance, id)
}

pub fn save_as(
    state: &mut AppState,
    instance: &str,
    caller: &Caller,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: SaveAsArgs = parse(arguments)?;
    not_too_many(state, caller)?;
    let index = document_index(state, instance, &args.document_id, args.expected_revision)?;
    let checked = check(&roots(state), &args.path, Access::Write, &["odx"])?;
    let doc = &state.docs[index];
    // Saving over the drawing's own file is just a save.
    let own = doc
        .path
        .as_deref()
        .is_some_and(|p| same_file(p, &checked.path));
    let id = begin(
        state,
        caller,
        Kind::SaveAs,
        Plan::SaveAs { uid: doc.uid },
        &checked,
        args.expected_revision.map(|r| r.0),
        checked.existed && !own,
    );
    status(state, instance, id)
}

pub fn export(
    state: &mut AppState,
    instance: &str,
    caller: &Caller,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: ExportArgs = parse(arguments)?;
    not_too_many(state, caller)?;
    let index = document_index(state, instance, &args.document_id, args.expected_revision)?;
    let checked = check(
        &roots(state),
        &args.path,
        Access::Write,
        &[args.format.extension()],
    )?;
    let uid = state.docs[index].uid;
    let plan = match (args.format, args.layout_id) {
        (ExportFormat::Dxf, None) => Plan::ExportDxf { uid },
        (ExportFormat::Dxf, Some(_)) => return Err(invalid("A DXF export has no layout.")),
        (_, None) => return Err(invalid("A PDF or SVG export needs layout_id.")),
        (_, Some(layout)) => {
            let layout = LayoutId(layout.0);
            if state.docs[index]
                .session
                .document()
                .layout(layout)
                .is_none()
            {
                return Err(ApiError::new(
                    ErrorCode::InvalidReference,
                    "No such layout.",
                ));
            }
            Plan::ExportSheet { uid, layout }
        }
    };
    let id = begin(
        state,
        caller,
        Kind::Export,
        plan,
        &checked,
        args.expected_revision.map(|r| r.0),
        checked.existed,
    );
    status(state, instance, id)
}

/// The operation `arguments` names, if this connection started it.
fn own_operation(
    state: &AppState,
    caller: &Caller,
    arguments: serde_json::Value,
) -> Result<u64, ApiError> {
    let args: OperationArgs = parse(arguments)?;
    args.operation_id
        .strip_prefix("op_")
        .and_then(|n| n.parse::<u64>().ok())
        .filter(|id| {
            state
                .file_ops
                .get(*id)
                .is_some_and(|o| o.grant_id == caller.grant_id)
        })
        .ok_or_else(|| invalid("No such operation."))
}

pub fn get(
    state: &mut AppState,
    instance: &str,
    caller: &Caller,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    super::command_runs::sync(state);
    let id = own_operation(state, caller, arguments)?;
    status(state, instance, id)
}

pub fn cancel(
    state: &mut AppState,
    instance: &str,
    caller: &Caller,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    super::command_runs::sync(state);
    let id = own_operation(state, caller, arguments)?;
    let Some(op) = state.file_ops.get(id) else {
        return Err(invalid("No such operation."));
    };
    let (job, plan, path, waiting) = (
        op.job,
        op.plan,
        op.path.clone(),
        op.state == OperationState::AwaitingConsent,
    );
    if let Plan::Command { serial, .. } = plan {
        // Esc, from outside: only if it is still this command.
        if state.command_serial == serial && state.command_running() {
            state.cancel_command();
        }
        super::command_runs::sync(state);
    } else if waiting {
        state.file_ops.cancel(id);
    } else if op.state == OperationState::Running {
        if let Some(job) = job {
            // Only this operation's job; the user's own file work goes on.
            state.cancel_file_job(job);
            forget(state, plan, &path);
        }
        state.file_ops.cancel(id);
        state.tell("Cancelled. Nothing was changed on disk.".to_owned());
    }
    status(state, instance, id)
}

/// Undo the "being opened / saved" bookkeeping of a job given up on.
fn forget(state: &mut AppState, plan: Plan, path: &Path) {
    match plan {
        Plan::Open => state.opening.retain(|p| !same_file(p, path)),
        Plan::Save { uid } | Plan::SaveAs { uid } => state.saving.retain(|u| *u != uid),
        Plan::ExportDxf { .. } | Plan::ExportSheet { .. } | Plan::Command { .. } => {}
    }
}

/// What the question to the user says: built only from what the app knows,
/// never from the drawing's text.
pub fn consent_text(state: &AppState) -> Option<(u64, String)> {
    let op = state.file_ops.awaiting()?;
    Some((
        op.id,
        format!(
            "\"{}\" wants to replace this file ({}):\n{}",
            op.client,
            op.kind.name().replace('_', " "),
            op.path.display()
        ),
    ))
}

/// The user's answer to the question `consent_text` asked.
pub fn answer_consent(state: &mut AppState, id: u64, allow: bool) {
    let waiting = state
        .file_ops
        .get(id)
        .is_some_and(|o| o.state == OperationState::AwaitingConsent);
    if !waiting {
        return;
    }
    if allow {
        start(state, id);
    } else {
        state.file_ops.fail(
            id,
            ErrorCode::ConsentRequired,
            "The user did not allow replacing that file.",
        );
        state.tell("Declined: the file was not replaced.".to_owned());
    }
}
