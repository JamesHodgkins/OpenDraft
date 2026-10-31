//! Telling the user a newer version exists. Off until the user allows it
//! (`Settings::update_checks`); then once a day, on a background thread, the
//! program reads GitHub's list of releases and compares it with its own
//! version. It only informs: nothing is downloaded or installed, and the page
//! opened on a click is built here from the version number. See
//! `private_material/release_and_update_plan.md` (phase 6).

mod fetch;
mod releases;
mod state;

pub use releases::release_page;
pub use state::{now_secs, Updates, View};

/// Whether a software store looks after updates, so the program must not offer
/// to. Inside a Flatpak the sandbox sets `FLATPAK_ID`, and Flathub updates the
/// app itself; a second, competing update notice there is confusing, and
/// Flathub asks apps not to have one.
pub fn handled_by_the_store() -> bool {
    in_flatpak(std::env::var_os("FLATPAK_ID").as_deref())
}

fn in_flatpak(flatpak_id: Option<&std::ffi::OsStr>) -> bool {
    flatpak_id.is_some_and(|id| !id.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flatpak_is_told_apart_by_its_id() {
        assert!(in_flatpak(Some("uk.openengineering.opendraft".as_ref())));
        assert!(!in_flatpak(None));
        assert!(!in_flatpak(Some("".as_ref())));
    }
}
