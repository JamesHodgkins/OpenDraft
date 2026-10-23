//! What every operation that changes the app must check first.

use od_api::{ApiError, ErrorCode};

use super::resolve;
use crate::state::AppState;

/// Who is asking, as far as policy is concerned.
pub struct Caller<'a> {
    pub grant_id: u64,
    /// What the connection called itself; shown beside questions, never
    /// trusted.
    pub client: &'a str,
    /// The user also let this connection undo what it did not make.
    pub history_any: bool,
}

/// Find the drawing `id` names and make sure nothing else is using it: no
/// command is running in it and (when `expected` is given) it is still at
/// the revision the caller read. Returns its index in `state.docs`.
pub(super) fn open_for_change(
    state: &AppState,
    instance: &str,
    id: &str,
    expected: Option<u64>,
) -> Result<usize, ApiError> {
    // Resolves the id and refuses a drawing in the Block Editor.
    let uid = resolve(state, instance, id)?.uid;
    let index = state
        .docs
        .iter()
        .position(|d| d.uid == uid)
        .ok_or_else(|| ApiError::new(ErrorCode::DocumentClosed, "The drawing was closed."))?;
    if let Some(runner) = state.runner.as_ref().filter(|_| state.run_doc == index) {
        return Err(ApiError::command_busy(runner.name()));
    }
    if let Some(expected) = expected {
        let actual = state.docs[index].session.revision();
        if expected != actual {
            return Err(ApiError::stale_revision(expected, actual));
        }
    }
    Ok(index)
}
