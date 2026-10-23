use std::fmt;

use crate::handles::DwgHandle;

/// What went wrong, without saying where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DwgErrorKind {
    /// The data ended before a field or block did.
    Truncated,
    /// A DWG, but not one of the versions this reader handles (the marker is kept).
    UnsupportedVersion([u8; 6]),
    /// Password-protected or encrypted content.
    UnsupportedProtection,
    /// Not a DWG, or its file header is inconsistent.
    InvalidHeader,
    InvalidSectionMap,
    MissingSection(&'static str),
    ChecksumMismatch,
    InvalidCompression,
    /// A handle reference that cannot be decoded or resolved.
    InvalidHandle,
    /// An object whose envelope, streams or fields are inconsistent.
    InvalidObject,
    /// Text that is not valid UTF-16.
    InvalidText,
    LimitExceeded {
        limit: &'static str,
        value: u64,
    },
    Cancelled,
}

/// The step of reading a file that failed, for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DwgStage {
    Probe,
    FileHeader,
    PageMap,
    SectionMap,
    SectionPage,
    Decompress,
    Header,
    Classes,
    ObjectMap,
    Object,
}

impl DwgStage {
    fn name(self) -> &'static str {
        match self {
            DwgStage::Probe => "version check",
            DwgStage::FileHeader => "file header",
            DwgStage::PageMap => "page map",
            DwgStage::SectionMap => "section map",
            DwgStage::SectionPage => "section page",
            DwgStage::Decompress => "decompression",
            DwgStage::Header => "header",
            DwgStage::Classes => "class table",
            DwgStage::ObjectMap => "object map",
            DwgStage::Object => "object",
        }
    }
}

/// A failure to read a DWG, with enough context to find it in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DwgError {
    pub kind: DwgErrorKind,
    pub stage: DwgStage,
    /// Byte offset in the file, or in the block being read, when known.
    pub offset: Option<u64>,
    pub section: Option<String>,
    /// The object being read, when the failure is inside one.
    pub object_handle: Option<DwgHandle>,
}

impl DwgError {
    pub(crate) fn new(kind: DwgErrorKind, stage: DwgStage) -> Self {
        DwgError {
            kind,
            stage,
            offset: None,
            section: None,
            object_handle: None,
        }
    }

    pub(crate) fn at(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }

    pub(crate) fn of_object(mut self, handle: DwgHandle) -> Self {
        self.object_handle = Some(handle);
        self
    }

    pub(crate) fn in_section(mut self, name: &str) -> Self {
        self.section = Some(name.to_owned());
        self
    }

    /// Why a file is damaged, as a short phrase ("it ends too early"), for the
    /// kinds that mean damage; `None` for the others.
    pub fn damage(&self) -> Option<String> {
        Some(match &self.kind {
            DwgErrorKind::Truncated => "it ends too early".to_owned(),
            DwgErrorKind::InvalidHeader => {
                "it is not a DWG file, or its header is damaged".to_owned()
            }
            DwgErrorKind::InvalidSectionMap => format!("its {} is inconsistent", self.stage.name()),
            DwgErrorKind::MissingSection(name) => format!("the {name} section is missing"),
            DwgErrorKind::ChecksumMismatch => match &self.section {
                Some(name) => format!("{name} checksum does not match"),
                None => format!("the {} checksum does not match", self.stage.name()),
            },
            DwgErrorKind::InvalidCompression => "a compressed block is invalid".to_owned(),
            DwgErrorKind::InvalidHandle
            | DwgErrorKind::InvalidObject
            | DwgErrorKind::InvalidText => {
                format!("an invalid entry in its {}", self.stage.name())
            }
            _ => return None,
        })
    }

    /// A one-line message fit for a user; offsets and stages are left to `Debug`.
    pub fn user_message(&self) -> String {
        if let Some(why) = self.damage() {
            return match self.kind {
                DwgErrorKind::InvalidHeader => "this is not a readable DWG file".to_owned(),
                _ => format!("this DWG is damaged ({why})"),
            };
        }
        match &self.kind {
            DwgErrorKind::UnsupportedVersion(marker) => format!(
                "this DWG uses an unsupported version ({})",
                String::from_utf8_lossy(marker)
            ),
            DwgErrorKind::UnsupportedProtection => {
                "this DWG is password-protected; protected drawings are not supported".to_owned()
            }
            DwgErrorKind::LimitExceeded { limit, .. } => {
                format!("this DWG exceeds the safe {limit} limit")
            }
            _ => "cancelled".to_owned(),
        }
    }
}

impl fmt::Display for DwgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.user_message())
    }
}

impl std::error::Error for DwgError {}
