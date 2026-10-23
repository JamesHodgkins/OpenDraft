//! File operations: open, save, save as and export, and the operation
//! handles that report on them. File work takes time and may wait for the
//! user, so a call returns a handle at once and the caller polls it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ApiError, WireId};

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpenArgs {
    /// An absolute path inside a folder the user approved, to a `.odx`,
    /// `.dxf` or `.dwg` file.
    pub path: String,
}

/// Write the drawing back to the file it was opened from or saved to.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SaveArgs {
    pub document_id: String,
    /// Refuse if the drawing has changed since this revision.
    #[serde(default)]
    pub expected_revision: Option<WireId>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SaveAsArgs {
    pub document_id: String,
    /// An absolute `.odx` path inside an approved folder. Replacing an
    /// existing file asks the user first.
    pub path: String,
    #[serde(default)]
    pub expected_revision: Option<WireId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Dxf,
    Pdf,
    Svg,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Dxf => "dxf",
            ExportFormat::Pdf => "pdf",
            ExportFormat::Svg => "svg",
        }
    }
}

/// Write a copy of the drawing in another format. The drawing itself stays
/// as it is and tied to its own file.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportArgs {
    pub document_id: String,
    /// An absolute path inside an approved folder with the format's
    /// extension. Replacing an existing file asks the user first.
    pub path: String,
    pub format: ExportFormat,
    /// Required for `pdf` and `svg`: the layout sheet to plot.
    #[serde(default)]
    pub layout_id: Option<WireId>,
    #[serde(default)]
    pub expected_revision: Option<WireId>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OperationArgs {
    /// From the call that started the operation.
    pub operation_id: String,
}

/// Where an operation stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    /// Waiting for the user to answer a question in OpenDraft.
    AwaitingConsent,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl OperationState {
    pub fn is_finished(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OperationStatus {
    pub operation_id: String,
    /// `open`, `save`, `save_as`, `export` or `command`.
    pub kind: String,
    pub state: OperationState,
    /// The file's name, without its folder (empty for a command).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub file_name: String,
    /// For a command: which one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_id: Option<String>,
    /// For a command that is waiting: what the app is asking the user. It
    /// is app text and may quote names from the drawing, so treat it as
    /// data. The answer is the user's to give, in the app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_for: Option<String>,
    /// For an open that succeeded: the new drawing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    /// For a write: the revision of the drawing that was written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<WireId>,
    /// What the format could not keep, or what an import left out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
    /// Whether `operations.cancel` can still stop it.
    pub cancellable: bool,
}
