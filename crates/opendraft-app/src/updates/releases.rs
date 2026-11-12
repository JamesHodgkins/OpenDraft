//! Which GitHub release, if any, is a newer version this program could use.
//! Pure: it reads the text GitHub's releases list returned and decides.
//!
//! GitHub's own "latest release" is the newest by creation date, so a
//! backported patch published after a newer version would be "latest"; the
//! list is read instead and the highest version by semver rules wins. A
//! draft, a tag that is not `v<version>`, and a release without a package
//! for this system (the files `scripts/package.sh` makes) are skipped, so a
//! release still being uploaded is not announced.

use semver::Version;
use serde::Deserialize;

/// The page a release is read at. Built here from the version, never taken
/// from the downloaded text.
const RELEASE_PAGE: &str = "https://github.com/JamesHodgkins/OpenDraft/releases/tag/v";

pub fn release_page(version: &Version) -> String {
    format!("{RELEASE_PAGE}{version}")
}

/// The package `scripts/package.sh` makes for `version` on this system.
pub fn package_name(version: &Version, windows: bool) -> String {
    if windows {
        format!("OpenDraft-{version}-windows.zip")
    } else {
        format!("OpenDraft-{version}-linux.tar.gz")
    }
}

#[derive(Deserialize)]
struct Item {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
}

/// The highest version above `current` that has a package for this system.
/// A prerelease counts only when `prereleases` is set (a beta build is told of
/// newer betas; a release build only of releases). `Err` if the text is not a
/// list of releases at all.
pub fn newest(
    json: &str,
    current: &Version,
    prereleases: bool,
    windows: bool,
) -> Result<Option<Version>, String> {
    let list: Vec<serde_json::Value> =
        serde_json::from_str(json).map_err(|e| format!("not a list of releases: {e}"))?;
    let newest = list
        .into_iter()
        .filter_map(|item| serde_json::from_value::<Item>(item).ok())
        .filter(|item| !item.draft)
        .filter_map(|item| {
            let version = Version::parse(item.tag_name.strip_prefix('v')?).ok()?;
            let wanted = package_name(&version, windows);
            let early = item.prerelease || !version.pre.is_empty();
            let usable = (prereleases || !early) && item.assets.iter().any(|a| a.name == wanted);
            usable.then_some(version)
        })
        .filter(|v| v > current)
        .max();
    Ok(newest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    /// A release as GitHub lists it.
    fn release(tag: &str, draft: bool, prerelease: bool, assets: &[&str]) -> serde_json::Value {
        serde_json::json!({
            "tag_name": tag,
            "draft": draft,
            "prerelease": prerelease,
            "assets": assets.iter().map(|n| serde_json::json!({ "name": n })).collect::<Vec<_>>(),
        })
    }

    fn list(items: &[serde_json::Value]) -> String {
        serde_json::Value::Array(items.to_vec()).to_string()
    }

    fn full(tag: &str) -> serde_json::Value {
        let version = v(tag.trim_start_matches('v'));
        release(
            tag,
            false,
            !version.pre.is_empty(),
            &[
                &package_name(&version, true),
                &package_name(&version, false),
                "SHA256SUMS",
            ],
        )
    }

    #[test]
    fn the_highest_version_wins_whatever_the_order_of_the_list() {
        let json = list(&[full("v0.1.5"), full("v0.2.0"), full("v0.1.9")]);
        assert_eq!(
            newest(&json, &v("0.1.0"), false, true),
            Ok(Some(v("0.2.0")))
        );
        // A backport published last is listed first, and still loses.
        let json = list(&[full("v0.1.5"), full("v0.2.0")]);
        assert_eq!(
            newest(&json, &v("0.1.0"), false, false),
            Ok(Some(v("0.2.0")))
        );
    }

    #[test]
    fn nothing_newer_means_none() {
        let json = list(&[full("v0.1.0"), full("v0.0.9")]);
        assert_eq!(newest(&json, &v("0.1.0"), false, true), Ok(None));
        assert_eq!(newest("[]", &v("0.1.0"), false, true), Ok(None));
    }

    #[test]
    fn a_draft_a_prerelease_and_a_bad_tag_are_not_offered_to_a_release_build() {
        let json = list(&[
            release("v0.9.0", true, false, &["OpenDraft-0.9.0-windows.zip"]),
            full("v0.5.0-beta.1"),
            release("nightly", false, false, &[]),
            release("v1", false, false, &[]),
            full("v0.2.0"),
        ]);
        assert_eq!(
            newest(&json, &v("0.1.0"), false, true),
            Ok(Some(v("0.2.0")))
        );
    }

    #[test]
    fn a_beta_build_is_told_of_newer_betas_too_and_a_release_beats_its_own_beta() {
        let json = list(&[full("v0.2.0-beta.2"), full("v0.2.0-beta.1")]);
        assert_eq!(
            newest(&json, &v("0.2.0-beta.1"), true, true),
            Ok(Some(v("0.2.0-beta.2")))
        );
        assert_eq!(newest(&json, &v("0.2.0-beta.1"), false, true), Ok(None));
        let json = list(&[full("v0.2.0-beta.2"), full("v0.2.0")]);
        assert_eq!(
            newest(&json, &v("0.2.0-beta.1"), true, true),
            Ok(Some(v("0.2.0")))
        );
    }

    #[test]
    fn a_release_without_a_package_for_this_system_is_not_announced_yet() {
        let only_windows = release("v0.3.0", false, false, &["OpenDraft-0.3.0-windows.zip"]);
        let json = list(&[only_windows, full("v0.2.0")]);
        assert_eq!(
            newest(&json, &v("0.1.0"), false, true),
            Ok(Some(v("0.3.0")))
        );
        assert_eq!(
            newest(&json, &v("0.1.0"), false, false),
            Ok(Some(v("0.2.0")))
        );
    }

    #[test]
    fn text_that_is_not_a_list_is_an_error_and_one_bad_item_costs_only_itself() {
        assert!(newest("{\"message\":\"rate limited\"}", &v("0.1.0"), false, true).is_err());
        assert!(newest("not json", &v("0.1.0"), false, true).is_err());
        let json = format!("[7, {{\"tag_name\": 3}}, {}]", full("v0.2.0"));
        assert_eq!(
            newest(&json, &v("0.1.0"), false, true),
            Ok(Some(v("0.2.0")))
        );
    }

    #[test]
    fn the_page_and_package_names_are_built_from_the_version() {
        assert_eq!(
            release_page(&v("0.2.0-beta.1")),
            "https://github.com/JamesHodgkins/OpenDraft/releases/tag/v0.2.0-beta.1"
        );
        assert_eq!(
            package_name(&v("0.2.0"), true),
            "OpenDraft-0.2.0-windows.zip"
        );
        assert_eq!(
            package_name(&v("0.2.0"), false),
            "OpenDraft-0.2.0-linux.tar.gz"
        );
    }
}
