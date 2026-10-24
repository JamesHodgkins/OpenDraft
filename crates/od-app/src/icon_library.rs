//! Where the icon drawings are read from when the app starts (see
//! `od_ui::icon_set` and `assets/icons/README.md`), and saying which files
//! could not be used. The drawings are also built into the program, so a
//! folder is only a way to replace some of them.

#[cfg(debug_assertions)]
use std::path::Path;
use std::path::PathBuf;

use crate::cmdline::CommandLine;

/// `OD_ICONS` when it is set; else an `icons` folder beside the program (an
/// archive's copy); else, in a debug build, the source tree's `assets/icons`,
/// so a build run from the source shows redrawn icons on its next launch with
/// no rebuild. With none of these the built-in drawings are used.
pub fn folder() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("OD_ICONS") {
        return Some(dir.into());
    }
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("icons")))
        .filter(|dir| dir.is_dir());
    beside.or_else(source_tree)
}

#[cfg(debug_assertions)]
fn source_tree() -> Option<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icons");
    dir.is_dir().then_some(dir)
}

/// A release build does not carry the path it was built from.
#[cfg(not(debug_assertions))]
fn source_tree() -> Option<PathBuf> {
    None
}

/// Read the drawings, and say in the command line which files were skipped
/// (the built-in drawing is used instead).
pub fn load(cmdline: &mut CommandLine) {
    let loaded = od_ui::icon_set::load(folder().as_deref());
    for (file, why) in &loaded.failed {
        cmdline.say(format!(
            "Icon {file} could not be used ({why}); the built-in drawing is used instead."
        ));
    }
}
