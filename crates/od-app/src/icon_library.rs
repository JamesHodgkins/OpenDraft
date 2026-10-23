//! Where the icon drawings are read from when the app starts (see
//! `od_ui::icon_set` and `assets/icons/README.md`), and saying which files
//! could not be used.

use std::path::{Path, PathBuf};

use crate::cmdline::CommandLine;

/// `OD_ICONS` when it is set; else an `icons` folder beside the program (an
/// installed copy); else the source tree's `assets/icons`, so a build run from
/// the source shows redrawn icons on its next launch with no rebuild.
pub fn folder() -> PathBuf {
    if let Some(dir) = std::env::var_os("OD_ICONS") {
        return dir.into();
    }
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("icons")));
    match beside {
        Some(dir) if dir.is_dir() => dir,
        _ => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icons"),
    }
}

/// Read the drawings, and say in the command line which files were skipped
/// (their glyphs are drawn instead).
pub fn load(cmdline: &mut CommandLine) {
    let loaded = od_ui::icon_set::load(&folder());
    for (file, why) in &loaded.failed {
        cmdline.say(format!(
            "Icon {file} could not be used ({why}); its glyph is drawn instead."
        ));
    }
}
