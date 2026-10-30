//! Idempotency keys: a caller that is not sure whether a change went through
//! (a timeout, a retry by its host) sends the same `idempotency_key` again
//! and gets the original answer instead of the change being made twice.
//!
//! The key travels inside an operation's arguments. The app takes it out
//! before the operation sees its arguments, so the argument types do not
//! carry it; the tool schemas add it for the operations that accept one.

use crate::ops;

/// The argument that carries the key.
pub const KEY_FIELD: &str = "idempotency_key";

/// Longest key.
pub const KEY_MAX: usize = 64;

/// Whether `operation` accepts a key: every operation that changes
/// something and would change it again if repeated. Reads need none, and
/// setting the selection or cancelling an operation gives the same result
/// however often it is repeated.
pub fn supports(operation: &str) -> bool {
    matches!(
        operation,
        ops::ENTITIES_APPLY_BATCH
            | ops::LAYERS_APPLY_BATCH
            | ops::HISTORY_UNDO
            | ops::HISTORY_REDO
            | ops::COMMANDS_INVOKE
            | ops::COMMANDS_START_INTERACTIVE
            | ops::SELECTION_REQUEST
            | ops::FILES_OPEN
            | ops::FILES_SAVE
            | ops::FILES_SAVE_AS
            | ops::FILES_EXPORT
    )
}

/// A key is 1 to 64 characters from a plain set, so it is safe to log and
/// to compare.
pub fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= KEY_MAX
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_short_and_plain() {
        for good in [
            "a",
            "retry-1",
            "2026-10-16T12:00:00.abc",
            "a_b.c:d",
            &"k".repeat(64),
        ] {
            assert!(valid_key(good), "{good}");
        }
        for bad in ["", " ", "a b", "a/b", "é", "a\n", &"k".repeat(65)] {
            assert!(!valid_key(bad), "{bad:?}");
        }
    }

    #[test]
    fn only_operations_that_would_repeat_their_effect_take_a_key() {
        assert!(supports(ops::ENTITIES_APPLY_BATCH));
        assert!(supports(ops::COMMANDS_INVOKE), "a toggle flips again");
        assert!(supports(ops::FILES_SAVE_AS));
        assert!(
            supports(ops::SELECTION_REQUEST),
            "a retry must find its own prompt, not be refused as busy"
        );
        for read in [
            ops::DOCUMENTS_LIST,
            ops::ENTITIES_QUERY,
            ops::VIEW_RENDER,
            ops::EVENTS_POLL,
        ] {
            assert!(!supports(read), "{read}");
        }
        assert!(
            !supports(ops::SELECTION_SET),
            "absolute, so repeating is harmless"
        );
        assert!(!supports(ops::OPERATIONS_CANCEL));
    }
}
