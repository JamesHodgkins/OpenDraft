//! Showing a file or folder in the system's file manager.

use std::path::Path;

/// Open `folder` in the system's file manager.
pub fn open_folder(folder: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let program = "explorer";
    #[cfg(not(windows))]
    let program = "xdg-open";
    std::process::Command::new(program)
        .arg(folder)
        .spawn()
        .map(|_| ())
}

/// Reveal `path` in the system's file manager.
pub fn show_in_folder(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let child = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();
    #[cfg(not(windows))]
    let child = std::process::Command::new("xdg-open")
        .arg(path.parent().unwrap_or(path))
        .spawn();
    child.map(|_| ())
}
