//! Remembering what a keyed operation did, so asking again changes nothing.
//!
//! A key belongs to one connection and one operation. The first success is
//! kept, with a hash of the arguments it was asked with; the same key and
//! arguments again gets that answer back (marked `replayed`), and the same
//! key with other arguments is refused rather than guessed at. Failures are
//! not kept: every failure leaves the drawing as it was, so trying again is
//! exactly what the caller wants. The cache is small and bounded; an old key
//! that has been pushed out simply runs again.

use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

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
    arguments: u64,
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

/// A hash of the arguments, for noticing that they changed. The map keys
/// are sorted, so the same arguments hash the same however they were
/// ordered. It guards against mistakes, not against a determined caller.
fn fingerprint(arguments: &Value) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(arguments)
        .unwrap_or_default()
        .hash(&mut hasher);
    hasher.finish()
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
