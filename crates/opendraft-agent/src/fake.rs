//! A scripted model, for tests of the loop and of everything that sits on
//! it. It says what it is told to, in order, and keeps every request it was
//! sent so a test can look at what the model would have seen.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::provider::{CancelToken, Provider, ProviderError};
use crate::types::{Message, ModelRequest, ModelResponse, Part, StopReason, ToolCall, Usage};

/// What the fake does on one call.
#[derive(Clone, Debug)]
pub enum Step {
    Reply(ModelResponse),
    Fail(ProviderError),
    /// Never answers (until cancelled, or a few seconds pass so a test's
    /// abandoned thread ends).
    Hang,
    /// Answers after a delay.
    After(Duration, Box<Step>),
}

impl Step {
    /// The model says something and stops.
    pub fn say(text: &str) -> Self {
        Self::reply(vec![Part::Text(text.to_owned())], StopReason::EndTurn)
    }

    /// The model calls one tool.
    pub fn call(id: &str, name: &str, arguments: Value) -> Self {
        Self::calls(vec![(id, name, arguments)])
    }

    /// The model calls several tools in one turn.
    pub fn calls(calls: Vec<(&str, &str, Value)>) -> Self {
        let parts = calls
            .into_iter()
            .map(|(id, name, arguments)| {
                Part::ToolCall(ToolCall {
                    id: id.to_owned(),
                    name: name.to_owned(),
                    arguments,
                })
            })
            .collect();
        Self::reply(parts, StopReason::ToolUse)
    }

    pub fn reply(parts: Vec<Part>, stop: StopReason) -> Self {
        Self::Reply(ModelResponse {
            message: Message::assistant(parts),
            usage: Usage {
                input: Some(100),
                output: Some(20),
            },
            stop,
        })
    }
}

#[derive(Default)]
pub struct FakeProvider {
    script: Mutex<VecDeque<Step>>,
    seen: Mutex<Vec<ModelRequest>>,
}

impl FakeProvider {
    pub fn new(steps: Vec<Step>) -> Self {
        Self {
            script: Mutex::new(steps.into()),
            seen: Mutex::new(Vec::new()),
        }
    }

    /// Every request received so far.
    pub fn requests(&self) -> Vec<ModelRequest> {
        self.seen.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Steps not yet used.
    pub fn remaining(&self) -> usize {
        self.script.lock().map(|s| s.len()).unwrap_or(0)
    }

    fn play(step: Step, cancel: &CancelToken) -> Result<ModelResponse, ProviderError> {
        match step {
            Step::Reply(response) => Ok(response),
            Step::Fail(error) => Err(error),
            Step::Hang => {
                let until = Instant::now() + Duration::from_secs(5);
                while !cancel.is_cancelled() && Instant::now() < until {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(ProviderError::Transient("the fake gave up waiting".into()))
            }
            Step::After(delay, next) => {
                thread::sleep(delay);
                Self::play(*next, cancel)
            }
        }
    }
}

impl Provider for FakeProvider {
    fn complete(
        &self,
        request: &ModelRequest,
        cancel: &CancelToken,
    ) -> Result<ModelResponse, ProviderError> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(request.clone());
        }
        let step = self.script.lock().ok().and_then(|mut s| s.pop_front());
        match step {
            Some(step) => Self::play(step, cancel),
            None => Err(ProviderError::Fatal("the fake's script ran out".into())),
        }
    }
}
