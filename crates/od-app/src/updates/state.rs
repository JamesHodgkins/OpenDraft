//! When to look for a newer version, what was found, and what the user did
//! about it. Nothing here touches the network on the thread that draws: a
//! check runs on its own thread and its answer is collected on a later frame.
//! Times are passed in (seconds since 1970) so the rules can be tested.

use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;

use semver::Version;
use serde::{Deserialize, Serialize};

use super::fetch::{Fetched, Fetcher};
use super::releases;
use crate::build_info::Channel;

/// The storage key holding [`Saved`].
const KEY: &str = "update_state";
/// A check is made at most this often (24 hours).
const CHECK_EVERY: u64 = 24 * 3600;
/// After a failed check (offline, rate limited), the next try waits this long.
const RETRY_AFTER_FAILURE: u64 = 3600;
/// "Remind me later" hides the notice this long (3 days).
const REMIND_AFTER: u64 = 3 * 24 * 3600;

/// What is kept between runs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    /// When a check last got an answer.
    pub last_checked: Option<u64>,
    /// When a check was last tried, even if it failed.
    pub last_attempt: Option<u64>,
    /// GitHub's tag for the list as last read, so an unchanged list costs no
    /// quota and no download.
    pub etag: Option<String>,
    /// The newest version found above the one that found it.
    pub latest: Option<String>,
    /// A version the user chose to ignore.
    pub skipped: Option<String>,
    /// The notice stays hidden until this time.
    pub remind_after: Option<u64>,
}

/// What the Home page and the title bar show.
#[derive(Clone, Debug, PartialEq)]
pub enum View {
    /// This build never checks (a development build).
    Disabled,
    /// The user has not been asked.
    Unasked,
    /// The user said no.
    Off,
    Checking,
    /// `checked` is when it last got an answer.
    Current {
        checked: Option<u64>,
    },
    /// The last attempt failed: offline, or GitHub would not answer.
    Offline {
        checked: Option<u64>,
    },
    Available {
        version: Version,
    },
}

type Answer = Result<Fetched, String>;

pub struct Updates {
    saved: Saved,
    fetcher: Arc<dyn Fetcher>,
    current: Version,
    channel: Channel,
    /// Whether this build may check at all.
    enabled_here: bool,
    inflight: Option<Receiver<Answer>>,
    /// "Check now" was pressed.
    forced: bool,
    last_failed: bool,
}

impl Updates {
    /// For this build. A development build never checks, unless the address
    /// was replaced (`OD_UPDATES_URL`) to try it.
    pub fn new(saved: Saved) -> Self {
        let channel = crate::build_info::channel();
        let current = Version::parse(crate::build_info::VERSION).unwrap_or(Version::new(0, 0, 0));
        let enabled = channel != Channel::Dev || super::fetch::Github::is_overridden();
        Self::with(
            saved,
            Arc::new(super::fetch::Github::new()),
            current,
            channel,
            enabled,
        )
    }

    pub fn with(
        saved: Saved,
        fetcher: Arc<dyn Fetcher>,
        current: Version,
        channel: Channel,
        enabled_here: bool,
    ) -> Self {
        Self {
            saved,
            fetcher,
            current,
            channel,
            enabled_here,
            inflight: None,
            forced: false,
            last_failed: false,
        }
    }

    pub fn load(storage: Option<&dyn eframe::Storage>) -> Saved {
        storage
            .and_then(|s| s.get_string(KEY))
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        if let Ok(text) = serde_json::to_string(&self.saved) {
            storage.set_string(KEY, text);
        }
    }

    #[cfg(test)]
    pub fn saved(&self) -> &Saved {
        &self.saved
    }

    /// Whether a check is due: never made, a day old, or retried an hour
    /// after a failure. A clock set back counts as due.
    fn due(&self, now: u64) -> bool {
        if self.last_failed {
            return self
                .saved
                .last_attempt
                .is_none_or(|t| t > now || now - t >= RETRY_AFTER_FAILURE);
        }
        self.saved
            .last_checked
            .is_none_or(|t| t > now || now - t >= CHECK_EVERY)
    }

