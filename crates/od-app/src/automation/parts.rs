//! `parts.draw`: a standard part from its dimensions. The shapes are worked
//! out in `od_core::parts`; they are then added by `entities.apply_batch`
//! itself, so the revision check, the single undo step and the attribution to
//! the caller are exactly that tool's and cannot differ.

use od_api::parts_ops::{batch, PartArgs};
use od_api::{ApiError, ErrorCode};

use super::guard::Caller;
use super::mutate::apply_batch;
use super::read::parse;
use crate::state::AppState;

pub fn draw(
    state: &mut AppState,
    instance: &str,
    caller: &Caller,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: PartArgs = parse(arguments)?;
    let drawing = args
        .part
        .drawing()
        .map_err(|e| ApiError::new(ErrorCode::InvalidGeometry, e.0))?;
    apply_batch(state, instance, caller, batch(&args, &drawing))
}
