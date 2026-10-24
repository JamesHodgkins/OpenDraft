//! Which build this is. `CARGO_PKG_VERSION` alone cannot say: `main` carries
//! the last release's number until the next one is cut, so a build from
//! between releases would look like the release. A build counts as a release
//! only when it is a release profile build of the commit tagged `v<version>`
//! (`build.rs` finds that out); any other build says so and names its commit.

/// The Cargo version, e.g. `0.2.0` or `0.2.0-beta.1`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where a build sits in the release process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Beta,
    /// Built from any other commit, or with debug assertions.
    Dev,
}

/// `on_tag` is `None` when git could not say (a source archive), which is
/// taken as a release: nothing marks it as anything else.
pub fn channel_of(version: &str, on_tag: Option<bool>, debug: bool) -> Channel {
    if debug || on_tag == Some(false) {
        return Channel::Dev;
    }
    match version.split_once('-') {
        None => Channel::Stable,
        Some((_, pre)) if pre == "dev" || pre.starts_with("dev.") => Channel::Dev,
        Some(_) => Channel::Beta,
    }
}

/// The version as shown to people: plain for a release, with `-dev` and the
/// commit for any other build.
pub fn describe(version: &str, commit: &str, channel: Channel) -> String {
    match (channel, commit.is_empty()) {
        (Channel::Dev, true) => format!("{version}-dev"),
        (Channel::Dev, false) => format!("{version}-dev ({commit})"),
        _ => version.to_owned(),
    }
}

fn on_tag() -> Option<bool> {
    match env!("OD_ON_TAG") {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

pub fn channel() -> Channel {
    channel_of(VERSION, on_tag(), cfg!(debug_assertions))
}

/// This build, for the Home page.
pub fn display() -> String {
    describe(VERSION, env!("OD_COMMIT"), channel())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tagged_release_build_is_stable_or_beta_by_its_version() {
        assert_eq!(channel_of("0.2.0", Some(true), false), Channel::Stable);
        assert_eq!(channel_of("0.2.0-beta.1", Some(true), false), Channel::Beta);
        assert_eq!(channel_of("0.2.0-rc.1", Some(true), false), Channel::Beta);
    }

    #[test]
    fn anything_else_is_dev() {
        assert_eq!(channel_of("0.2.0", Some(false), false), Channel::Dev);
        assert_eq!(channel_of("0.2.0", Some(true), true), Channel::Dev);
        assert_eq!(channel_of("0.3.0-dev", Some(true), false), Channel::Dev);
        assert_eq!(channel_of("0.3.0-dev.2", None, false), Channel::Dev);
    }

    #[test]
    fn a_build_without_git_is_taken_as_its_version() {
        assert_eq!(channel_of("0.2.0", None, false), Channel::Stable);
    }

    #[test]
    fn only_dev_builds_are_marked() {
        assert_eq!(describe("0.2.0", "abc", Channel::Stable), "0.2.0");
        assert_eq!(
            describe("0.2.0-beta.1", "abc", Channel::Beta),
            "0.2.0-beta.1"
        );
        assert_eq!(
            describe("0.2.0", "abc1234", Channel::Dev),
            "0.2.0-dev (abc1234)"
        );
        assert_eq!(describe("0.2.0", "", Channel::Dev), "0.2.0-dev");
    }
}