    /// Collect a finished check and, when allowed and due, start one. Call
    /// every frame; it is cheap. `allowed` is the user's choice.
    pub fn tick(&mut self, ctx: &eframe::egui::Context, allowed: Option<bool>, now: u64) {
        self.collect(now);
        if allowed != Some(true) || !self.enabled_here || self.inflight.is_some() {
            return;
        }
        if self.forced || self.due(now) {
            self.forced = false;
            self.start(ctx);
        }
    }

    fn start(&mut self, ctx: &eframe::egui::Context) {
        let (tx, rx) = channel();
        let fetcher = Arc::clone(&self.fetcher);
        let etag = self.saved.etag.clone();
        let wake = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("update-check".into())
            .spawn(move || {
                let _ = tx.send(fetcher.fetch(etag.as_deref()));
                wake.request_repaint();
            });
        if spawned.is_ok() {
            self.inflight = Some(rx);
        }
    }

    /// Take in the answer if the check has finished.
    fn collect(&mut self, now: u64) {
        let Some(rx) = &self.inflight else { return };
        let answer = match rx.try_recv() {
            Ok(answer) => answer,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("the check stopped".to_owned()),
        };
        self.inflight = None;
        self.saved.last_attempt = Some(now);
        self.last_failed = false;
        match answer {
            Ok(Fetched::NotModified) => self.saved.last_checked = Some(now),
            Ok(Fetched::Body { text, etag }) => {
                let found = releases::newest(
                    &text,
                    &self.current,
                    self.channel == Channel::Beta,
                    cfg!(windows),
                );
                match found {
                    Ok(newest) => {
                        self.saved.latest = newest.map(|v| v.to_string());
                        self.saved.etag = etag;
                        self.saved.last_checked = Some(now);
                    }
                    Err(_) => self.last_failed = true,
                }
            }
            Err(_) => self.last_failed = true,
        }
    }

    /// Look now, whatever the time since the last check.
    pub fn check_now(&mut self) {
        self.forced = true;
        self.last_failed = false;
    }

    /// Ignore this version; a newer one is announced again.
    pub fn skip(&mut self, version: &Version) {
        self.saved.skipped = Some(version.to_string());
    }

    pub fn remind_later(&mut self, now: u64) {
        self.saved.remind_after = Some(now + REMIND_AFTER);
    }

    /// The newest version found that is above this one. A version found by an
    /// older program that this one has since overtaken is not offered.
    fn newer(&self) -> Option<Version> {
        let found = Version::parse(self.saved.latest.as_deref()?).ok()?;
        (found > self.current).then_some(found)
    }

    /// An update to announce: found, not skipped, and not put off.
    pub fn announced(&self, now: u64) -> Option<Version> {
        let found = self.newer()?;
        let skipped = self.saved.skipped.as_deref() == Some(found.to_string().as_str());
        let put_off = self.saved.remind_after.is_some_and(|t| now < t);
        (!skipped && !put_off).then_some(found)
    }

    pub fn view(&self, allowed: Option<bool>, now: u64) -> View {
        if !self.enabled_here {
            return View::Disabled;
        }
        match allowed {
            None => return View::Unasked,
            Some(false) => return View::Off,
            Some(true) => {}
        }
        if let Some(version) = self.announced(now) {
            return View::Available { version };
        }
        let checked = self.saved.last_checked;
        if self.inflight.is_some() {
            View::Checking
        } else if self.last_failed {
            View::Offline { checked }
        } else {
            View::Current { checked }
        }
    }
}

