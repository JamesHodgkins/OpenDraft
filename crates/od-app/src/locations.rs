//! Where the app keeps its files. Every folder the user may move is a
//! [`Location`], chosen on the Settings dialog's Files page and stored in
//! [`Settings`]; one that is not set falls back to a folder beside the saved
//! settings. Code needing a folder asks here rather than building a path.

use std::path::PathBuf;

use crate::profile::app_data;
use crate::settings::Settings;

/// A folder the user can point somewhere else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    /// The user's own `.odt` files.
    Templates,
    /// Recovery copies of unsaved drawings.
    Recovery,
}

impl Location {
    pub const ALL: [Location; 2] = [Location::Templates, Location::Recovery];

    pub fn label(self) -> &'static str {
        match self {
            Self::Templates => "Template folder",
            Self::Recovery => "Recovery folder",
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            Self::Templates => "The .odt files listed when starting a new drawing.",
            Self::Recovery => {
                "Autosave copies of unsaved drawings. Takes effect the next time the app starts."
            }
        }
    }

    /// The folder chosen in `settings`, if any.
    pub fn chosen(self, settings: &Settings) -> Option<&PathBuf> {
        match self {
            Self::Templates => settings.templates_dir.as_ref(),
            Self::Recovery => settings.recovery_dir.as_ref(),
        }
    }

    pub fn set(self, settings: &mut Settings, folder: Option<PathBuf>) {
        match self {
            Self::Templates => settings.templates_dir = folder,
            Self::Recovery => settings.recovery_dir = folder,
        }
    }

    /// Where it is when nothing is chosen.
    pub fn default_folder(self) -> Option<PathBuf> {
        match self {
            Self::Templates => app_data().map(|d| d.join("templates")),
            Self::Recovery => app_data().map(|d| d.join("recovery")),
        }
    }

    /// The folder in use: the chosen one, else the default.
    pub fn resolve(self, settings: &Settings) -> Option<PathBuf> {
        self.chosen(settings)
            .cloned()
            .or_else(|| self.default_folder())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chosen_folder_wins_and_clearing_it_restores_the_default() {
        let mut settings = Settings::default();
        for location in Location::ALL {
            assert_eq!(location.resolve(&settings), location.default_folder());
            location.set(&mut settings, Some("chosen".into()));
            assert_eq!(location.resolve(&settings), Some("chosen".into()));
            location.set(&mut settings, None);
            assert_eq!(location.chosen(&settings), None);
        }
    }

    #[test]
    fn each_location_has_its_own_slot() {
        let mut settings = Settings::default();
        Location::Templates.set(&mut settings, Some("t".into()));
        assert_eq!(Location::Recovery.chosen(&settings), None);
        assert_eq!(settings.templates_dir, Some("t".into()));
    }
}
