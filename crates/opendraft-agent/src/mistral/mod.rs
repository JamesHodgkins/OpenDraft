//! Mistral as a model provider: our request lowered to their chat-completions
//! form, one blocking HTTPS call, their answer raised back. This is the only
//! part of the crate that touches the network, and the only place the user's
//! key is held. The key is never written anywhere by this module: not into a
//! log, an error message (those are scrubbed of it), a recording or `Debug`.
//!
//! Non-streaming: the documented, whole response is enough until a screen
//! wants text as it arrives.

mod lower;
mod raise;

#[cfg(test)]
mod tests;

use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use serde_json::Value;

use crate::provider::{CancelToken, Provider, ProviderError};
use crate::types::{ModelRequest, ModelResponse};

pub const DEFAULT_BASE_URL: &str = "https://api.mistral.ai";
/// Supports function calling and pictures.
pub const DEFAULT_MODEL: &str = "mistral-large-latest";
/// Low, because the work is accuracy.
pub const DEFAULT_TEMPERATURE: f64 = 0.2;
/// How a refused key is worded in a failure, so a screen can ask for another.
pub const KEY_REFUSED: &str = "the model service did not accept the key";
/// The largest answer read: a runaway body is not held in memory.
const BODY_MAX: u64 = 16 * 1024 * 1024;

pub struct MistralProvider {
    key: String,
    base_url: String,
    model: String,
    temperature: f64,
    timeout: Duration,
    /// Where to keep each raw answer, for building test fixtures.
    record_to: Option<PathBuf>,
    recorded: AtomicU32,
}

impl fmt::Debug for MistralProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MistralProvider")
            .field("key", &"[hidden]")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .finish()
    }
}

impl MistralProvider {
    pub fn new(key: &str) -> Self {
        Self {
            key: key.trim().to_owned(),
            base_url: DEFAULT_BASE_URL.to_owned(),
            model: DEFAULT_MODEL.to_owned(),
            temperature: DEFAULT_TEMPERATURE,
            timeout: Duration::from_secs(120),
            record_to: None,
            recorded: AtomicU32::new(0),
        }
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_owned();
        self
    }

    /// Another address, for tests against a local server.
    pub fn with_base_url(mut self, url: &str) -> Self {
        self.base_url = url.trim_end_matches('/').to_owned();
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Keep each raw answer as `answer-N.json` in `dir`. Answers carry the
    /// drawing data the model was shown, never the key.
    pub fn recording_to(mut self, dir: PathBuf) -> Self {
        self.record_to = Some(dir);
        self
    }

    /// Text with the key taken out, for anything that might be shown.
    fn scrub(&self, text: &str) -> String {
        if self.key.is_empty() {
            return text.to_owned();
        }
        text.replace(&self.key, "[key]")
    }

    fn record(&self, body: &str) {
        let Some(dir) = &self.record_to else {
            return;
        };
        let n = self.recorded.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(dir.join(format!("answer-{n}.json")), body);
    }

    fn post(&self, body: &Value) -> Result<String, ProviderError> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(self.timeout))
            // Read the body of an error answer: it says what was wrong.
            .http_status_as_error(false)
            .build()
            .into();
        let payload = serde_json::to_string(body)
            .map_err(|e| ProviderError::Fatal(format!("the request could not be written: {e}")))?;
        let mut response = agent
            .post(&format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", &format!("Bearer {}", self.key))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header(
                "User-Agent",
                concat!("OpenDraft/", env!("CARGO_PKG_VERSION")),
            )
            .send(payload.as_bytes())
            .map_err(|e| self.transport(&e))?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .with_config()
            .limit(BODY_MAX)
            .read_to_string()
            .map_err(|e| self.transport(&e))?;
        if (200..300).contains(&status) {
            Ok(text)
        } else {
            Err(self.refusal(status, &text))
        }
    }

    fn transport(&self, error: &ureq::Error) -> ProviderError {
        let message = self.scrub(&error.to_string());
        match error {
            ureq::Error::BadUri(_) => ProviderError::Fatal(message),
            _ => ProviderError::Transient(format!("could not reach the model service: {message}")),
        }
    }

    /// A non-success answer, sorted by what to do about it.
    fn refusal(&self, status: u16, body: &str) -> ProviderError {
        let said = serde_json::from_str::<Value>(body)
            .ok()
            .and_then(|v| {
                v["message"]
                    .as_str()
                    .or_else(|| v["error"]["message"].as_str())
                    .or_else(|| v["detail"].as_str())
                    .map(str::to_owned)
            })
            .unwrap_or_default();
        let said: String = self.scrub(&said).chars().take(300).collect();
        let detail = if said.is_empty() {
            format!("status {status}")
        } else {
            format!("status {status}: {said}")
        };
        match status {
            401 | 403 => ProviderError::Fatal(format!("{KEY_REFUSED} ({detail})")),
            408 | 425 | 429 | 500..=599 => {
                ProviderError::Transient(format!("the model service is busy or down ({detail})"))
            }
            _ => ProviderError::Fatal(format!("the model service refused the request ({detail})")),
        }
    }
}

impl Provider for MistralProvider {
    fn complete(
        &self,
        request: &ModelRequest,
        cancel: &CancelToken,
    ) -> Result<ModelResponse, ProviderError> {
        if cancel.is_cancelled() {
            return Err(ProviderError::Fatal("stopped".to_owned()));
        }
        let body = lower::body(request, &self.model, self.temperature);
        let text = self.post(&body)?;
        self.record(&text);
        let value: Value = serde_json::from_str(&text).map_err(|e| {
            ProviderError::Fatal(format!("the model service sent something unreadable: {e}"))
        })?;
        raise::response(&value)
    }
}
