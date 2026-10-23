//! AutoCAD DWG, read by the standalone `od-dwg` crate. Like DXF it is an
//! exchange format: opening one is an import (a new unsaved drawing), and what
//! did not come across is reported. Writing is not built yet.

mod convert;
mod text;
mod warnings;

use std::sync::atomic::AtomicBool;

use od_core::Document;
use od_dwg::{Drawing, DwgError, DwgErrorKind, DwgLimits};

use crate::{Encoded, FileError, FileFormat, Loaded};

pub struct DwgFormat;

/// A reader error as a file error. Damage is `Invalid`, a file that is not a
/// DWG `Unreadable`, and what is recognised but refused (an old version, a
/// protected file, a drawing over a size limit) `Unsupported`.
fn file_error(error: &DwgError) -> FileError {
    match (&error.kind, error.damage()) {
        (DwgErrorKind::Cancelled, _) => FileError::Cancelled,
        (DwgErrorKind::InvalidHeader, Some(why)) => FileError::Unreadable(why),
        (_, Some(why)) => FileError::Invalid(why),
        (_, None) => FileError::Unsupported(error.user_message()),
    }
}

impl FileFormat for DwgFormat {
    fn name(&self) -> &'static str {
        "AutoCAD DWG"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["dwg"]
    }

    fn read(&self, bytes: &[u8]) -> Result<Loaded, FileError> {
        self.read_cancellable(bytes, &AtomicBool::new(false))
    }

    fn read_cancellable(&self, bytes: &[u8], cancelled: &AtomicBool) -> Result<Loaded, FileError> {
        let drawing =
            Drawing::read(bytes, &DwgLimits::default(), cancelled).map_err(|e| file_error(&e))?;
        let (document, notes) = convert::to_document(&drawing, cancelled)?;
        let mut warnings = warnings::words(&drawing.report);
        warnings.extend(warnings::conversion_words(&notes));
        Ok(Loaded {
            document,
            warnings,
            imported: true,
        })
    }

    /// The reader's own file size limit, checked before the bytes are read
    /// so that a huge file is never loaded into memory just to be refused.
    fn check_size(&self, len: u64) -> Result<(), FileError> {
        if len > DwgLimits::default().max_file_bytes {
            return Err(FileError::Unsupported(
                "this DWG exceeds the safe file size limit".into(),
            ));
        }
        Ok(())
    }

    fn write(&self, _document: &Document) -> Result<Encoded, FileError> {
        Err(FileError::Unsupported(
            "writing DWG files is not available yet".into(),
        ))
    }
}
