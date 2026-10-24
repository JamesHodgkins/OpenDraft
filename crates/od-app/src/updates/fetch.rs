//! Asking GitHub for the releases list. Blocking: it runs on its own thread
//! (see `state.rs`), never on the one that draws. It reads nothing but the
//! list, sends nothing but the program's version and the usual details of any
//! web request, and gives up after a few seconds.

use std::time::Duration;

/// The releases list of this project, newest first (at most 20).
const RELEASES_URL: &str =
    "https://api.github.com/repos/JamesHodgkins/OpenDraft/releases?per_page=20";
/// A developer's override, for trying the check against a local server.
pub const URL_ENV: &str = "OD_UPDATES_URL";
const TIMEOUT: Duration = Duration::from_secs(10);
/// More than this is not a releases list.
const MAX_BYTES: u64 = 2 * 1024 * 1024;

/// What an answer was.
#[derive(Clone, Debug, PartialEq)]
pub enum Fetched {
    /// The list is as it was when `etag` was given.
    NotModified,
    Body {
        text: String,
        etag: Option<String>,
    },
}

/// Where the list comes from, so tests need no network.
pub trait Fetcher: Send + Sync {
    fn fetch(&self, etag: Option<&str>) -> Result<Fetched, String>;
}

pub struct Github {
    url: String,
}

impl Github {
    pub fn new() -> Self {
        let url = std::env::var(URL_ENV)
            .ok()
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| RELEASES_URL.to_owned());
        Self { url }
    }

    /// Whether the address was replaced for development.
    pub fn is_overridden() -> bool {
        std::env::var_os(URL_ENV).is_some_and(|u| !u.is_empty())
    }
}

impl Fetcher for Github {
    fn fetch(&self, etag: Option<&str>) -> Result<Fetched, String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .build()
            .into();
        let mut request = agent
            .get(&self.url)
            .header(
                "User-Agent",
                concat!("OpenDraft/", env!("CARGO_PKG_VERSION")),
            )
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(etag) = etag {
            request = request.header("If-None-Match", etag);
        }
        let mut response = request.call().map_err(|e| e.to_string())?;
        if response.status().as_u16() == 304 {
            return Ok(Fetched::NotModified);
        }
        let etag = response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let text = response
            .body_mut()
            .with_config()
            .limit(MAX_BYTES)
            .read_to_string()
            .map_err(|e| e.to_string())?;
        Ok(Fetched::Body { text, etag })
    }
}
