//! Whose files this run uses. Everything the app keeps (settings, window,
//! recent files, previews, recovery copies, templates) lives in one folder
//! named after the profile, so a debug build, a beta and the release can run on
//! one machine without overwriting each other's settings. The name is chosen
//! here and nowhere else: the window system's application name and every
//! path in the app come from [`name`] and [`app_data`].
//!
//! A release build uses `OpenDraft`, so `cargo run --release` tests the
//! release's own files. A debug build uses `OpenDraft Dev`, a beta
//! `OpenDraft Beta`. `OPENDRAFT_PROFILE=<name>` picks `OpenDraft <name>`
//! instead, for tests and for trying a migration on a copy. Drawings are not
//! part of a profile: a newer build can save a newer file format.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::build_info::{channel_of, Channel, VERSION};

/// Names a profile other than the build's own.
pub const PROFILE_ENV: &str = "OPENDRAFT_PROFILE";
/// The files a new beta profile starts with, copied once from the release's.
const SEEDED: [&str; 2] = ["app.ron", "window.json"];

/// A profile name from the environment, if it is a plain short name.
fn clean(requested: Option<&OsStr>) -> Option<&str> {
    let text = requested?.to_str()?.trim();
    let plain = text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_'));
    (!text.is_empty() && text.len() <= 32 && plain).then_some(text)
}

fn name_for(version: &str, debug: bool, requested: Option<&OsStr>) -> String {
    if let Some(profile) = clean(requested) {
        return format!("OpenDraft {profile}");
    }
    // The tag is ignored: a release build is the release's profile wherever
    // it was built from.
    match channel_of(version, Some(true), debug) {
        Channel::Stable => "OpenDraft".to_owned(),
        Channel::Beta => "OpenDraft Beta".to_owned(),
        Channel::Dev => "OpenDraft Dev".to_owned(),
    }
}

/// The profile's name: the application name and the window title.
pub fn name() -> &'static str {
    static NAME: OnceLock<String> = OnceLock::new();
    NAME.get_or_init(|| {
        name_for(
            VERSION,
            cfg!(debug_assertions),
            std::env::var_os(PROFILE_ENV).as_deref(),
        )
    })
}

/// The window's application id, which a Linux desktop matches against the menu
/// entry (`opendraft.desktop`, `StartupWMClass=opendraft`) to give the window
/// its icon and group it: the profile name in lower case, with `-` for spaces.
pub fn app_id() -> String {
    id_of(name())
}

fn id_of(name: &str) -> String {
    name.to_ascii_lowercase().replace(' ', "-")
}

/// The profile's folder, which holds the saved settings and the defaults for
/// the other folders.
pub fn app_data() -> Option<PathBuf> {
    eframe::storage_dir(name())
}

/// Copy the saved settings and window of the release to `to`, once, when
/// `to` has none yet.
fn seed(from: &Path, to: &Path) {
    if to.join(SEEDED[0]).exists() || !from.join(SEEDED[0]).exists() {
        return;
    }
    if std::fs::create_dir_all(to).is_err() {
        return;
    }
    for file in SEEDED {
        let _ = std::fs::copy(from.join(file), to.join(file));
    }
}

/// A beta's first run starts from the release's settings instead of the
/// defaults. Nothing is ever written back to the release's folder.
pub fn seed_beta_from_release() {
    if name() != "OpenDraft Beta" {
        return;
    }
    if let (Some(from), Some(to)) = (eframe::storage_dir("OpenDraft"), app_data()) {
        seed(&from, &to);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(s: &str) -> Option<&OsStr> {
        Some(OsStr::new(s))
    }

    #[test]
    fn each_build_has_its_own_profile() {
        assert_eq!(name_for("0.2.0", false, None), "OpenDraft");
        assert_eq!(name_for("0.2.0-beta.1", false, None), "OpenDraft Beta");
        assert_eq!(name_for("0.3.0-dev", false, None), "OpenDraft Dev");
        assert_eq!(name_for("0.2.0", true, None), "OpenDraft Dev");
        assert_eq!(name_for("0.2.0-beta.1", true, None), "OpenDraft Dev");
    }

    #[test]
    fn the_window_id_is_the_name_in_lower_case() {
        assert_eq!(id_of("OpenDraft"), "opendraft");
        assert_eq!(id_of("OpenDraft Dev"), "opendraft-dev");
        assert_eq!(id_of("OpenDraft Test 1"), "opendraft-test-1");
    }

    #[test]
    fn a_named_profile_wins_when_it_is_a_plain_name() {
        assert_eq!(name_for("0.2.0", false, os("Test 1")), "OpenDraft Test 1");
        assert_eq!(name_for("0.2.0", true, os("a-b_c")), "OpenDraft a-b_c");
        for bad in ["", "  ", "../x", "a/b", "a\\b", "a:b", &"x".repeat(33)] {
            assert_eq!(name_for("0.2.0", false, os(bad)), "OpenDraft", "{bad:?}");
        }
    }

    #[test]
    fn a_new_beta_starts_from_the_releases_settings_and_never_overwrites_its_own() {
        let root = std::env::temp_dir().join(format!("od-profile-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (release, beta) = (root.join("release"), root.join("beta"));
        std::fs::create_dir_all(&release).unwrap();
        seed(&release, &beta);
        assert!(!beta.exists(), "nothing to copy yet");
        std::fs::write(release.join("app.ron"), "release").unwrap();
        std::fs::write(release.join("window.json"), "window").unwrap();
        std::fs::create_dir_all(release.join("recovery")).unwrap();
        seed(&release, &beta);
        assert_eq!(
            std::fs::read_to_string(beta.join("app.ron")).unwrap(),
            "release"
        );
        assert_eq!(
            std::fs::read_to_string(beta.join("window.json")).unwrap(),
            "window"
        );
        assert!(!beta.join("recovery").exists());
        std::fs::write(beta.join("app.ron"), "beta's own").unwrap();
        seed(&release, &beta);
        assert_eq!(
            std::fs::read_to_string(beta.join("app.ron")).unwrap(),
            "beta's own"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn only_this_file_asks_eframe_where_the_files_are() {
        fn visit(dir: &Path, found: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    visit(&path, found);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && !path.ends_with("profile.rs")
                {
                    let text = std::fs::read_to_string(&path).unwrap();
                    if text.contains("eframe::storage_dir(") {
                        found.push(path);
                    }
                }
            }
        }
        let mut found = Vec::new();
        visit(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut found,
        );
        assert!(found.is_empty(), "use profile::app_data: {found:?}");
    }
}