/// Seconds since 1970; 0 if the clock is before it.
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    struct Fake {
        answers: Mutex<Vec<Answer>>,
        etags_seen: Mutex<Vec<Option<String>>>,
    }

    impl Fake {
        fn new(answers: Vec<Answer>) -> Arc<Self> {
            Arc::new(Self {
                answers: Mutex::new(answers),
                etags_seen: Mutex::new(Vec::new()),
            })
        }
        fn calls(&self) -> usize {
            self.etags_seen.lock().unwrap().len()
        }
    }

    impl Fetcher for Fake {
        fn fetch(&self, etag: Option<&str>) -> Answer {
            self.etags_seen
                .lock()
                .unwrap()
                .push(etag.map(str::to_owned));
            self.answers.lock().unwrap().remove(0)
        }
    }

    fn list(version: &str) -> Answer {
        let v = Version::parse(version).unwrap();
        let text = serde_json::json!([{
            "tag_name": format!("v{version}"),
            "draft": false,
            "prerelease": false,
            "assets": [
                { "name": releases::package_name(&v, true) },
                { "name": releases::package_name(&v, false) },
            ],
        }])
        .to_string();
        Ok(Fetched::Body {
            text,
            etag: Some("\"abc\"".to_owned()),
        })
    }

    fn updates(fake: &Arc<Fake>, saved: Saved) -> Updates {
        let fetcher: Arc<dyn Fetcher> = fake.clone();
        Updates::with(saved, fetcher, Version::new(0, 1, 0), Channel::Stable, true)
    }

    /// Run a tick and wait for its check to be collected.
    fn run(u: &mut Updates, allowed: Option<bool>, now: u64) {
        let ctx = eframe::egui::Context::default();
        u.tick(&ctx, allowed, now);
        let start = Instant::now();
        while u.inflight.is_some() && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
            u.tick(&ctx, allowed, now);
        }
    }

    const T: u64 = 1_000_000;

    #[test]
    fn nothing_is_fetched_until_the_user_has_said_yes() {
        let fake = Fake::new(vec![list("0.2.0")]);
        let mut u = updates(&fake, Saved::default());
        run(&mut u, None, T);
        run(&mut u, Some(false), T);
        assert_eq!(fake.calls(), 0);
        assert_eq!(u.view(None, T), View::Unasked);
        assert_eq!(u.view(Some(false), T), View::Off);
        run(&mut u, Some(true), T);
        assert_eq!(fake.calls(), 1);
    }

    #[test]
    fn a_newer_release_is_announced_and_remembered() {
        let fake = Fake::new(vec![list("0.2.0")]);
        let mut u = updates(&fake, Saved::default());
        run(&mut u, Some(true), T);
        assert_eq!(
            u.view(Some(true), T),
            View::Available {
                version: Version::new(0, 2, 0)
            }
        );
        assert_eq!(u.saved().etag.as_deref(), Some("\"abc\""));
        assert_eq!(u.saved().last_checked, Some(T));
    }

    #[test]
    fn an_up_to_date_program_says_so_with_the_time() {
        let fake = Fake::new(vec![list("0.1.0")]);
        let mut u = updates(&fake, Saved::default());
        run(&mut u, Some(true), T);
        assert_eq!(u.view(Some(true), T), View::Current { checked: Some(T) });
    }

    #[test]
    fn a_check_is_made_once_a_day_and_the_etag_is_sent_back() {
        let fake = Fake::new(vec![list("0.1.0"), Ok(Fetched::NotModified)]);
        let mut u = updates(&fake, Saved::default());
        run(&mut u, Some(true), T);
        run(&mut u, Some(true), T + 3600);
        assert_eq!(fake.calls(), 1, "an hour later is too soon");
        run(&mut u, Some(true), T + CHECK_EVERY);
        assert_eq!(fake.calls(), 2);
        assert_eq!(
            fake.etags_seen.lock().unwrap()[1].as_deref(),
            Some("\"abc\"")
        );
        assert_eq!(u.saved().last_checked, Some(T + CHECK_EVERY));
    }

    #[test]
    fn a_failed_check_is_quiet_and_tried_again_after_an_hour() {
        let fake = Fake::new(vec![Err("offline".into()), list("0.2.0")]);
        let mut u = updates(&fake, Saved::default());
        run(&mut u, Some(true), T);
        assert_eq!(u.view(Some(true), T), View::Offline { checked: None });
        run(&mut u, Some(true), T + 60);
        assert_eq!(fake.calls(), 1);
        run(&mut u, Some(true), T + RETRY_AFTER_FAILURE);
        assert_eq!(fake.calls(), 2);
        assert!(matches!(
            u.view(Some(true), T + 7200),
            View::Available { .. }
        ));
    }

    #[test]
    fn text_that_is_not_a_releases_list_counts_as_a_failure() {
        let body = Ok(Fetched::Body {
            text: "{\"message\":\"API rate limit exceeded\"}".into(),
            etag: None,
        });
        let fake = Fake::new(vec![body]);
        let mut u = updates(&fake, Saved::default());
        run(&mut u, Some(true), T);
        assert_eq!(u.view(Some(true), T), View::Offline { checked: None });
        assert_eq!(u.saved().last_checked, None);
    }

    #[test]
    fn check_now_checks_even_when_a_check_was_just_made() {
        let fake = Fake::new(vec![list("0.1.0"), list("0.2.0")]);
        let mut u = updates(&fake, Saved::default());
        run(&mut u, Some(true), T);
        u.check_now();
        run(&mut u, Some(true), T + 5);
        assert_eq!(fake.calls(), 2);
        assert!(matches!(u.view(Some(true), T + 5), View::Available { .. }));
    }

    #[test]
    fn skipping_hides_that_version_only_and_remind_later_hides_for_a_while() {
        let saved = Saved {
            latest: Some("0.2.0".into()),
            last_checked: Some(T),
            ..Saved::default()
        };
        let fake = Fake::new(vec![]);
        let mut u = updates(&fake, saved);
        assert!(
            u.announced(T).is_some(),
            "found in an earlier run, no network needed"
        );
        u.remind_later(T);
        assert!(u.announced(T + 60).is_none());
        assert!(u.announced(T + REMIND_AFTER).is_some());
        u.skip(&Version::new(0, 2, 0));
        assert!(u.announced(T + REMIND_AFTER).is_none());
        u.saved.latest = Some("0.3.0".into());
        assert!(
            u.announced(T + REMIND_AFTER).is_some(),
            "a newer one is announced again"
        );
    }

    #[test]
    fn a_version_the_program_has_overtaken_is_not_offered() {
        let saved = Saved {
            latest: Some("0.1.0".into()),
            ..Saved::default()
        };
        let u = updates(&Fake::new(vec![]), saved);
        assert_eq!(u.announced(T), None);
    }

    #[test]
    fn a_build_that_may_not_check_never_does() {
        let fake = Fake::new(vec![list("0.2.0")]);
        let fetcher: Arc<dyn Fetcher> = fake.clone();
        let mut u = Updates::with(
            Saved::default(),
            fetcher,
            Version::new(0, 1, 0),
            Channel::Dev,
            false,
        );
        run(&mut u, Some(true), T);
        assert_eq!(fake.calls(), 0);
        assert_eq!(u.view(Some(true), T), View::Disabled);
    }

    #[test]
    fn a_clock_set_back_does_not_stop_checks() {
        let saved = Saved {
            last_checked: Some(T + 10 * CHECK_EVERY),
            ..Saved::default()
        };
        let fake = Fake::new(vec![list("0.1.0")]);
        let mut u = updates(&fake, saved);
        run(&mut u, Some(true), T);
        assert_eq!(fake.calls(), 1);
    }

    #[test]
    fn saved_state_survives_a_round_trip_and_unknown_text_is_ignored() {
        let saved = Saved {
            latest: Some("0.2.0".into()),
            skipped: Some("0.1.5".into()),
            ..Saved::default()
        };
        let text = serde_json::to_string(&saved).unwrap();
        assert_eq!(serde_json::from_str::<Saved>(&text).unwrap(), saved);
        let from_newer: Saved = serde_json::from_str("{\"latest\":\"0.2.0\",\"extra\":1}").unwrap();
        assert_eq!(from_newer.latest.as_deref(), Some("0.2.0"));
    }
}
