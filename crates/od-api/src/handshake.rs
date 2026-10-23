//! The first exchange on a connection: the sidecar proves it holds the
//! instance's token and the two sides pick an API version. Nothing else is
//! accepted before it succeeds.

use serde::{Deserialize, Serialize};

use crate::{ApiError, Scope};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub token: String,
    pub api_versions: Vec<u32>,
    /// Shown in the app and the audit view; never used to look up a grant.
    pub client_name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HelloReply {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_version: Option<u32>,
    /// Identifies this connection's grant until it closes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant_id: Option<String>,
    #[serde(default)]
    pub scopes: Vec<Scope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

/// Compare secrets without stopping at the first difference.
pub fn tokens_match(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_compare_exactly() {
        assert!(tokens_match("abc", "abc"));
        assert!(!tokens_match("abc", "abd"));
        assert!(!tokens_match("abc", "abcd"));
        assert!(!tokens_match("", "a"));
        assert!(tokens_match("", ""));
    }

    #[test]
    fn a_hello_with_extra_fields_is_refused() {
        let json =
            r#"{"token":"t","api_versions":[1],"client_name":"x","scopes":["drawing.write"]}"#;
        assert!(serde_json::from_str::<Hello>(json).is_err());
    }
}
