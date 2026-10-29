//! Asking the model: on a worker thread the loop can stop waiting for, with
//! a limit on the wait, and a few more tries when the failure is the kind
//! that passes. An HTTP call cannot be interrupted part way, so on cancel or
//! a timeout the answer, if it comes, is dropped and the call's own timeout
//! ends it; what the model already generated is still billed.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use crate::provider::ProviderError;
use crate::state::{Failure, TaskState};
use crate::task::Task;
use crate::types::{ModelRequest, ModelResponse};

const SLICE: Duration = Duration::from_millis(20);

impl Task<'_> {
    /// One answer from the model, or the state the task ends in instead.
    pub(crate) fn ask_model(&mut self, request: &ModelRequest) -> Result<ModelResponse, TaskState> {
        let tries = self.config.retry.tries.max(1);
        let mut wait = self.config.retry.base;
        for attempt in 1..=tries {
            if self.cancel.is_cancelled() {
                return Err(TaskState::Cancelled);
            }
            self.meter
                .charge_model_call()
                .map_err(|kind| TaskState::Failed(Failure::Budget(kind)))?;
            match self.ask_once(request)? {
                Ok(response) => return Ok(response),
                Err(ProviderError::Transient(message)) => {
                    if attempt == tries {
                        return Err(TaskState::Failed(Failure::Provider(message)));
                    }
                    self.sleep(wait)?;
                    wait = wait.saturating_mul(2);
                }
                Err(ProviderError::Fatal(message)) => {
                    return Err(TaskState::Failed(Failure::Provider(message)));
                }
                Err(ProviderError::Refused(message)) => {
                    return Err(TaskState::Failed(Failure::Refused(message)));
                }
            }
        }
        Err(TaskState::Failed(Failure::Internal(
            "no model call was made".to_owned(),
        )))
    }

    /// One try. The outer error ends the task; the inner is the provider's.
    fn ask_once(
        &self,
        request: &ModelRequest,
    ) -> Result<Result<ModelResponse, ProviderError>, TaskState> {
        let (sender, receiver) = mpsc::channel();
        let provider = self.provider.clone();
        let request = request.clone();
        let cancel = self.cancel.clone();
        let spawned = thread::Builder::new()
            .name("od-agent-model".to_owned())
            .spawn(move || {
                let _ = sender.send(provider.complete(&request, &cancel));
            });
        if spawned.is_err() {
            return Err(TaskState::Failed(Failure::Internal(
                "could not start the model call".to_owned(),
            )));
        }
        let call_limit = Instant::now() + self.meter.budgets().model_call;
        loop {
            match receiver.recv_timeout(SLICE) {
                Ok(answer) => return Ok(answer),
                Err(RecvTimeoutError::Disconnected) => {
                    return Ok(Err(ProviderError::Fatal(
                        "the model call ended without an answer".to_owned(),
                    )));
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
            if self.cancel.is_cancelled() {
                return Err(TaskState::Cancelled);
            }
            self.meter
                .time_left()
                .map_err(|kind| TaskState::Failed(Failure::Budget(kind)))?;
            if Instant::now() >= call_limit {
                return Ok(Err(ProviderError::Transient(
                    "the model did not answer in time".to_owned(),
                )));
            }
        }
    }

    /// Wait, but not past cancel or the task's time.
    fn sleep(&self, total: Duration) -> Result<(), TaskState> {
        let until = Instant::now() + total;
        while Instant::now() < until {
            if self.cancel.is_cancelled() {
                return Err(TaskState::Cancelled);
            }
            self.meter
                .time_left()
                .map_err(|kind| TaskState::Failed(Failure::Budget(kind)))?;
            thread::sleep(SLICE.min(until.saturating_duration_since(Instant::now())));
        }
        Ok(())
    }
}
