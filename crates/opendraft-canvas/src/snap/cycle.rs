//! Tab cycling: which of the snap points under the pointer is the chosen one.

use super::{SnapId, SnapPoint};

/// The snap point to hold on to after `step` presses of Tab (negative for
/// Shift+Tab, zero for none). With nothing chosen the best point is the one
/// shown, so the first press moves on from it; the choice wraps at the end. A
/// `current` that is no longer among `found` has been left behind by the
/// pointer, so it is dropped and the choice starts again from the best one.
pub(crate) fn cycled(found: &[SnapPoint], current: Option<SnapId>, step: i32) -> Option<SnapId> {
    let len = i64::try_from(found.len()).ok().filter(|n| *n > 0)?;
    let at = current.and_then(|c| found.iter().position(|f| f.id() == c));
    if step == 0 {
        return at.and(current);
    }
    let shown = i64::try_from(at.unwrap_or(0)).ok()?;
    let next = (shown + i64::from(step)).rem_euclid(len);
    found.get(usize::try_from(next).ok()?).map(SnapPoint::id)
}
