//! Reading and writing drawings. No UI dependencies.
//!
//! Each file format is a [`FileFormat`]; the native one is [`NativeFormat`]
//! (`.odx`, JSON). Others (DXF, DWG) plug in the same way, so the app asks
//! [`format_for`] what to use for a path and never names a format itself.

mod block_import;
mod dwg;
mod dxf;
mod error;
mod linetype_import;
mod native;
mod plot;
mod plot_png;
mod template;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use od_core::Document;

pub use dwg::DwgFormat;
pub use dxf::DxfFormat;
pub use error::FileError;
pub use native::{NativeFormat, CURRENT_VERSION, NATIVE_EXTENSION};
pub use plot::{pdf, plot_cancellable, plot_filter, svg};
pub use plot_png::{png, png_size, PngError};
pub use template::{
    is_template, template_filter, template_path, TemplateFormat, TEMPLATE_EXTENSION,
};

/// "1 object", "2 objects": a count and a noun for a warning.
pub(crate) fn plural(count: u64, noun: &str) -> String {
    if count == 1 {
        format!("{count} {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// A drawing read from a file, with what the user should know about it.
pub struct Loaded {
    pub document: Document,
    /// Things in the file that did not come across (unsupported objects,
    /// curves turned into segments), worded for the user.
    pub warnings: Vec<String>,
    /// The file is in an exchange format, not the app's own: open it as a new
    /// unsaved drawing rather than as the file itself.
    pub imported: bool,
}

/// A drawing encoded for a file.
pub struct Encoded {
    pub bytes: Vec<u8>,
    /// What this format could not keep (colours approximated, lineweights
    /// dropped), worded for the user. Empty for a lossless format.
    pub notes: Vec<String>,
}

/// A way of storing a drawing in bytes.
pub trait FileFormat: Sync {
    /// Shown in file dialogs, e.g. "OpenDraft drawing".
    fn name(&self) -> &'static str;
    /// Lower-case extensions without the dot; the first is the default.
    fn extensions(&self) -> &'static [&'static str];
    fn read(&self, bytes: &[u8]) -> Result<Loaded, FileError>;
    /// Like `read`, giving up with `FileError::Cancelled` soon after
    /// `cancelled` is set. A format that reads quickly need not look at it.
    fn read_cancellable(&self, bytes: &[u8], cancelled: &AtomicBool) -> Result<Loaded, FileError> {
        let _ = cancelled;
        self.read(bytes)
    }
    /// Refuse a file of `len` bytes before it is read into memory. Formats
    /// without a size limit take anything.
    fn check_size(&self, len: u64) -> Result<(), FileError> {
        let _ = len;
        Ok(())
    }
    fn write(&self, document: &Document) -> Result<Encoded, FileError>;
}

/// Every format the app can open, native first.
pub const FORMATS: &[&dyn FileFormat] = &[&NativeFormat, &TemplateFormat, &DxfFormat, &DwgFormat];

/// The format whose extension `path` has.
pub fn format_for(path: &Path) -> Option<&'static dyn FileFormat> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    FORMATS
        .iter()
        .copied()
        .find(|f| f.extensions().contains(&extension.as_str()))
}

/// Whether `path` names a file in the app's own format.
pub fn is_native(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(NATIVE_EXTENSION))
}

/// `path` as a native drawing's name: "plan" saves as "plan.odx", and
/// "plan.dxf" as "plan.odx" (a DXF is exported, not saved to).
pub fn native_path(path: &Path) -> PathBuf {
    if is_native(path) {
        path.to_owned()
    } else if format_for(path).is_some() {
        path.with_extension(NATIVE_EXTENSION)
    } else {
        let mut name = path.as_os_str().to_owned();
        name.push(".");
        name.push(NATIVE_EXTENSION);
        PathBuf::from(name)
    }
}

/// Open a drawing, choosing the format from the file's extension.
pub fn load(path: &Path) -> Result<Loaded, FileError> {
    load_cancellable(path, &AtomicBool::new(false))
}

/// Like [`load`], but gives up with `FileError::Cancelled` soon after
/// `cancelled` is set (for a slow import the user gave up on). A file over
/// its format's size limit is refused before it is read.
pub fn load_cancellable(path: &Path, cancelled: &AtomicBool) -> Result<Loaded, FileError> {
    let format = format_for(path).ok_or_else(|| FileError::UnknownFormat(path.to_owned()))?;
    format.check_size(fs::metadata(path)?.len())?;
    let bytes = fs::read(path)?;
    format.read_cancellable(&bytes, cancelled)
}

