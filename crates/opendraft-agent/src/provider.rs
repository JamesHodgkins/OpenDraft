//! The model, as the loop sees it: one call in, one answer out.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::types::{ModelRequest, ModelResponse};

/// A flag the user's Stop sets and every wait in the loop looks at. Cloning
/// shares it.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Why a call failed, sorted by what to do about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderError {
    /// A timeout, a rate limit, a server fault: worth trying again.
    Transient(String),
    /// A bad key, a bad request: trying again changes nothing.
    Fatal(String),
    /// The service declined to answer.
    Refused(String),
}

impl ProviderError {
    pub fn message(&self) -> &str {
        match self {
            Self::Transient(m) | Self::Fatal(m) | Self::Refused(m) => m,
        }
    }
}

/// A model service. `complete` may block for a long time; the loop runs it
/// on a worker thread and stops waiting if the task is cancelled, so an
/// implementation should also look at `cancel` where it can.
pub trait Provider: Send + Sync {
    fn complete(
        &self,
        request: &ModelRequest,
        cancel: &CancelToken,
    ) -> Result<ModelResponse, ProviderError>;
}
