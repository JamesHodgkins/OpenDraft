use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Stable domain error codes. Callers branch on these, never on messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    AppNotRunning,
    InstanceRequired,
    InstanceGone,
    DocumentNotFound,
    DocumentClosed,
    EntityNotFound,
    StaleRevision,
    CommandBusy,
    BlockEditorOpen,
    OperationBusy,
    InvalidGeometry,
    InvalidReference,
    DocumentInvalid,
    UnsupportedEntityWrite,
    CommandNotAutomatable,
    ScopeDenied,
    ConsentRequired,
    PathDenied,
    OverwriteDenied,
    PayloadTooLarge,
    ResultTooLarge,
    RateLimited,
    DeadlineExceeded,
    Cancelled,
    InvalidArguments,
    /// An `idempotency_key` was used again with different arguments.
    IdempotencyKeyReused,
    /// A file operation the app tried did not work (disk, format).
    OperationFailed,
    InternalError,
}

/// A structured domain error, as carried in a response and in MCP tool
/// error content.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: false,
            details: BTreeMap::new(),
        }
    }

    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    pub fn detail(mut self, key: &str, value: impl ToString) -> Self {
        self.details.insert(key.to_owned(), value.to_string());
        self
    }

    pub fn stale_revision(expected: u64, actual: u64) -> Self {
        Self::new(
            ErrorCode::StaleRevision,
            "The drawing changed after it was read.",
        )
        .retryable()
        .detail("expected_revision", expected)
        .detail("actual_revision", actual)
    }

    /// Recoverable only once the user closes the Block Editor, so there is
    /// deliberately no retry delay: a client must not poll.
    pub fn block_editor_open() -> Self {
        Self::new(
            ErrorCode::BlockEditorOpen,
            "A block is open in the Block Editor. Ask the user to save or discard it and run BCLOSE.",
        )
        .retryable()
        .detail("recovery", "ask_user_to_close_block_editor")
    }

    pub fn command_busy(command: &str) -> Self {
        Self::new(
            ErrorCode::CommandBusy,
            "A command is running in the drawing.",
        )
        .retryable()
        .detail("command", command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_snake_case_on_the_wire() {
        let json = serde_json::to_value(ApiError::stale_revision(41, 43)).unwrap();
        assert_eq!(json["code"], "stale_revision");
        assert_eq!(json["retryable"], true);
        assert_eq!(json["details"]["expected_revision"], "41");
        assert_eq!(json["details"]["actual_revision"], "43");
    }

    #[test]
    fn block_editor_open_has_no_retry_delay() {
        let error = ApiError::block_editor_open();
        assert!(error.retryable);
        assert!(!error.details.contains_key("retry_after_ms"));
        assert_eq!(error.details["recovery"], "ask_user_to_close_block_editor");
    }
}