/// Save a drawing, in the format the extension names. Returns what that
/// format could not keep. The bytes go to a temporary file beside `path`
/// first and replace it only once fully written, so a crash or a full disk
/// never leaves a half-written drawing where a good one was.
pub fn save(path: &Path, document: &Document) -> Result<Vec<String>, FileError> {
    save_cancellable(path, document, &AtomicBool::new(false))
}

/// Like [`save`], but once `cancelled` is set the file is not put in place:
/// a write that was stuck on a slow disk and finishes later must not replace
/// the drawing the user chose not to save over.
pub fn save_cancellable(
    path: &Path,
    document: &Document,
    cancelled: &AtomicBool,
) -> Result<Vec<String>, FileError> {
    let format = format_for(path).ok_or_else(|| FileError::UnknownFormat(path.to_owned()))?;
    let Encoded { bytes, notes } = format.write(document)?;
    write_atomically(path, &bytes, cancelled)?;
    Ok(notes)
}

/// Put `bytes` at `path` through a temporary file beside it, so the old file
/// is replaced only once the new one is fully written (see `save`), unless
/// `cancelled` is set first.
pub(crate) fn write_atomically(
    path: &Path,
    bytes: &[u8],
    cancelled: &AtomicBool,
) -> Result<(), FileError> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(FileError::Cancelled);
    }
    let (temporary, mut file) = create_beside(path)?;
    let written = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        if cancelled.load(Ordering::Relaxed) {
            return Err(std::io::ErrorKind::Interrupted.into());
        }
        fs::rename(&temporary, path)
    })();
    if written.is_err() {
        // Ours alone: `create_beside` made it, and refuses to reuse a name.
        let _ = fs::remove_file(&temporary);
    }
    written?;
    sync_directory(path);
    Ok(())
}

/// A new, empty file in the same folder as `path` (so the final rename stays
/// on one volume), named for this process and save so that nothing else, not
/// another instance and not a file the user made, can have the name. It is
/// created exclusively, so an existing file is never touched.
fn create_beside(path: &Path) -> std::io::Result<(PathBuf, fs::File)> {
    static SAVES: AtomicU64 = AtomicU64::new(0);
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let mut last = None;
    for _ in 0..8 {
        let n = SAVES.fetch_add(1, Ordering::Relaxed);
        let temporary = path.with_file_name(format!(".{name}.{}-{n}.tmp", std::process::id()));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| std::io::ErrorKind::AlreadyExists.into()))
}

