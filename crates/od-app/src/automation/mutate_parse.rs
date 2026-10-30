//! Reading a batch request, and saying which operation is wrong when one is.
//! A caller that is told only "missing field" has to guess where.

use od_api::write::{BatchArgs, Operation};
use od_api::{ApiError, ErrorCode};
use serde_json::Value;

use super::read::parse;

const FIELDS: &str = "add takes client_ref and entity; patch takes target and patch; \
                      replace takes target and entity; delete takes target";

pub(super) fn parse_batch(arguments: Value) -> Result<BatchArgs, ApiError> {
    let operations = arguments
        .get("operations")
        .and_then(Value::as_array)
        .cloned();
    let error = match parse::<BatchArgs>(arguments) {
        Ok(args) => return Ok(args),
        Err(error) => error,
    };
    for (index, operation) in operations.iter().flatten().enumerate() {
        let Err(inner) = serde_json::from_value::<Operation>(operation.clone()) else {
            continue;
        };
        let kind = operation.get("op").and_then(Value::as_str).unwrap_or("?");
        let mut message = format!("operation {index} (op {kind:?}): {inner}. {FIELDS}.");
        if inner.to_string().contains("missing field `type`") {
            message.push_str(" A geometry always names its type, even in a patch.");
        }
        return Err(ApiError::new(ErrorCode::InvalidArguments, message).detail("operation", index));
    }
    Err(error)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn batch(operations: Value) -> Value {
        json!({"document_id": "d", "expected_revision": "1", "operations": operations})
    }

    #[test]
    fn the_wrong_operation_is_named_with_what_each_kind_takes() {
        let error = parse_batch(batch(json!([
            {"op": "delete", "target": {"entity_id": "1"}},
            {"op": "replace", "client_ref": "x", "target": {"entity_id": "2"}},
        ])))
        .unwrap_err();
        assert!(
            error.message.contains("operation 1 (op \"replace\")"),
            "{error:?}"
        );
        assert!(
            error.message.contains("replace takes target and entity"),
            "{error:?}"
        );
        assert_eq!(
            error.details.get("operation").map(String::as_str),
            Some("1")
        );
    }

    #[test]
    fn a_geometry_patch_without_a_type_is_told_why() {
        let error = parse_batch(batch(json!([
            {"op": "patch", "target": {"entity_id": "1"},
             "patch": {"geometry": {"end": {"x": 1, "y": 2}}}},
        ])))
        .unwrap_err();
        assert!(error.message.contains("names its type"), "{error:?}");
    }

    #[test]
    fn a_good_batch_and_a_bad_envelope_are_as_before() {
        assert!(parse_batch(batch(
            json!([{"op": "delete", "target": {"entity_id": "1"}}])
        ))
        .is_ok());
        let error = parse_batch(json!({"operations": []})).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArguments);
        assert!(!error.message.contains("operation 0"));
    }
}
