//! The "What's new" note on the Home page: a few plain-language changes for
//! one release, written into the program, shown once until dismissed. It is
//! local only: nothing is fetched for it, and the release page is opened only
//! when the user asks. (Telling the user a newer version exists is separate,
//! and off until they allow it: see `updates`.)
//!
//! The release maintainer adds an entry for a release with changes worth
//! announcing (capabilities and workflow, not refactors or dependency updates)
//! and leaves it out when there is nothing to say; see "Releasing" in
//! `CONTRIBUTING.md`.

/// One release's note.
pub struct Note {
    /// The exact `CARGO_PKG_VERSION` it is for.
    pub version: &'static str,
    pub title: &'static str,
    /// At most three.
    pub changes: &'static [&'static str],
}

const NOTES: &[Note] = &[
    Note {
        version: "0.1.1",
        title: "A tidier status bar and a Windows installer",
        changes: &[
            "The status bar has icon buttons in AutoCAD's order; right-click Object Snap to choose its modes.",
            "Windows has an installer as well as the zip: it installs for your user, with no administrator rights.",
        ],
    },
    Note {
        version: "0.1.0",
        title: "A Home page to start from",
        changes: &[
            "Recent drawings show a picture of the drawing, and can be pinned.",
            "Drawings saved before a crash are offered back on Home.",
            "Dimensions, leaders and PEDIT for editing polylines.",
        ],
    },
];

/// Where a release's notes are, opened only on a click.
const RELEASES: &str = "https://github.com/JamesHodgkins/OpenDraft/releases/tag/v";

/// The note for exactly `version`, if there is one.
pub fn note_for(version: &str) -> Option<&'static Note> {
    NOTES.iter().find(|n| n.version == version)
}

/// This build's note.
pub fn current() -> Option<&'static Note> {
    note_for(env!("CARGO_PKG_VERSION"))
}

pub fn release_url(note: &Note) -> String {
    format!("{RELEASES}{}", note.version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_short_and_each_version_has_one() {
        for note in NOTES {
            assert!(!note.title.is_empty());
            assert!(
                (1..=3).contains(&note.changes.len()),
                "{}: {} changes",
                note.version,
                note.changes.len()
            );
            assert_eq!(
                NOTES.iter().filter(|n| n.version == note.version).count(),
                1,
                "{} twice",
                note.version
            );
        }
    }

    #[test]
    fn a_note_is_found_by_the_exact_version_only() {
        assert!(note_for("0.1.0").is_some());
        assert!(note_for("0.1.0-beta.1").is_none());
        assert!(note_for("0.1").is_none());
        assert!(note_for("").is_none());
    }

    #[test]
    fn the_release_page_is_that_versions_tag() {
        let note = note_for("0.1.0").unwrap();
        assert_eq!(
            release_url(note),
            "https://github.com/JamesHodgkins/OpenDraft/releases/tag/v0.1.0"
        );
    }
}
