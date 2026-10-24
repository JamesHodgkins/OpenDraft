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
