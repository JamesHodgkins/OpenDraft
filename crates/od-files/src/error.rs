use std::fmt;
use std::io;
use std::path::PathBuf;

/// Why a drawing could not be read or written. The messages are written for
/// the user: they appear in the status bar and command line.
#[derive(Debug)]
pub enum FileError {
    Io(io::Error),
    /// No format handles this file's extension.
    UnknownFormat(PathBuf),
    /// Not a drawing this app wrote, or damaged: the text could not be read.
    Unreadable(String),
    /// A recognised format or feature this app does not support (an old
    /// version, a protected file): not the same as a damaged file.
    Unsupported(String),
    /// A file format version this one does not read, older or newer;
    /// `saved_by` names the build that wrote it ("" if the file does not say).
    WrongVersion {
        found: u32,
        supported: u32,
        saved_by: String,
    },
    /// Readable, but describes a drawing that cannot exist (an object on a
    /// layer that is not there, a circle with no radius, ...).
    Invalid(String),
    /// The user gave up on it before it finished.
    Cancelled,
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileError::Io(e) => write!(f, "{e}"),
            FileError::UnknownFormat(path) => {
                write!(
                    f,
                    "{} is not a drawing format OpenDraft knows",
                    path.display()
                )
            }
            FileError::Unsupported(why) => write!(f, "unsupported drawing feature: {why}"),
            FileError::Unreadable(why) => write!(f, "this is not a readable drawing ({why})"),
            FileError::WrongVersion {
                found,
                supported,
                saved_by,
            } => {
                let by = if saved_by.is_empty() {
                    "an unknown build"
                } else {
                    saved_by
                };
                write!(
                    f,
                    "it was saved by {by} in file format {found}, and this version reads only format {supported}"
                )
            }
            FileError::Invalid(why) => write!(f, "the drawing is damaged: {why}"),
            FileError::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::error::Error for FileError {}

impl From<io::Error> for FileError {
    fn from(error: io::Error) -> Self {
        FileError::Io(error)
    }
}
