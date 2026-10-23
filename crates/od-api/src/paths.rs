//! Which files an automation caller may touch: only regular files with an
//! expected extension, inside folders the user approved, named by a plain
//! absolute path. Everything is checked on the canonical path, so `..`,
//! symlinks and junctions cannot lead out of an approved folder, and device
//! names, alternate data streams and network paths are refused outright.
//!
//! A path checked here can still change before it is used (another process
//! swapping a link): the checks narrow what is possible, they do not make
//! the disk stand still.

use std::fs;
use std::path::{Component, Path, PathBuf, Prefix};

use crate::{ApiError, ErrorCode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// The file must exist.
    Read,
    /// The file may be new; its folder must exist.
    Write,
}

/// A path that passed, in canonical form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    pub path: PathBuf,
    /// Writing would replace a file that is there now.
    pub existed: bool,
}

impl Checked {
    /// The file's name without its folder: what may be shown to a model.
    pub fn file_name(&self) -> String {
        file_name(&self.path)
    }
}

pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn denied(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::PathDenied, message)
}

/// Windows file-name rules, applied everywhere so a drawing made on one
/// system is safe to name on another.
fn check_name(name: &std::ffi::OsStr) -> Result<(), ApiError> {
    let text = name.to_string_lossy();
    if text.ends_with('.') || text.ends_with(' ') {
        return Err(denied(
            "A file or folder name cannot end with a dot or space.",
        ));
    }
    if text.chars().any(|c| "<>:\"|?*".contains(c)) {
        return Err(denied(
            "A file or folder name cannot contain < > : \" | ? * (no alternate data streams).",
        ));
    }
    let stem = text.split('.').next().unwrap_or("").to_ascii_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        return Err(denied("That name is reserved by the operating system."));
    }
    Ok(())
}

/// On Windows `canonicalize` returns the extended form (`\\?\C:\...`).
/// The plain `C:\...` form is what the rest of the app uses (titles, recent
/// files, `same_file`) and what this module accepts as input, so a path
/// handed back must be plain too. Other forms are left alone.
fn plain(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path,
    }
}

fn canonical(path: &Path) -> std::io::Result<PathBuf> {
    path.canonicalize().map(plain)
}

fn inside(roots: &[PathBuf], path: &Path) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}