/// Make the rename itself durable where the platform allows it (a folder can
/// be opened and synced on Unix, not on Windows). Best effort: the drawing is
/// already in place, so a failure here is not worth reporting.
fn sync_directory(path: &Path) {
    #[cfg(unix)]
    if let Some(folder) = path.parent() {
        let folder = if folder.as_os_str().is_empty() {
            Path::new(".")
        } else {
            folder
        };
        if let Ok(folder) = fs::File::open(folder) {
            let _ = folder.sync_all();
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// The filters for an Open dialog: all drawings first, then each format.
pub fn open_filters() -> Vec<(String, Vec<&'static str>)> {
    let all: Vec<&'static str> = FORMATS
        .iter()
        .flat_map(|f| f.extensions().iter().copied())
        .collect();
    let mut filters = vec![("All drawings".to_owned(), all)];
    filters.extend(
        FORMATS
            .iter()
            .map(|f| (f.name().to_owned(), f.extensions().to_vec())),
    );
    filters
}

/// The name and extensions of the format that exports to DXF.
pub fn dxf_filter() -> (&'static str, &'static [&'static str]) {
    (DxfFormat.name(), DxfFormat.extensions())
}

/// The name and extensions of the app's own format.
pub fn native_filter() -> (&'static str, &'static [&'static str]) {
    (NativeFormat.name(), NativeFormat.extensions())
}

#[cfg(test)]
mod tests {
    use od_core::{Entity, Point};

    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("od-files-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn drawing() -> Document {
        Document::from_entities([Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(3.0, 4.0),
        }])
    }

    #[test]
    fn the_extension_picks_the_format() {
        assert!(format_for(Path::new("plan.odx")).is_some());
        assert!(format_for(Path::new("PLAN.ODX")).is_some());
        assert!(format_for(Path::new("plan.txt")).is_none());
        assert!(format_for(Path::new("plan")).is_none());
    }

    #[test]
    fn a_native_name_gets_the_native_extension() {
        assert_eq!(
            native_path(Path::new("dir/plan")),
            PathBuf::from("dir/plan.odx")
        );
        assert_eq!(
            native_path(Path::new("dir/plan.odx")),
            PathBuf::from("dir/plan.odx")
        );
        // A name with other dots still gets one.
        assert_eq!(
            native_path(Path::new("v1.2 plan")),
            PathBuf::from("v1.2 plan.odx")
        );
        // A DXF name is not saved to: it becomes the native name.
        assert_eq!(
            native_path(Path::new("dir/plan.dxf")),
            PathBuf::from("dir/plan.odx")
        );
        assert!(is_native(Path::new("a.ODX")) && !is_native(Path::new("a.dxf")));
    }

    #[test]
    fn saving_then_loading_gives_the_same_drawing() {
        let dir = scratch_dir("roundtrip");
        let path = dir.join("plan.odx");
        let document = drawing();
        save(&path, &document).unwrap();
        assert_eq!(load(&path).unwrap().document, document);
        let names: Vec<_> = fs::read_dir(&dir).unwrap().flatten().collect();
        assert_eq!(names.len(), 1, "no temporary file left");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn saving_replaces_an_existing_file() {
        let dir = scratch_dir("replace");
        let path = dir.join("plan.odx");
        save(&path, &Document::default()).unwrap();
        save(&path, &drawing()).unwrap();
        assert_eq!(load(&path).unwrap().document.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_save_leaves_no_temporary_and_touches_no_other_file() {
        let dir = scratch_dir("failed");
        let path = dir.join("plan.odx");
        // A user's own file with the name the old scheme would have used.
        let theirs = dir.join("plan.odx.tmp");
        fs::write(&theirs, "not ours").unwrap();
        // A folder where the drawing should go makes the final rename fail.
        fs::create_dir(&path).unwrap();
        assert!(save(&path, &drawing()).is_err());
        assert_eq!(fs::read_to_string(&theirs).unwrap(), "not ours");
        assert!(path.is_dir());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2, "nothing else left");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cancelled_save_leaves_the_old_file_and_no_temporary() {
        let dir = scratch_dir("cancelled");
        let path = dir.join("plan.odx");
        save(&path, &drawing()).unwrap();
        let cancelled = AtomicBool::new(true);
        assert!(save_cancellable(&path, &Document::default(), &cancelled).is_err());
        assert_eq!(load(&path).unwrap().document.len(), 1, "old file kept");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1, "no temporary");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_saves_never_share_a_temporary_name() {
        let dir = scratch_dir("names");
        let path = dir.join("plan.odx");
        let (first, _a) = create_beside(&path).unwrap();
        let (second, _b) = create_beside(&path).unwrap();
        assert_ne!(first, second);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_extensions_and_missing_files_are_errors_not_panics() {
        let dir = scratch_dir("errors");
        assert!(matches!(
            load(&dir.join("plan.xyz")),
            Err(FileError::UnknownFormat(_))
        ));
        assert!(matches!(
            load(&dir.join("missing.odx")),
            Err(FileError::Io(_))
        ));
        assert!(matches!(
            save(&dir.join("plan.xyz"), &drawing()),
            Err(FileError::UnknownFormat(_))
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_filters_start_with_everything() {
        let filters = open_filters();
        assert_eq!(filters[0].0, "All drawings");
        assert_eq!(filters[0].1, vec!["odx", "odt", "dxf", "dwg"]);
        assert_eq!(filters[1].1, vec!["odx"]);
        assert_eq!(filters[2], ("OpenDraft template".to_owned(), vec!["odt"]));
        assert_eq!(filters[3].1, vec!["dxf"]);
        assert_eq!(filters[4], ("AutoCAD DWG".to_owned(), vec!["dwg"]));
    }

    #[test]
    fn a_dwg_name_is_not_saved_to() {
        assert_eq!(
            native_path(Path::new("dir/plan.dwg")),
            PathBuf::from("dir/plan.odx")
        );
        let dir = scratch_dir("dwgsave");
        assert!(matches!(
            save(&dir.join("plan.dwg"), &drawing()),
            Err(FileError::Unsupported(_))
        ));
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0, "nothing written");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_dxf_saves_and_loads_as_an_import() {
        let dir = scratch_dir("dxf");
        let path = dir.join("plan.dxf");
        let notes = save(&path, &drawing()).unwrap();
        assert!(notes.is_empty(), "a plain line loses nothing");
        let loaded = load(&path).unwrap();
        assert!(loaded.imported);
        assert_eq!(loaded.document.len(), 1);
        assert!(!load_native_flag(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Native files are not imports.
    fn load_native_flag(dir: &Path) -> bool {
        let path = dir.join("plan.odx");
        save(&path, &drawing()).unwrap();
        load(&path).unwrap().imported
    }
}
