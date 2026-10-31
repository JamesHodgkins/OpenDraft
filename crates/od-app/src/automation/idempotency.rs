//! Remembering what a keyed operation did, so asking again changes nothing.
//!
//! A key belongs to one connection and one operation. The first success is
//! kept, with a digest of the arguments it was asked with; the same key and
//! arguments again gets that answer back (marked `replayed`), and the same
//! key with other arguments is refused rather than guessed at. Failures are
//! not kept: every failure leaves the drawing as it was, so trying again is
//! exactly what the caller wants. The cache is small and bounded; an old key
//! that has been pushed out simply runs again.

use std::collections::VecDeque;

use sha2::{Digest, Sha256};

use od_api::idempotency::{supports, valid_key, KEY_FIELD, KEY_MAX};
use od_api::{ApiError, ErrorCode};
use serde_json::Value;

/// Answers kept.
const KEPT: usize = 256;
/// A bigger answer is not worth keeping: the caller can look at the result.
const ANSWER_MAX: usize = 256 * 1024;

struct Entry {
    grant: u64,
    operation: String,
    key: String,
    arguments: Fingerprint,
    answer: Value,
}

#[derive(Default)]
pub struct Replays {
    entries: VecDeque<Entry>,
}

pub enum Seen {
    /// Done before with these arguments: here is what it said.
    Answer(Value),
    /// The key was used with different arguments.
    Reused,
    /// New.
    New,
}

/// Take the key out of `arguments` for an operation that accepts one.
/// Other operations are left alone: one that checks its arguments then
/// refuses the stray field with the other unknown ones.
pub fn take_key(
    operation: &str,
    mut arguments: Value,
) -> Result<(Value, Option<String>), ApiError> {
    if !supports(operation) {
        return Ok((arguments, None));
    }
    let Some(map) = arguments.as_object_mut() else {
        return Ok((arguments, None));
    };
    let Some(key) = map.remove(KEY_FIELD) else {
        return Ok((arguments, None));
    };
    match key.as_str().filter(|k| valid_key(k)) {
        Some(key) => {
            let key = key.to_owned();
            Ok((arguments, Some(key)))
        }
        None => Err(ApiError::new(
            ErrorCode::InvalidArguments,
            format!("{KEY_FIELD} must be 1 to {KEY_MAX} letters, digits, or any of _ - . :"),
        )),
    }
}

/// What the arguments came to, for noticing that they changed.
type Fingerprint = [u8; 32];

/// A SHA-256 digest of the arguments. The map keys are sorted, so the same
/// arguments give the same digest however they were ordered, and two
/// different sets of arguments do not share one. (A `Value` always
/// serializes, so the empty fallback is never used.) It is kept in place of the
/// arguments themselves, which can be a large batch.
fn fingerprint(arguments: &Value) -> Fingerprint {
    let text = serde_json::to_vec(arguments).unwrap_or_default();
    Sha256::digest(&text).into()
}

impl Replays {
    fn position(&self, grant: u64, operation: &str, key: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.grant == grant && e.operation == operation && e.key == key)
    }

    pub fn look(&self, grant: u64, operation: &str, key: &str, arguments: &Value) -> Seen {
        match self.position(grant, operation, key) {
            None => Seen::New,
            Some(at) if self.entries[at].arguments == fingerprint(arguments) => {
                let mut answer = self.entries[at].answer.clone();
                if let Some(map) = answer.as_object_mut() {
                    map.insert("replayed".to_owned(), Value::Bool(true));
                }
                Seen::Answer(answer)
            }
            Some(_) => Seen::Reused,
        }
    }

    /// Keep the answer to a success.
    pub fn keep(
        &mut self,
        grant: u64,
        operation: &str,
        key: &str,
        arguments: &Value,
        answer: &Value,
    ) {
        if serde_json::to_string(answer).map_or(true, |text| text.len() > ANSWER_MAX) {
            return;
        }
        if let Some(at) = self.position(grant, operation, key) {
            self.entries.remove(at);
        }
        if self.entries.len() >= KEPT {
            self.entries.pop_front();
        }
        self.entries.push_back(Entry {
            grant,
            operation: operation.to_owned(),
            key: key.to_owned(),
            arguments: fingerprint(arguments),
            answer: answer.clone(),
        });
    }
}

pub fn reused(key: &str) -> ApiError {
    ApiError::new(
        ErrorCode::IdempotencyKeyReused,
        "That idempotency_key was already used with different arguments. Use a new key for a different request.",
    )
    .detail("idempotency_key", key)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_same_arguments_have_the_same_digest_whatever_the_field_order() {
        let a = json!({"x": 1, "y": [1, 2, 3], "name": "a"});
        let b = json!({"name": "a", "y": [1, 2, 3], "x": 1});
        assert_eq!(fingerprint(&a), fingerprint(&b));
    }

    #[test]
    fn arguments_that_differ_in_the_smallest_way_have_different_digests() {
        let base = json!({"entities": [{"x": 1.0}, {"x": 2.0}]});
        for other in [
            json!({"entities": [{"x": 1.0}, {"x": 2.000001}]}),
            json!({"entities": [{"x": 2.0}, {"x": 1.0}]}),
            json!({"entities": [{"x": 1.0}]}),
            json!({"entities": [{"x": 1.0}, {"x": 2.0}], "extra": null}),
        ] {
            assert_ne!(fingerprint(&base), fingerprint(&other), "{other}");
        }
    }

    #[test]
    fn the_digest_does_not_grow_with_the_arguments() {
        let big = json!({"entities": vec![json!({"x": 1.5}); 50_000]});
        assert_eq!(fingerprint(&big).len(), 32);
        assert_eq!(std::mem::size_of::<Fingerprint>(), 32);
    }
}