/// `raw` as a canonical path inside one of `roots`, or why not.
pub fn check(
    roots: &[PathBuf],
    raw: &str,
    access: Access,
    extensions: &[&str],
) -> Result<Checked, ApiError> {
    if raw.is_empty() || raw.len() > 4096 || raw.chars().any(char::is_control) {
        return Err(denied(
            "The path is empty, too long or has control characters.",
        ));
    }
    let path = Path::new(raw);
    if !path.is_absolute() {
        return Err(denied("Give an absolute path."));
    }
    for component in path.components() {
        match component {
            Component::Prefix(p) => {
                if !matches!(p.kind(), Prefix::Disk(_)) {
                    return Err(denied(
                        "Network, device and extended-length paths are not allowed.",
                    ));
                }
            }
            Component::ParentDir | Component::CurDir => {
                return Err(denied("A path cannot contain . or .. parts."));
            }
            Component::Normal(name) => check_name(name)?,
            Component::RootDir => {}
        }
    }
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    if !extension.is_some_and(|e| extensions.contains(&e.as_str())) {
        return Err(denied(format!(
            "The file must end in one of: {}.",
            extensions.join(", ")
        ))
        .detail("extensions", extensions.join(",")));
    }
    let roots: Vec<PathBuf> = roots.iter().filter_map(|r| canonical(r).ok()).collect();
    if roots.is_empty() {
        return Err(denied(
            "No folder has been approved yet. Ask the user to add one in OpenDraft (Settings > Automation).",
        ));
    }
    let (canonical, existed) = match access {
        Access::Read => {
            let meta = fs::metadata(path).map_err(|_| denied("There is no such file."))?;
            if !meta.is_file() {
                return Err(denied("That is not a regular file."));
            }
            let canonical = canonical(path).map_err(|_| denied("Cannot read that file."))?;
            (canonical, true)
        }
        Access::Write => match fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(denied("Will not write through a link."));
            }
            Ok(meta) if meta.is_file() => (
                canonical(path).map_err(|_| denied("Cannot reach that file."))?,
                true,
            ),
            Ok(_) => return Err(denied("That is not a regular file.")),
            Err(_) => {
                let parent = path
                    .parent()
                    .ok_or_else(|| denied("No folder in the path."))?;
                let folder = canonical(parent).map_err(|_| denied("The folder does not exist."))?;
                if !folder.is_dir() {
                    return Err(denied("The folder does not exist."));
                }
                let name = path.file_name().ok_or_else(|| denied("No file name."))?;
                (folder.join(name), false)
            }
        },
    };
    if !inside(&roots, &canonical) {
        return Err(denied(
            "That file is outside the folders the user approved. Ask them to add its folder in OpenDraft (Settings > Automation).",
        ));
    }
    Ok(Checked {
        path: canonical,
        existed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Area(PathBuf);

    impl Area {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("od-paths-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(dir.join("approved/sub")).unwrap();
            fs::create_dir_all(dir.join("approved-evil")).unwrap();
            fs::create_dir_all(dir.join("elsewhere")).unwrap();
            Area(dir)
        }

        fn root(&self) -> Vec<PathBuf> {
            vec![self.0.join("approved")]
        }

        fn file(&self, relative: &str) -> String {
            let path = self.0.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, b"x").unwrap();
            path.to_string_lossy().into_owned()
        }

        fn name(&self, relative: &str) -> String {
            self.0.join(relative).to_string_lossy().into_owned()
        }
    }

    impl Drop for Area {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const EXT: &[&str] = &["odx", "dxf"];

    fn code(r: Result<Checked, ApiError>) -> ErrorCode {
        r.unwrap_err().code
    }

    #[test]
    fn files_inside_an_approved_folder_pass_for_reading_and_writing() {
        let area = Area::new("ok");
        let existing = area.file("approved/sub/plan.odx");
        let read = check(&area.root(), &existing, Access::Read, EXT).unwrap();
        assert!(read.existed);
        assert!(
            !read.path.to_string_lossy().starts_with(r"\?\"),
            "a plain path, as the rest of the app writes it: {:?}",
            read.path
        );
        assert_eq!(read.file_name(), "plan.odx");
        let write = check(&area.root(), &existing, Access::Write, EXT).unwrap();
        assert!(write.existed, "writing would replace it");
        let new = check(
            &area.root(),
            &area.name("approved/new.DXF"),
            Access::Write,
            EXT,
        )
        .unwrap();
        assert!(!new.existed);
    }

    #[test]
    fn outside_the_approved_folders_is_refused_including_look_alike_folders() {
        let area = Area::new("outside");
        let other = area.file("elsewhere/a.odx");
        assert_eq!(
            code(check(&area.root(), &other, Access::Read, EXT)),
            ErrorCode::PathDenied
        );
        // "approved-evil" starts with the same letters but is another folder.
        let evil = area.file("approved-evil/a.odx");
        assert_eq!(
            code(check(&area.root(), &evil, Access::Read, EXT)),
            ErrorCode::PathDenied
        );
        let new_outside = area.name("elsewhere/new.odx");
        assert_eq!(
            code(check(&area.root(), &new_outside, Access::Write, EXT)),
            ErrorCode::PathDenied
        );
    }

    #[test]
    fn dot_dot_cannot_climb_out() {
        let area = Area::new("dots");
        let target = area.file("elsewhere/a.odx");
        let sneaky = format!(
            "{}{}approved{}sub{}..{}..{}elsewhere{}a.odx",
            area.0.display(),
            std::path::MAIN_SEPARATOR,
            std::path::MAIN_SEPARATOR,
            std::path::MAIN_SEPARATOR,
            std::path::MAIN_SEPARATOR,
            std::path::MAIN_SEPARATOR,
            std::path::MAIN_SEPARATOR
        );
        assert!(Path::new(&target).exists());
        assert_eq!(
            code(check(&area.root(), &sneaky, Access::Read, EXT)),
            ErrorCode::PathDenied
        );
    }

    #[test]
    fn nothing_approved_means_nothing_allowed() {
        let area = Area::new("none");
        let file = area.file("approved/a.odx");
        let error = check(&[], &file, Access::Read, EXT).unwrap_err();
        assert_eq!(error.code, ErrorCode::PathDenied);
        assert!(error.message.contains("Settings"));
        let missing_root = vec![area.0.join("no-such-folder")];
        assert!(check(&missing_root, &file, Access::Read, EXT).is_err());
    }

    #[test]
    fn odd_inputs_are_refused() {
        let area = Area::new("odd");
        let root = area.root();
        let good = area.file("approved/a.odx");
        for bad in [
            "",
            "relative/a.odx",
            "a.odx",
            "./a.odx",
            &format!("{good}\0"),
        ] {
            assert_eq!(
                code(check(&root, bad, Access::Read, EXT)),
                ErrorCode::PathDenied,
                "{bad:?}"
            );
        }
        assert!(check(&root, &"x".repeat(5000), Access::Read, EXT).is_err());
        // Wrong extension, no extension, a folder, a missing file.
        let txt = area.file("approved/notes.txt");
        assert!(check(&root, &txt, Access::Read, EXT).is_err());
        let bare = area.file("approved/noext");
        assert!(check(&root, &bare, Access::Read, EXT).is_err());
        let folder = area.name("approved/sub.odx");
        fs::create_dir_all(&folder).unwrap();
        assert!(check(&root, &folder, Access::Read, EXT).is_err());
        assert!(check(&root, &folder, Access::Write, EXT).is_err());
        assert!(check(&root, &area.name("approved/missing.odx"), Access::Read, EXT).is_err());
        assert!(check(
            &root,
            &area.name("approved/no-folder/a.odx"),
            Access::Write,
            EXT
        )
        .is_err());
    }

    #[test]
    fn device_names_streams_and_trailing_dots_are_refused() {
        let area = Area::new("names");
        let root = area.root();
        for name in [
            "approved/CON.odx",
            "approved/nul.odx",
            "approved/Com1.odx",
            "approved/aux.dxf",
            "approved/a.odx:stream.odx",
            "approved/bad?.odx",
            "approved/trailing.odx.",
            "approved/space.odx ",
            "approved/a|b.odx",
        ] {
            assert_eq!(
                code(check(&root, &area.name(name), Access::Write, EXT)),
                ErrorCode::PathDenied,
                "{name}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn network_and_extended_paths_are_refused_on_windows() {
        let root = vec![std::env::temp_dir()];
        for bad in [r"\\server\share\a.odx", r"\\?\C:\a.odx", r"\\.\pipe\x.odx"] {
            assert_eq!(
                code(check(&root, bad, Access::Write, EXT)),
                ErrorCode::PathDenied,
                "{bad}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_link_cannot_lead_out_or_be_written_through() {
        use std::os::unix::fs::symlink;
        let area = Area::new("links");
        let outside = area.file("elsewhere/secret.odx");
        symlink(&outside, area.0.join("approved/link.odx")).unwrap();
        let link = area.name("approved/link.odx");
        assert!(
            check(&area.root(), &link, Access::Read, EXT).is_err(),
            "reads follow to the real file"
        );
        assert!(check(&area.root(), &link, Access::Write, EXT).is_err());
        symlink(area.0.join("elsewhere"), area.0.join("approved/dir")).unwrap();
        let through = area.name("approved/dir/new.odx");
        assert!(check(&area.root(), &through, Access::Write, EXT).is_err());
    }
}
