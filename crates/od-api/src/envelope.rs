use serde::{Deserialize, Serialize};

use crate::limits::REQUEST_DEADLINE_MS;
use crate::ApiError;

/// A sidecar-to-app message. `operation` names what to do, `arguments` is
/// that operation's own payload, and `scope` is the grant it needs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub api_version: String,
    pub request_id: String,
    #[serde(default = "default_deadline")]
    pub deadline_ms: u64,
    pub scope: Scope,
    pub operation: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
}

fn default_deadline() -> u64 {
    REQUEST_DEADLINE_MS
}

/// The permissions a connection can be granted (spec section 8.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Scope {
    #[serde(rename = "drawing.read")]
    DrawingRead,
    #[serde(rename = "drawing.write")]
    DrawingWrite,
    #[serde(rename = "ui.selection")]
    UiSelection,
    #[serde(rename = "history.any")]
    HistoryAny,
    #[serde(rename = "command.invoke")]
    CommandInvoke,
    #[serde(rename = "file.read")]
    FileRead,
    #[serde(rename = "file.write")]
    FileWrite,
}

/// The reply to one request: exactly one of a result or an error.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub request_id: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

impl Response {
    pub fn success(request_id: &str, result: serde_json::Value) -> Self {
        Self {
            request_id: request_id.to_owned(),
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn failure(request_id: &str, error: ApiError) -> Self {
        Self {
            request_id: request_id.to_owned(),
            ok: false,
            result: None,
            error: Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCode;

    #[test]
    fn a_request_parses_with_defaults_and_rejects_unknown_fields() {
        let json = r#"{"api_version":"1","request_id":"r1","scope":"drawing.read","operation":"documents.list"}"#;
        let request: Request = serde_json::from_str(json).unwrap();
        assert_eq!(request.scope, Scope::DrawingRead);
        assert_eq!(request.deadline_ms, REQUEST_DEADLINE_MS);
        let extra = r#"{"api_version":"1","request_id":"r1","scope":"drawing.read","operation":"x","force":true}"#;
        assert!(serde_json::from_str::<Request>(extra).is_err());
        let unknown = r#"{"api_version":"1","request_id":"r1","scope":"root","operation":"x"}"#;
        assert!(serde_json::from_str::<Request>(unknown).is_err());
    }

    #[test]
    fn a_response_carries_a_result_or_an_error_never_both() {
        let ok = Response::success("r", serde_json::json!({"n": 1}));
        assert!(ok.ok && ok.error.is_none());
        let bad = Response::failure("r", ApiError::new(ErrorCode::InternalError, "x"));
        let json = serde_json::to_value(&bad).unwrap();
        assert!(!bad.ok && json.get("result").is_none());
    }
}
