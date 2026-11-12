//! Templates: a drawing saved to start others from, as `.odraftt`. The contents
//! are exactly those of an `.odraft` (JSON); only the extension differs, so a
//! template is not mistaken for a drawing in a file dialog.

use std::path::{Path, PathBuf};

use opendraft_core::Document;

use crate::native::NativeFormat;
use crate::{Encoded, FileError, FileFormat, Loaded};

pub const TEMPLATE_EXTENSION: &str = "odraftt";

pub struct TemplateFormat;

impl FileFormat for TemplateFormat {
    fn name(&self) -> &'static str {
        "OpenDraft template"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[TEMPLATE_EXTENSION]
    }

    fn check_size(&self, len: u64) -> Result<(), FileError> {
        NativeFormat.check_size(len)
    }

    fn read(&self, bytes: &[u8]) -> Result<Loaded, FileError> {
        NativeFormat.read(bytes)
    }

    fn write(&self, document: &Document) -> Result<Encoded, FileError> {
        NativeFormat.write(document)
    }
}

/// Whether `path` names a template.
pub fn is_template(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(TEMPLATE_EXTENSION))
}

/// `path` as a template's name: "plan" and "plan.odraft" save as "plan.odraftt".
pub fn template_path(path: &Path) -> PathBuf {
    if is_template(path) {
        path.to_owned()
    } else if crate::format_for(path).is_some() {
        path.with_extension(TEMPLATE_EXTENSION)
    } else {
        let mut name = path.as_os_str().to_owned();
        name.push(".");
        name.push(TEMPLATE_EXTENSION);
        PathBuf::from(name)
    }
}

/// The name and extensions of the template format.
pub fn template_filter() -> (&'static str, &'static [&'static str]) {
    (TemplateFormat.name(), TemplateFormat.extensions())
}

#[cfg(test)]
mod tests {
    use opendraft_core::{Entity, Point};

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("opendraft-tpl-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_template_is_a_drawing_under_its_own_extension() {
        let dir = scratch("roundtrip");
        let mut doc = Document::default();
        doc.add(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(3.0, 4.0),
        });
        let path = dir.join("starter.odraftt");
        crate::save(&path, &doc).unwrap();
        let loaded = crate::load(&path).unwrap();
        assert!(
            !loaded.imported,
            "it is not an import: nothing was left out"
        );
        assert_eq!(loaded.document.len(), 1);
        // The bytes are those of a native drawing.
        let odraft = dir.join("same.odraft");
        crate::save(&odraft, &doc).unwrap();
        assert_eq!(
            std::fs::read(&path).unwrap(),
            std::fs::read(&odraft).unwrap()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_get_the_template_extension() {
        assert_eq!(template_path(Path::new("a")), PathBuf::from("a.odraftt"));
        assert_eq!(
            template_path(Path::new("a.odraft")),
            PathBuf::from("a.odraftt")
        );
        assert_eq!(
            template_path(Path::new("a.ODRAFTT")),
            PathBuf::from("a.ODRAFTT")
        );
        assert!(is_template(Path::new("x/y.odraftt")));
        assert!(!is_template(Path::new("x/y.odraft")));
        // A template is opened and saved as a native drawing is.
        assert_eq!(
            crate::native_path(Path::new("a.odraftt")),
            PathBuf::from("a.odraft")
        );
    }
}
